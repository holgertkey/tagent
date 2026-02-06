use crate::clipboard::ClipboardManager;
use crate::config::{self, ConfigManager};
use crate::providers::{self, TranslationProvider};
use crate::window::WindowManager;
use std::error::Error;
use std::io::{self, Write};
use std::sync::Arc;

#[derive(Clone)]
pub struct Translator {
    provider: Arc<dyn TranslationProvider>,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Arc<WindowManager>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<windows::Win32::Foundation::HWND>>>,
}

impl Translator {
    pub fn new_with_config(config_manager: Arc<ConfigManager>) -> Result<Self, Box<dyn Error>> {
        let window_manager = Arc::new(WindowManager::new()?);

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
    ) -> Result<(), Box<dyn Error>> {
        if config.copy_to_clipboard {
            self.clipboard.set_text(text)
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
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error>> {
        // Check if config file was modified and reload if necessary
        if let Err(e) = self.config_manager.check_and_reload() {
            println!("Config reload error: {}", e);
        }

        let config = self.config_manager.get_config();

        // Store the current foreground window before any operations
        if config.show_terminal_on_translate {
            if let Some(fg_window) = self.window_manager.get_foreground_window() {
                if let Ok(mut stored) = self.stored_foreground_window.lock() {
                    *stored = Some(fg_window);
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
                return Err(e);
            }
        };

        // Show terminal window if configured
        if config.show_terminal_on_translate {
            if let Err(e) = self.window_manager.show_terminal() {
                println!("Failed to show terminal: {}", e);
            }
        }

        let (source_code, target_code) = self.config_manager.get_language_codes();

        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && config::is_single_word(&original_text) {
            match self
                .get_dictionary_entry(&original_text, &source_code, &target_code)
                .await
            {
                Ok(dictionary_info) => {
                    // Clear any existing prompt and print on new line
                    print!("\r");
                    io::stdout().flush().ok();

                    // Show the original text (source word)
                    let source_display = Self::source_display(&source_code, &config);
                    let source_label = format!("[{}]: ", source_display);
                    config::print_colored(&source_label, &config.source_prompt_color);
                    println!("{}", original_text);

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
        if config.show_terminal_on_translate && config.auto_hide_terminal_seconds > 0 {
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
    ) -> Result<(), Box<dyn Error>> {
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

    /// Public method to get dictionary entry
    pub async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error>> {
        let entry_opt = self.provider.get_dictionary_entry(word, from, to).await?;

        match entry_opt {
            Some(entry) => Ok(self.format_dictionary_entry(&entry, to, true)),
            None => Err("Limited dictionary information available".into()),
        }
    }

    /// Public method to translate text
    pub async fn translate_text_public(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error>> {
        self.translate_text_internal(text, from, to).await
    }

    /// Format dictionary entry into string
    /// cli_mode: true for CLI (no word header), false for GUI (with word header)
    fn format_dictionary_entry(
        &self,
        entry: &crate::providers::DictionaryEntry,
        target_lang: &str,
        cli_mode: bool,
    ) -> String {
        let mut result = Vec::new();

        // Add the original word at the beginning (only for GUI mode)
        if !cli_mode {
            result.push(entry.word.clone());
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
        // Wait specified time to let user see the result
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

        // Check if mouse is over terminal, and wait until it moves away
        loop {
            if !self.window_manager.is_mouse_over_terminal() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        // Restore the previously active window
        if let Ok(stored) = self.stored_foreground_window.lock() {
            if let Some(prev_window) = *stored {
                if let Err(e) = self.window_manager.set_foreground_window(prev_window) {
                    println!("Failed to restore previous window: {}", e);
                }
            }
        }

        // Hide the terminal
        if let Err(e) = self.window_manager.hide_terminal() {
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
    ) -> Result<String, Box<dyn Error>> {
        self.provider.translate_text(text, from, to).await
    }
}
