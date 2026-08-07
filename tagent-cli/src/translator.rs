use crate::config::{self, ConfigManager};
use crate::platform::{ClipboardManager, WindowHandle, WindowManager};
use rustyline::ExternalPrinter;
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tagent::providers::{self, TranslationProvider};

/// Shared slot for the rustyline external printer used to route hotkey-triggered
/// translation output safely while the interactive prompt may be mid-read on another
/// thread. `None` until [`InteractiveMode::start`](crate::interactive::InteractiveMode::start)
/// installs one; always `None` in CLI mode, which never calls [`Translator::translate_clipboard`].
type SharedPrinter = Arc<Mutex<Option<Box<dyn ExternalPrinter + Send>>>>;

/// High-level translation orchestrator.
///
/// `Translator` ties together a [`TranslationProvider`] (from the [`tagent`] library
/// crate), the system clipboard, and optional window management to provide the full
/// Tagent translation experience. Use [`Translator::new_cli`] when window management
/// is not needed (e.g. one-off CLI translations).
#[derive(Clone)]
pub struct Translator {
    provider: Arc<dyn TranslationProvider>,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Option<Arc<WindowManager>>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<WindowHandle>>>,
    printer: SharedPrinter,
}

impl Translator {
    /// Create a full translator for unified mode, including window management for
    /// showing/hiding the terminal. If window management fails to initialize (e.g. no
    /// display server), falls back to a translator without it rather than erroring out —
    /// use [`Translator::new_cli`] instead if window management is never needed.
    pub fn new_with_config(
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let window_manager = match WindowManager::new() {
            Ok(wm) => Some(Arc::new(wm)),
            Err(_) => {
                eprintln!(
                    "Window management unavailable (show/hide terminal and hotkeys disabled)."
                );
                eprintln!(
                    "This is expected on Wayland or when running outside a graphical terminal."
                );
                None
            }
        };

        Self::build(config_manager, window_manager)
    }

    /// Create Translator without window management (for CLI mode)
    pub fn new_cli(
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::build(config_manager, None)
    }

    fn build(
        config_manager: Arc<ConfigManager>,
        window_manager: Option<Arc<WindowManager>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Create translation provider based on config
        let config = config_manager.get_config();
        let provider = providers::create_provider(&config.translate_provider)?;

        Ok(Self {
            provider: Arc::from(provider),
            clipboard: ClipboardManager::new(),
            config_manager,
            window_manager,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
        })
    }

    /// Install the external printer used to route [`translate_clipboard`](Self::translate_clipboard)
    /// output safely while the interactive prompt may be reading a line in raw mode.
    /// Called once, from `InteractiveMode::start()`, right after the rustyline `Editor` is built.
    pub fn set_external_printer(&self, printer: impl ExternalPrinter + Send + 'static) {
        *self.printer.lock().unwrap() = Some(Box::new(printer));
    }

    /// True once an external printer has been installed via [`set_external_printer`](Self::set_external_printer).
    fn has_external_printer(&self) -> bool {
        self.printer.lock().unwrap().is_some()
    }

    /// Emit hotkey-triggered translation output, routed through the external printer when
    /// one is installed (so it can't corrupt an interactive prompt mid-read on another
    /// thread), or printed directly to stdout otherwise.
    fn emit(&self, msg: &str) {
        let mut guard = self.printer.lock().unwrap();
        if let Some(printer) = guard.as_mut() {
            if printer.print(msg.to_string()).is_ok() {
                return;
            }
        }
        drop(guard);
        print!("{}", msg);
        io::stdout().flush().ok();
    }

    /// Like [`emit`](Self::emit) but appends a trailing newline, mirroring `println!`.
    fn emit_line(&self, msg: impl AsRef<str>) {
        self.emit(&format!("{}\n", msg.as_ref()));
    }

