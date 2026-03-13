use crate::config::{self, ConfigManager};
use crate::platform::{ClipboardManager, WindowHandle, WindowManager};
use crate::providers::{self, TranslationProvider};
use std::error::Error;
use std::io::{self, Write};
use std::sync::Arc;

/// High-level translation orchestrator.
///
/// `Translator` ties together a [`TranslationProvider`], the system clipboard,
/// and optional window management to provide the full Tagent translation
/// experience. For embedding in other applications, prefer [`Translator::new_cli`]
/// which skips window management.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use tagent::config::ConfigManager;
/// use tagent::translator::Translator;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///     let path = ConfigManager::get_default_config_path()?;
///     let cm = Arc::new(ConfigManager::new(path.to_str().unwrap())?);
///     let t = Translator::new_cli(cm)?;
///     t.translate_text_public("Hello world", "auto", "en").await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Translator {
    provider: Arc<dyn TranslationProvider>,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Option<Arc<WindowManager>>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<WindowHandle>>>,
}

impl Translator {
    pub fn new_with_config(config_manager: Arc<ConfigManager>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let window_manager = match WindowManager::new() {
            Ok(wm) => Some(Arc::new(wm)),
            Err(_) => {
                eprintln!("Window management unavailable (show/hide terminal and hotkeys disabled).");
                eprintln!("This is expected on Wayland or when running outside a graphical terminal.");
                None
            }
        };

        Self::build(config_manager, window_manager)
    }

    /// Create Translator without window management (for CLI mode)
    pub fn new_cli(config_manager: Arc<ConfigManager>) -> Result<Self, Box<dyn Error + Send + Sync>> {
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
        })
    }

    /// Copy text to clipboard if enabled in config
    fn copy_to_clipboard_if_enabled(
        &self,
        text: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if config.copy_to_clipboard {
            self.clipboard.set_text(text).map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })
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

    /// Print source language prompt with color
    fn print_source_prompt(config: &crate::config::Config) {
        let source_prompt = format!("[{}]: ", config.source_language);
        config::print_colored(&source_prompt, &config.source_prompt_color);
        io::stdout().flush().ok();
    }

    /// Main function for translating text from clipboard
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check if config file was modified and reload if necessary
        if let Err(e) = self.config_manager.check_and_reload() {
            println!("Config reload error: {}", e);
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
                    println!("No selected text or clipboard is empty");
                    return Ok(());
                }
                text.trim().to_string()
            }
            Err(e) => {
                println!("Copy or clipboard read error: {}", e);
                return Err(e.to_string().into());
            }
        };

        // Show terminal window if configured
        if config.show_terminal_on_translate {
            if let Some(wm) = &self.window_manager {
                if let Err(e) = wm.show_terminal() {
                    println!("Failed to show terminal: {}", e);
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
                    // Clear any existing prompt and print on new line
                    print!("\r");
                    io::stdout().flush().ok();

                    // Show the original text (source word)
                    let source_display = Self::source_display(&source_code, &config);
                    let source_label = format!("[{}]: ", source_display);
                    config::print_colored(&source_label, &config.source_prompt_color);
                    println!("{}", original_text);

                    // If a spelling correction was applied, notify the user
                    if config.spell_check {
                        if let Some(ref corrected) = corrected_word {
                            if corrected.to_lowercase() != original_text.to_lowercase() {
                                println!(
                                    "{}",
                                    Self::correction_notice(corrected, &target_code)
                                );
                            }
                        }
                    }

                    // Print colored dictionary label
                    config::print_colored("[Word]: ", &config.dictionary_prompt_color);
                    println!("{}", dictionary_info);
                    println!();

                    if let Err(e) = self.copy_to_clipboard_if_enabled(&dictionary_info, &config) {
                        println!("Dictionary clipboard write error: {}", e);
                    }

                    // Save dictionary entry to history
                    if let Err(e) = config::save_translation_history(
                        &original_text,
                        &dictionary_info,
                        &source_code,
                        &target_code,
                        &config,
                    ) {
                        println!("History save error: {}", e);
                    }

                    // Show source language prompt after hotkey translation
                    Self::print_source_prompt(&config);
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
        if config.show_terminal_on_translate && config.auto_hide_terminal_seconds > 0 && self.window_manager.is_some() {
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
        // Clear any existing prompt and move to new line
        print!("\r");
        io::stdout().flush().ok();

        // Show source language info with colored prompt
        let source_display = Self::source_display(source_code, config);
        let source_label = format!("[{}]: ", source_display);
        config::print_colored(&source_label, &config.source_prompt_color);
        println!("{}", text);

        // If source language is not Auto, check if text matches expected language
        if source_code != "auto" && !self.is_expected_language(text, source_code) {
            println!(
                "Text does not appear to be in {} language",
                config.source_language
            );
            return Ok(());
        }

        match self
            .translate_text_internal(text, source_code, target_code)
            .await
        {
            Ok(translated_text) => {
                // Print colored translation label
                let trans_label = format!("[{}]: ", config.target_language);
                config::print_colored(&trans_label, &config.target_prompt_color);
                println!("{}", translated_text);
                println!();

                if let Err(e) = self.copy_to_clipboard_if_enabled(&translated_text, config) {
                    println!("Translation clipboard write error: {}", e);
                }

                // Save translation to history
                if let Err(e) = config::save_translation_history(
                    text,
                    &translated_text,
                    source_code,
                    target_code,
                    config,
                ) {
                    println!("History save error: {}", e);
                }

                // Show source language prompt after hotkey translation
                Self::print_source_prompt(config);
            }
            Err(e) => {
                println!("Translation error: {}", e);
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
                let formatted = self.format_dictionary_entry(
                    &entry,
                    to,
                    true,
                    primary_translation.as_deref(),
                );
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
        entry: &crate::providers::DictionaryEntry,
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
            let header = primary_translation
                .map(|s| s.to_string())
                .or_else(|| {
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
                    println!("Failed to restore previous window: {}", e);
                }
            }
        }

        // Hide the terminal
        if let Err(e) = wm.hide_terminal() {
            println!("Failed to hide terminal: {}", e);
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
        self.provider.translate_text(text, from, to).await
    }
}
