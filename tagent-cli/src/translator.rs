use crate::config::{self, ConfigManager};
use crate::platform::{ClipboardManager, WindowHandle, WindowManager};
use rustyline::ExternalPrinter;
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tagent::article;
use tagent::providers::{self, DictionaryProvider, TranslationProvider};

/// Shared slot for the rustyline external printer used to route hotkey-triggered
/// translation output safely while the interactive prompt may be mid-read on another
/// thread. `None` until [`InteractiveMode::start`](crate::interactive::InteractiveMode::start)
/// installs one; always `None` in CLI mode, which never calls [`Translator::translate_clipboard`].
type SharedPrinter = Arc<Mutex<Option<Box<dyn ExternalPrinter + Send>>>>;

/// The most recent successful translation, kept so `/s` and `/ss` can replay it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastTranslation {
    /// The text that was translated.
    pub phrase: String,
    /// The plain translation; for a dictionary hit, the primary translation (never the
    /// full article). `None` when a dictionary lookup succeeded but its plain translation
    /// failed.
    pub translation: Option<String>,
    /// Source language code at translation time (may be `"auto"`).
    pub source_code: String,
    /// Target language code at translation time.
    pub target_code: String,
}

/// Result of [`Translator::get_dictionary_entry`].
#[derive(Debug)]
pub struct DictionaryLookup {
    /// The dictionary article as plain text, for the clipboard and the history file.
    pub formatted: String,
    /// The same article as role-tagged lines, for highlighted display with
    /// [`config::render_article`].
    pub lines: Vec<article::Line>,
    /// Set when the provider looked up a spelling-corrected word instead.
    pub corrected_word: Option<String>,
    /// The plain translation of the word, if the translate request succeeded.
    pub primary_translation: Option<String>,
}

/// High-level translation orchestrator.
///
/// `Translator` ties together a [`TranslationProvider`] and a [`DictionaryProvider`] (from
/// the [`tagent`] library crate), the system clipboard, and optional window management to provide the full
/// Tagent translation experience. Use [`Translator::new_cli`] when window management
/// is not needed (e.g. one-off CLI translations).
#[derive(Clone)]
pub struct Translator {
    provider: Arc<dyn TranslationProvider>,
    /// `None` when the configured `DictionaryProvider` could not be created; dictionary
    /// lookups then fail immediately and callers fall back to plain translation.
    dictionary_provider: Option<Arc<dyn DictionaryProvider>>,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Option<Arc<WindowManager>>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<WindowHandle>>>,
    printer: SharedPrinter,
    /// Shared between the hotkey path and interactive mode (both hold clones of this
    /// `Translator`), so `/s`/`/ss` replay whichever translation happened last.
    last_translation: Arc<Mutex<Option<LastTranslation>>>,
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
        let provider = providers::create_provider(&config.translate_provider).map_err(|e| {
            config::provider_error_message(
                &e,
                "TranslateProvider",
                providers::TRANSLATION_PROVIDERS,
            )
        })?;

        // A bad dictionary provider must never break translation: warn once and disable
        // dictionary lookups instead of failing to start (unlike the translate provider).
        let dictionary_provider =
            match providers::create_dictionary_provider(&config.dictionary_provider) {
                Ok(dictionary) => Some(Arc::from(dictionary)),
                Err(e) => {
                    eprintln!(
                        "Dictionary provider unavailable: {}; dictionary lookups disabled",
                        config::provider_error_message(
                            &e,
                            "DictionaryProvider",
                            providers::DICTIONARY_PROVIDERS
                        )
                    );
                    None
                }
            };