    /// Copy text to clipboard if enabled in config
    fn copy_to_clipboard_if_enabled(
        &self,
        text: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if config.copy_to_clipboard {
            self.clipboard
                .set_text(text)
                .map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })
        } else {
            Ok(())
        }
    }

    /// Get the source display name (used for prompt labels)
    fn source_display(source_code: &str, config: &crate::config::Config) -> String {
        if source_code == "auto" {
            "Auto".to_string()
        } else {
            config.source_language.clone()
        }
    }

    /// Reprint the source language prompt after hotkey-triggered output, but only on the
    /// no-printer fallback path — once an external printer is installed, rustyline redraws
    /// the real (possibly non-empty) prompt itself, and a plain `print!` here would bypass
    /// the printer and corrupt it.
    fn maybe_print_source_prompt(&self, config: &crate::config::Config) {
        if self.has_external_printer() {
            return;
        }
        let source_prompt = format!("[{}]: ", config.source_language);
        config::print_colored(&source_prompt, &config.source_prompt_color);
        io::stdout().flush().ok();
    }

    /// Main function for translating text from clipboard
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check if config file was modified and reload if necessary
        if let Err(e) = self.config_manager.check_and_reload() {
            self.emit_line(format!("Config reload error: {}", e));
        }

        let config = self.config_manager.get_config();

        // Store the current foreground window before any operations
        if config.show_terminal_on_translate {
            if let Some(wm) = &self.window_manager {
                if let Some(fg_window) = wm.get_foreground_window() {
                    if let Ok(mut stored) = self.stored_foreground_window.lock() {
                        *stored = Some(fg_window);
                    }
                }
            }
        }

        let original_text = match self.clipboard.get_text_with_copy() {
            Ok(text) => {
                if text.trim().is_empty() {
                    self.emit_line("No selected text or clipboard is empty");
                    return Ok(());
                }
                text.trim().to_string()
            }
            Err(e) => {
                self.emit_line(format!("Copy or clipboard read error: {}", e));
                return Err(e.to_string().into());
            }
        };

        // Show terminal window if configured
        if config.show_terminal_on_translate {
            if let Some(wm) = &self.window_manager {
                if let Err(e) = wm.show_terminal() {
                    self.emit_line(format!("Failed to show terminal: {}", e));
                }
            }
        }

        let (source_code, target_code) = self.config_manager.get_language_codes();

        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && config::is_single_word(&original_text) {
            match self
                .get_dictionary_entry(&original_text, &source_code, &target_code)
                .await
            {
                Ok((dictionary_info, corrected_word)) => {
                    // Clear any existing prompt and print on new line (no-printer fallback only;
                    // with a printer installed, rustyline handles redrawing on its own).
                    if !self.has_external_printer() {
                        print!("\r");
                        io::stdout().flush().ok();
                    }

                    // Show the original text (source word)
                    let source_display = Self::source_display(&source_code, &config);
                    let source_label = format!("[{}]: ", source_display);
                    self.emit_line(format!(
                        "{}{}",
                        config::colorize(&source_label, &config.source_prompt_color),
                        original_text
                    ));

                    // If a spelling correction was applied, notify the user
                    if config.spell_check {
                        if let Some(ref corrected) = corrected_word {
                            if corrected.to_lowercase() != original_text.to_lowercase() {
                                self.emit_line(Self::correction_notice(corrected, &target_code));
                            }
                        }
                    }

                    // Print colored dictionary label
                    self.emit_line(format!(
                        "{}{}\n",
                        config::colorize("[Word]: ", &config.dictionary_prompt_color),
                        dictionary_info
                    ));

                    if let Err(e) = self.copy_to_clipboard_if_enabled(&dictionary_info, &config) {
                        self.emit_line(format!("Dictionary clipboard write error: {}", e));
                    }

                    // Save dictionary entry to history
                    if let Err(e) = config::save_translation_history(
                        &original_text,
                        &dictionary_info,
                        &source_code,
                        &target_code,
                        &config,
                    ) {
                        self.emit_line(format!("History save error: {}", e));
                    }

                    // Show source language prompt after hotkey translation
                    self.maybe_print_source_prompt(&config);
                }
                Err(_) => {
                    // Fall back to regular translation
                    self.perform_translation(&original_text, &source_code, &target_code, &config)
                        .await?;
                }
            }
        } else {
            // Regular translation for phrases or when dictionary is disabled
            self.perform_translation(&original_text, &source_code, &target_code, &config)
                .await?;
        }

        // Hide terminal and restore previous window after delay if configured
        if config.show_terminal_on_translate
            && config.auto_hide_terminal_seconds > 0
            && self.window_manager.is_some()
        {
            self.hide_terminal_and_restore(config.auto_hide_terminal_seconds)
                .await;
        }

        Ok(())
    }

    /// Perform regular translation
    async fn perform_translation(
        &self,
        text: &str,
        source_code: &str,
        target_code: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Clear any existing prompt and move to new line (no-printer fallback only).
        if !self.has_external_printer() {
            print!("\r");
            io::stdout().flush().ok();
        }

        // Show source language info with colored prompt
        let source_display = Self::source_display(source_code, config);
        let source_label = format!("[{}]: ", source_display);
        self.emit_line(format!(
            "{}{}",
            config::colorize(&source_label, &config.source_prompt_color),
            text
        ));

        // If source language is not Auto, check if text matches expected language
        if source_code != "auto" && !self.is_expected_language(text, source_code) {
            self.emit_line(format!(
                "Text does not appear to be in {} language",
                config.source_language
            ));
            self.maybe_print_source_prompt(config);
            return Ok(());
        }

        match self
            .translate_text_internal(text, source_code, target_code)
            .await
        {
            Ok(translated_text) => {
                // Print colored translation label
                let trans_label = format!("[{}]: ", config.target_language);
                self.emit_line(format!(
                    "{}{}\n",
                    config::colorize(&trans_label, &config.target_prompt_color),
                    translated_text
                ));

                if let Err(e) = self.copy_to_clipboard_if_enabled(&translated_text, config) {
                    self.emit_line(format!("Translation clipboard write error: {}", e));
                }

                // Save translation to history
                if let Err(e) = config::save_translation_history(
                    text,
                    &translated_text,
                    source_code,
                    target_code,
                    config,
                ) {
                    self.emit_line(format!("History save error: {}", e));
                }

                // Show source language prompt after hotkey translation
                self.maybe_print_source_prompt(config);
            }
            Err(e) => {
                self.emit_line(format!("Translation error: {}", e));
                self.maybe_print_source_prompt(config);
            }
        }

        Ok(())
    }

    /// Public method to get dictionary entry.
    /// Returns `(formatted_entry, corrected_word)` where `corrected_word` is `Some` when
    /// the provider detected a spelling error and used a corrected word for the lookup.
    pub async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<(String, Option<String>), Box<dyn Error + Send + Sync>> {
        // Run regular translation and dictionary lookup concurrently
        let (translation_result, dict_result) = tokio::join!(
            self.translate_text_internal(word, from, to),
            self.provider.get_dictionary_entry(word, from, to)
        );

        let primary_translation = translation_result.ok();

        match dict_result? {
            Some(entry) => {
                let corrected_word = entry.corrected_word.clone();
                let formatted =
                    self.format_dictionary_entry(&entry, to, true, primary_translation.as_deref());
                Ok((formatted, corrected_word))
            }
            None => Err("Limited dictionary information available".into()),
        }
    }

    /// Returns a localized notice to show when a spelling correction was applied.
    pub fn correction_notice(corrected_word: &str, target_lang: &str) -> String {
        let phrase = match target_lang {
            "ru" => "Показан перевод слова",
            "es" => "Mostrando traducción de la palabra",
            "fr" => "Traduction affichée pour le mot",
            "de" => "Übersetzung angezeigt für das Wort",
            "it" => "Traduzione mostrata per la parola",
            "pt" => "Tradução mostrada para a palavra",
            "zh" => "显示单词翻译",
            _ => "Showing translation for word",
        };
        format!("{} {}", phrase, corrected_word)
    }

    /// Public method to translate text
    pub async fn translate_text_public(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.translate_text_internal(text, from, to).await
    }

    /// Format dictionary entry into string
    /// cli_mode: true for CLI/terminal (no word header), false for GUI (with word header)
    /// primary_translation: result from regular translate API, used as header in terminal mode
    fn format_dictionary_entry(
        &self,
        entry: &tagent::providers::DictionaryEntry,
        target_lang: &str,
        cli_mode: bool,
        primary_translation: Option<&str>,
    ) -> String {
        let mut result = Vec::new();

        // Add the original word at the beginning (only for GUI mode)
        if !cli_mode {
            result.push(entry.word.clone());
        }

        // In terminal mode use primary translation (from translate API) as the header,
        // falling back to the first dictionary definition if translation unavailable
        if cli_mode {
            let header = primary_translation.map(|s| s.to_string()).or_else(|| {
                entry
                    .definitions
                    .first()
                    .and_then(|pos| pos.definitions.first())
                    .map(|def| def.text.clone())
            });
            if let Some(h) = header {
                result.push(h);
            }
        }

        // Format each part of speech entry
        for pos_entry in &entry.definitions {
            let pos_full = self.get_full_part_of_speech(&pos_entry.part_of_speech, target_lang);
            result.push(pos_full.to_string());

            // Format definitions with synonyms
            for def in &pos_entry.definitions {
                if !def.synonyms.is_empty() {
                    result.push(format!("  {} [{}]", def.text, def.synonyms.join(", ")));
                } else {
                    result.push(format!("  {}", def.text));
                }
            }
        }

        result.join("\n")
    }

    /// Get full part of speech name in target language
    fn get_full_part_of_speech(&self, pos: &str, target_lang: &str) -> &'static str {
        let pos_lower = pos.to_lowercase();

        match target_lang {
            "ru" => match pos_lower.as_str() {
                "noun" | "существительное" => "Существительное",
                "verb" | "глагол" => "Глагол",
                "adjective" | "прилагательное" => "Прилагательное",
                "adverb" | "наречие" => "Наречие",
                "preposition" | "предлог" => "Предлог",
                "conjunction" | "союз" => "Союз",
                "pronoun" | "местоимение" => "Местоимение",
                "interjection" | "междометие" => "Междометие",
                "article" | "артикль" => "Артикль",
                "determiner" | "определитель" => "Определитель",
                "participle" | "причастие" => "Причастие",
                _ => "Прочее",
            },
            "es" => match pos_lower.as_str() {
                "noun" => "Sustantivo",
                "verb" => "Verbo",
                "adjective" => "Adjetivo",
                "adverb" => "Adverbio",
                "preposition" => "Preposición",
                "conjunction" => "Conjunción",
                "pronoun" => "Pronombre",
                "interjection" => "Interjección",
                "article" => "Artículo",
                "determiner" => "Determinante",
                "participle" => "Participio",
                _ => "Otro",
            },
            "fr" => match pos_lower.as_str() {
                "noun" => "Nom",
                "verb" => "Verbe",
                "adjective" => "Adjectif",
                "adverb" => "Adverbe",
                "preposition" => "Préposition",
                "conjunction" => "Conjonction",
                "pronoun" => "Pronom",
                "interjection" => "Interjection",
                "article" => "Article",
                "determiner" => "Déterminant",
                "participle" => "Participe",
                _ => "Autre",
            },
            "de" => match pos_lower.as_str() {
                "noun" => "Substantiv",
                "verb" => "Verb",
                "adjective" => "Adjektiv",
                "adverb" => "Adverb",
                "preposition" => "Präposition",
                "conjunction" => "Konjunktion",
                "pronoun" => "Pronomen",
                "interjection" => "Interjektion",
                "article" => "Artikel",
                "determiner" => "Bestimmungswort",
                "participle" => "Partizip",
                _ => "Andere",
            },
            "it" => match pos_lower.as_str() {
                "noun" => "Sostantivo",
                "verb" => "Verbo",
                "adjective" => "Aggettivo",
                "adverb" => "Avverbio",
                "preposition" => "Preposizione",
                "conjunction" => "Congiunzione",
                "pronoun" => "Pronome",
                "interjection" => "Interiezione",
                "article" => "Articolo",
                "determiner" => "Determinante",
                "participle" => "Participio",
                _ => "Altro",
            },
            "pt" => match pos_lower.as_str() {
                "noun" => "Substantivo",
                "verb" => "Verbo",
                "adjective" => "Adjetivo",
                "adverb" => "Advérbio",
                "preposition" => "Preposição",
                "conjunction" => "Conjunção",
                "pronoun" => "Pronome",
                "interjection" => "Interjeição",
                "article" => "Artigo",
                "determiner" => "Determinante",
                "participle" => "Particípio",
                _ => "Outro",
            },
            "zh" => match pos_lower.as_str() {
                "noun" => "名词",
                "verb" => "动词",
                "adjective" => "形容词",
                "adverb" => "副词",
                "preposition" => "介词",
                "conjunction" => "连词",
                "pronoun" => "代词",
                "interjection" => "感叹词",
                "article" => "冠词",
                "determiner" => "限定词",
                "participle" => "分词",
                _ => "其他",
            },
            // English fallback (default)
            _ => match pos_lower.as_str() {
                "noun" | "существительное" => "Noun",
                "verb" | "глагол" => "Verb",
                "adjective" | "прилагательное" => "Adjective",
                "adverb" | "наречие" => "Adverb",
                "preposition" | "предлог" => "Preposition",
                "conjunction" | "союз" => "Conjunction",
                "pronoun" | "местоимение" => "Pronoun",
                "interjection" | "междометие" => "Interjection",
                "article" | "артикль" => "Article",
                "determiner" | "определитель" => "Determiner",
                "participle" | "причастие" => "Participle",
                _ => "Other",
            },
        }
    }

    /// Hide terminal window and restore previously active window
    /// Delays hiding if mouse cursor is over the terminal
    async fn hide_terminal_and_restore(&self, delay_seconds: u64) {
        let Some(wm) = &self.window_manager else {
            return;
        };

        // Wait specified time to let user see the result
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

        // Check if mouse is over terminal, and wait until it moves away
        loop {
            if !wm.is_mouse_over_terminal() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        // Restore the previously active window
        if let Ok(stored) = self.stored_foreground_window.lock() {
            if let Some(prev_window) = *stored {
                if let Err(e) = wm.set_foreground_window(prev_window) {
                    self.emit_line(format!("Failed to restore previous window: {}", e));
                }
            }
        }

        // Hide the terminal
        if let Err(e) = wm.hide_terminal() {
            self.emit_line(format!("Failed to hide terminal: {}", e));
        }
    }

    /// Check if text appears to be in expected language
    fn is_expected_language(&self, text: &str, language_code: &str) -> bool {
        match language_code {
            "en" => self.is_english_text(text),
            "ru" => self.is_russian_text(text),
            _ => true,
        }
    }

    /// Check if text contains English characters
    fn is_english_text(&self, text: &str) -> bool {
        let english_chars = text.chars().filter(|c| c.is_alphabetic()).count();
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();

        if total_chars == 0 {
            return false;
        }

        let english_ratio = english_chars as f64 / total_chars as f64;
        english_ratio > 0.7 && text.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Check if text contains Russian characters
    fn is_russian_text(&self, text: &str) -> bool {
        let russian_chars = text
            .chars()
            .filter(|c| c.is_alphabetic() && (*c as u32) >= 0x0400 && (*c as u32) <= 0x04FF)
            .count();

        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();

        if total_chars == 0 {
            return false;
        }

        let russian_ratio = russian_chars as f64 / total_chars as f64;
        russian_ratio > 0.3
    }

    /// Translate text using translation provider
    async fn translate_text_internal(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok(self.provider.translate_text(text, from, to).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tagent::providers::DictionaryEntry;

    struct MockProvider {
        translation: String,
    }

    #[async_trait::async_trait]
    impl TranslationProvider for MockProvider {
        async fn translate_text(
            &self,
            _text: &str,
            _from: &str,
            _to: &str,
        ) -> Result<String, tagent::error::Error> {
            Ok(self.translation.clone())
        }

        async fn get_dictionary_entry(
            &self,
            _word: &str,
            _from: &str,
            _to: &str,
        ) -> Result<Option<DictionaryEntry>, tagent::error::Error> {
            Ok(None)
        }

        async fn detect_language(&self, _text: &str) -> Result<String, tagent::error::Error> {
            Ok("en".to_string())
        }

        fn split_for_speech(&self, text: &str) -> Vec<String> {
            vec![text.to_string()]
        }

        async fn speak_chunk(
            &self,
            _text: &str,
            _lang: &str,
        ) -> Result<Vec<u8>, tagent::error::Error> {
            Ok(Vec::new())
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_correction_notice_russian() {
        let notice = Translator::correction_notice("violent", "ru");
        assert_eq!(notice, "Показан перевод слова violent");
    }

    #[test]
    fn test_correction_notice_english() {
        let notice = Translator::correction_notice("violent", "en");
        assert_eq!(notice, "Showing translation for word violent");
    }

    #[derive(Clone, Default)]
    struct MockPrinter {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl ExternalPrinter for MockPrinter {
        fn print(&mut self, msg: String) -> rustyline::Result<()> {
            self.messages.lock().unwrap().push(msg);
            Ok(())
        }
    }

    fn test_config_manager(unique: &str) -> Arc<ConfigManager> {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_translator_{}_{}.conf",
            unique,
            std::process::id()
        ));
        std::fs::write(
            &path,
            "[Translation]\nSourceLanguage = Auto\nTargetLanguage = Russian\n\
             [Interface]\nCopyToClipboard = false\n\
             [History]\nSaveTranslationHistory = false\n",
        )
        .unwrap();
        let manager = Arc::new(ConfigManager::new(path.to_str().unwrap()).unwrap());
        let _ = std::fs::remove_file(&path);
        manager
    }

    /// Regression test for a bug where hotkey-triggered translations showed the
    /// `[Auto]: ` prompt label and the translated text on separate lines.
    ///
    /// Root cause: `perform_translation` used to call `self.emit(&label)` and
    /// `self.emit_line(&text)` as two separate calls, which became two separate
    /// `ExternalPrinter::print()` invocations. rustyline's `State::external_print`
    /// unconditionally appends a newline to any message that doesn't already end
    /// with one, so the bare label (no trailing `\n`) was always forced onto its
    /// own line. The fix combines the label and the text into a single
    /// `emit_line` call so they travel through exactly one `print()` invocation.
    #[tokio::test]
    async fn hotkey_translation_emits_label_and_text_in_one_printer_call() {
        let config_manager = test_config_manager("label_line");
        let provider: Arc<dyn TranslationProvider> = Arc::new(MockProvider {
            translation: "Добавлена постоянная дедуплицированная история ввода".to_string(),
        });
        let translator = Translator {
            provider,
            clipboard: ClipboardManager::new(),
            config_manager: config_manager.clone(),
            window_manager: None,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
        };

        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);

        let config = config_manager.get_config();
        let source_text = "Added persistent, deduplicated input history";
        translator
            .perform_translation(source_text, "auto", "ru", &config)
            .await
            .unwrap();

        let messages = captured.lock().unwrap();

        let source_line = messages
            .iter()
            .find(|m| m.contains(source_text))
            .unwrap_or_else(|| panic!("no message contained the source text: {:?}", *messages));
        assert!(
            source_line.starts_with("[Auto]: "),
            "label and source text must be emitted together, got: {:?}",
            source_line
        );

        let target_line = messages
            .iter()
            .find(|m| m.contains(&config.target_language) && m.contains("дедуплицированная"))
            .unwrap_or_else(|| panic!("no message contained the translated text: {:?}", *messages));
        assert!(
            target_line.starts_with(&format!("[{}]: ", config.target_language)),
            "label and translated text must be emitted together, got: {:?}",
            target_line
        );

        // No message should ever be just a bare "[...]: " label with nothing after it.
        assert!(
            !messages.iter().any(|m| {
                let trimmed = m.trim_end_matches('\n');
                trimmed.ends_with(": ") && trimmed.len() <= "[Russian]: ".len()
            }),
            "a label was emitted as its own print() call, split from its content: {:?}",
            *messages
        );
    }
}