        Ok(Self {
            provider: Arc::from(provider),
            dictionary_provider,
            clipboard: ClipboardManager::new(),
            config_manager,
            window_manager,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
        })
    }

    /// Install the external printer used to route [`translate_clipboard`](Self::translate_clipboard)
    /// output safely while the interactive prompt may be reading a line in raw mode.
    /// Called once, from `InteractiveMode::start()`, right after the rustyline `Editor` is built.
    pub fn set_external_printer(&self, printer: impl ExternalPrinter + Send + 'static) {
        *self.printer.lock().unwrap() = Some(Box::new(printer));
    }

    /// Remember a successful translation for `/s` and `/ss`.
    pub fn record_last_translation(
        &self,
        phrase: &str,
        translation: Option<&str>,
        source_code: &str,
        target_code: &str,
    ) {
        *self.last_translation.lock().unwrap() = Some(LastTranslation {
            phrase: phrase.to_string(),
            translation: translation.map(str::to_string),
            source_code: source_code.to_string(),
            target_code: target_code.to_string(),
        });
    }

    /// The most recent successful translation (hotkey or interactive), if any.
    pub fn last_translation(&self) -> Option<LastTranslation> {
        self.last_translation.lock().unwrap().clone()
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
                Ok(DictionaryLookup {
                    formatted: dictionary_info,
                    lines,
                    corrected_word,
                    primary_translation,
                }) => {
                    self.record_last_translation(
                        &original_text,
                        primary_translation.as_deref(),
                        &source_code,
                        &target_code,
                    );

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
                                self.emit_line(config::colorize(
                                    &Self::correction_notice(corrected, &target_code),
                                    &config.notice_color,
                                ));
                            }
                        }
                    }

                    // Print colored dictionary label
                    self.emit_line(format!(
                        "{}{}\n",
                        config::colorize("[Word]: ", &config.dictionary_prompt_color),
                        config::render_article(&lines, &config)
                    ));

                    if let Err(e) = self.copy_to_clipboard_if_enabled(&dictionary_info, &config) {
                        self.emit_line(config::colorize(
                            &format!("Dictionary clipboard write error: {}", e),
                            &config.error_color,
                        ));
                    }

                    // Save dictionary entry to history
                    if let Err(e) = config::save_translation_history(
                        &original_text,
                        &dictionary_info,
                        &source_code,
                        &target_code,
                        &config,
                    ) {
                        self.emit_line(config::colorize(
                            &format!("History save error: {}", e),
                            &config.error_color,
                        ));
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
                self.record_last_translation(
                    text,
                    Some(&translated_text),
                    source_code,
                    target_code,
                );

                // Print colored translation label
                let trans_label = format!("[{}]: ", config.target_language);
                self.emit_line(format!(
                    "{}{}\n",
                    config::colorize(&trans_label, &config.target_prompt_color),
                    translated_text
                ));

                if let Err(e) = self.copy_to_clipboard_if_enabled(&translated_text, config) {
                    self.emit_line(config::colorize(
                        &format!("Translation clipboard write error: {}", e),
                        &config.error_color,
                    ));
                }

                // Save translation to history
                if let Err(e) = config::save_translation_history(
                    text,
                    &translated_text,
                    source_code,
                    target_code,
                    config,
                ) {
                    self.emit_line(config::colorize(
                        &format!("History save error: {}", e),
                        &config.error_color,
                    ));
                }

                // Show source language prompt after hotkey translation
                self.maybe_print_source_prompt(config);
            }
            Err(e) => {
                self.emit_line(config::colorize(
                    &format!("Translation error: {}", e),
                    &config.error_color,
                ));
                self.maybe_print_source_prompt(config);
            }
        }

        Ok(())
    }

    /// Public method to get dictionary entry.
    /// See [`DictionaryLookup`] for what is returned; `corrected_word` is `Some` when
    /// the provider detected a spelling error and used a corrected word for the lookup.
    pub async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<DictionaryLookup, Box<dyn Error + Send + Sync>> {
        // No dictionary provider: fail before any network call so the caller's fallback to
        // plain translation runs exactly once.
        let dictionary_provider = self
            .dictionary_provider
            .as_ref()
            .ok_or("Dictionary provider unavailable")?;

        // Run regular translation and dictionary lookup concurrently
        let (translation_result, dict_result) = tokio::join!(
            self.translate_text_internal(word, from, to),
            dictionary_provider.lookup(word, from, to)
        );

        let primary_translation = translation_result.ok();

        match dict_result? {
            Some(entry) => {
                let corrected_word = entry.corrected_word.clone();
                let lines = article::article_lines(&entry, to, primary_translation.as_deref());
                Ok(DictionaryLookup {
                    formatted: article::to_plain(&lines),
                    lines,
                    corrected_word,
                    primary_translation,
                })
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tagent::providers::{Definition, DictionaryEntry, PartOfSpeechEntry};

    struct MockProvider {
        translation: String,
        /// Counts `translate_text` calls, to assert a code path never reached the network.
        translate_calls: Arc<AtomicUsize>,
    }

    impl MockProvider {
        fn new(translation: &str) -> Self {
            Self {
                translation: translation.to_string(),
                translate_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    /// Returns a canned entry, or a miss when `entry` is `None`.
    struct MockDictionary {
        entry: Option<DictionaryEntry>,
    }

    #[async_trait::async_trait]
    impl DictionaryProvider for MockDictionary {
        async fn lookup(
            &self,
            _word: &str,
            _from: &str,
            _to: &str,
        ) -> Result<Option<DictionaryEntry>, tagent::error::Error> {
            Ok(self.entry.clone())
        }

        fn name(&self) -> &str {
            "mock dictionary"
        }
    }

    #[async_trait::async_trait]
    impl TranslationProvider for MockProvider {
        async fn translate_text(
            &self,
            _text: &str,
            _from: &str,
            _to: &str,
        ) -> Result<String, tagent::error::Error> {
            self.translate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.translation.clone())
        }

        async fn detect_language(&self, _text: &str) -> Result<String, tagent::error::Error> {
            Ok("en".to_string())
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
        let provider: Arc<dyn TranslationProvider> = Arc::new(MockProvider::new(
            "Добавлена постоянная дедуплицированная история ввода",
        ));
        let translator = Translator {
            provider,
            dictionary_provider: None,
            clipboard: ClipboardManager::new(),
            config_manager: config_manager.clone(),
            window_manager: None,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
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

    fn translator_with(
        provider: MockProvider,
        dictionary: Option<MockDictionary>,
        unique: &str,
    ) -> Translator {
        Translator {
            provider: Arc::new(provider),
            dictionary_provider: dictionary.map(|d| Arc::new(d) as Arc<dyn DictionaryProvider>),
            clipboard: ClipboardManager::new(),
            config_manager: test_config_manager(unique),
            window_manager: None,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
        }
    }

    /// The `Translator`/`DictionaryProvider` seam had no coverage before the provider split:
    /// the old mock's `get_dictionary_entry` always returned `None`.
    #[tokio::test]
    async fn dictionary_entry_is_formatted_and_reports_corrected_word() {
        let entry = DictionaryEntry::new(
            "violnt",
            vec![PartOfSpeechEntry::new(
                "adjective",
                vec![Definition::new("жестокий", vec!["violent".to_string()])],
            )],
        )
        .with_corrected_word("violent");
        let translator = translator_with(
            MockProvider::new("насилие"),
            Some(MockDictionary { entry: Some(entry) }),
            "dict_entry",
        );

        let DictionaryLookup {
            formatted,
            lines,
            corrected_word: corrected,
            primary_translation,
        } = translator
            .get_dictionary_entry("violnt", "en", "ru")
            .await
            .unwrap();

        assert_eq!(corrected.as_deref(), Some("violent"));
        assert_eq!(primary_translation.as_deref(), Some("насилие"));
        // Terminal mode: the translate provider's result is the header line.
        assert!(formatted.starts_with("насилие"), "got: {formatted:?}");
        assert!(formatted.contains("жестокий"), "got: {formatted:?}");
        assert!(formatted.contains("[violent]"), "got: {formatted:?}");
        // The plain text (clipboard, history) and the highlighted display share one layout.
        assert_eq!(formatted, article::to_plain(&lines));
    }

    #[tokio::test]
    async fn dictionary_miss_is_an_error_so_callers_fall_back_to_translation() {
        let translator = translator_with(
            MockProvider::new("ксиззик"),
            Some(MockDictionary { entry: None }),
            "dict_miss",
        );

        let result = translator.get_dictionary_entry("xyzzyq", "en", "ru").await;

        assert!(result.is_err());
    }

    /// A bad `DictionaryProvider` must never break translation: with none available the
    /// lookup fails immediately, without touching the translate provider.
    #[tokio::test]
    async fn missing_dictionary_provider_fails_without_any_network_call() {
        let provider = MockProvider::new("не должно вызываться");
        let calls = provider.translate_calls.clone();
        let translator = translator_with(provider, None, "dict_none");

        let result = translator.get_dictionary_entry("violent", "en", "ru").await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    /// `/s` and `/ss` replay what `perform_translation` (the hotkey path) records.
    #[tokio::test]
    async fn successful_translation_is_recorded_for_speech_replay() {
        let translator = translator_with(MockProvider::new("Привет, мир"), None, "last_ok");
        translator.set_external_printer(MockPrinter::default());
        let config = translator.config_manager.get_config();

        translator
            .perform_translation("Hello, world", "auto", "ru", &config)
            .await
            .unwrap();

        assert_eq!(
            translator.last_translation(),
            Some(LastTranslation {
                phrase: "Hello, world".to_string(),
                translation: Some("Привет, мир".to_string()),
                source_code: "auto".to_string(),
                target_code: "ru".to_string(),
            })
        );
    }

    /// A rejected phrase ("Text does not appear to be in ...") must not replace the
    /// previously recorded translation.
    #[tokio::test]
    async fn rejected_translation_keeps_previous_last_translation() {
        let translator = translator_with(MockProvider::new("мир"), None, "last_rejected");
        translator.set_external_printer(MockPrinter::default());
        let config = translator.config_manager.get_config();
        translator.record_last_translation("world", Some("мир"), "en", "ru");

        translator
            .perform_translation("Привет всем", "en", "ru", &config)
            .await
            .unwrap();

        let last = translator.last_translation().unwrap();
        assert_eq!(last.phrase, "world");
        assert_eq!(last.translation.as_deref(), Some("мир"));
    }

    /// Clones share the slot, so a hotkey translation is visible to interactive mode.
    #[test]
    fn last_translation_is_shared_between_clones() {
        let translator = translator_with(MockProvider::new(""), None, "last_shared");
        let interactive_copy = translator.clone();

        translator.record_last_translation("word", None, "auto", "ru");

        let last = interactive_copy.last_translation().unwrap();
        assert_eq!(last.phrase, "word");
        assert_eq!(last.translation, None);
    }
}
