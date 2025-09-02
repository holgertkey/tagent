use crate::clipboard::ClipboardManager;
use crate::config::ConfigManager;
use crate::window::WindowManager;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use std::sync::Arc;
use url::form_urlencoded;

#[derive(Clone)]
pub struct Translator {
    client: Client,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Arc<WindowManager>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<windows::Win32::Foundation::HWND>>>,
}

impl Translator {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let config_manager = Arc::new(ConfigManager::new("translator.conf")?);
        let window_manager = Arc::new(WindowManager::new()?);
        
        Ok(Self {
            client: Client::new(),
            clipboard: ClipboardManager::new(),
            config_manager,
            window_manager,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Check if text is a single word (no spaces, punctuation at edges allowed)
    fn is_single_word(&self, text: &str) -> bool {
        let cleaned = text.trim_matches(|c: char| !c.is_alphabetic());
        !cleaned.is_empty() && !cleaned.contains(' ') && cleaned.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
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
        if config.show_dictionary && self.is_single_word(&original_text) {
            println!("\n--- Dictionary lookup ---");
            println!("[Word]: {}", original_text);
            
            match self.get_dictionary_entry(&original_text, &source_code, &target_code).await {
                Ok(dictionary_info) => {
                    println!("{}", dictionary_info);
                    
                    if let Err(e) = self.clipboard.set_text(&dictionary_info) {
                        println!("Dictionary clipboard write error: {}", e);
                    }
                }
                Err(e) => {
                    println!("Dictionary lookup error: {}", e);
                    // Fall back to regular translation
                    self.perform_translation(&original_text, &source_code, &target_code, &config).await?;
                }
            }
        } else {
            // Regular translation for phrases or when dictionary is disabled
            self.perform_translation(&original_text, &source_code, &target_code, &config).await?;
        }

        // Hide terminal and restore previous window after delay if configured
        if config.show_terminal_on_translate && config.auto_hide_terminal_seconds > 0 {
            self.hide_terminal_and_restore(config.auto_hide_terminal_seconds).await;
        }

        Ok(())
    }

    /// Perform regular translation
    async fn perform_translation(&self, text: &str, source_code: &str, target_code: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        println!("\n--- Translating text ---");
        
        // Show source language info
        let source_display = if source_code == "auto" {
            "Auto".to_string()
        } else {
            config.source_language.clone()
        };
        
        println!("[{}]: {}", source_display, text);

        // If source language is not Auto, check if text matches expected language
        if source_code != "auto" && !self.is_expected_language(text, source_code) {
            println!("Text does not appear to be in {} language", config.source_language);
            return Ok(());
        }

        match self.translate_text(text, source_code, target_code).await {
            Ok(translated_text) => {
                println!("[{}]: {}", config.target_language, translated_text);
                
                if let Err(e) = self.clipboard.set_text(&translated_text) {
                    println!("Translation clipboard write error: {}", e);
                }
            }
            Err(e) => {
                println!("Translation error: {}", e);
            }
        }
        
        Ok(())
    }

    /// Get dictionary entry for a single word
    async fn get_dictionary_entry(&self, word: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        let url = "https://translate.googleapis.com/translate_a/single";
        
        let encoded_word = form_urlencoded::byte_serialize(word.as_bytes()).collect::<String>();
        let from_param = if from == "auto" { "auto" } else { from };
        
        // Request additional data types for dictionary information
        let params = format!(
            "?client=gtx&sl={}&tl={}&dt=t&dt=bd&dt=ex&dt=ld&dt=md&dt=qca&dt=rw&dt=rm&dt=ss&q={}",
            from_param, to, encoded_word
        );

        let full_url = format!("{}{}", url, params);

        let response = self.client
            .get(&full_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await?;
        let json: Value = serde_json::from_str(&body)?;
        
        self.format_dictionary_response(word, &json)
    }

    /// Format dictionary response into compact format
    fn format_dictionary_response(&self, _word: &str, json: &Value) -> Result<String, Box<dyn Error>> {
        let mut result = Vec::new();
        
        // Main translation
        if let Some(translations) = json.get(0).and_then(|v| v.as_array()) {
            let mut main_translation = String::new();
            for translation in translations {
                if let Some(text) = translation.get(0).and_then(|v| v.as_str()) {
                    main_translation.push_str(text);
                }
            }
            if !main_translation.is_empty() {
                result.push(format!("→ {}", main_translation));
            }
        }

        // Dictionary definitions (part of speech + definitions)
        if let Some(dict_data) = json.get(1).and_then(|v| v.as_array()) {
            for entry in dict_data {
                if let Some(entry_array) = entry.as_array() {
                    if entry_array.len() >= 2 {
                        // Part of speech
                        if let Some(pos) = entry_array.get(0).and_then(|v| v.as_str()) {
                            let pos_short = self.shorten_part_of_speech(pos);
                            
                            // Definitions
                            if let Some(definitions) = entry_array.get(1).and_then(|v| v.as_array()) {
                                let mut def_list = Vec::new();
                                for (i, def) in definitions.iter().take(3).enumerate() { // Limit to 3 definitions per part of speech
                                    if let Some(def_array) = def.as_array() {
                                        if let Some(definition) = def_array.get(0).and_then(|v| v.as_str()) {
                                            def_list.push(format!("{}. {}", i + 1, definition));
                                        }
                                    }
                                }
                                
                                if !def_list.is_empty() {
                                    result.push(format!("[{}] {}", pos_short, def_list.join("; ")));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Examples (if available)
        if let Some(examples) = json.get(2).and_then(|v| v.as_array()) {
            let mut example_list = Vec::new();
            for example in examples.iter().take(2) { // Limit to 2 examples
                if let Some(example_array) = example.as_array() {
                    if let Some(example_text) = example_array.get(0).and_then(|v| v.as_str()) {
                        example_list.push(example_text);
                    }
                }
            }
            
            if !example_list.is_empty() {
                result.push(format!("Ex: {}", example_list.join("; ")));
            }
        }

        if result.is_empty() {
            return Err("No dictionary information found".into());
        }

        Ok(result.join("\n"))
    }

    /// Shorten part of speech labels for compact display
    fn shorten_part_of_speech(&self, pos: &str) -> &'static str {
        match pos.to_lowercase().as_str() {
            "noun" => "n",
            "verb" => "v", 
            "adjective" => "adj",
            "adverb" => "adv",
            "preposition" => "prep",
            "conjunction" => "conj",
            "pronoun" => "pron",
            "interjection" => "interj",
            "article" => "art",
            "determiner" => "det",
            _ => "misc"
        }
    }

    /// Hide terminal window and restore previously active window
    async fn hide_terminal_and_restore(&self, delay_seconds: u64) {
        // Wait specified time to let user see the result
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;
        
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
            _ => true, // For other languages, assume it's correct
        }
    }

    /// Check if text contains English characters
    fn is_english_text(&self, text: &str) -> bool {
        let english_chars = text
            .chars()
            .filter(|c| c.is_alphabetic())
            .count();
        
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
        russian_ratio > 0.3 // Lower threshold for Russian as it might contain English words
    }

    /// Translate text using Google Translate API
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        let url = "https://translate.googleapis.com/translate_a/single";
        
        let encoded_text = form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
        
        let from_param = if from == "auto" { "auto" } else { from };
        
        let params = format!(
            "?client=gtx&sl={}&tl={}&dt=t&q={}",
            from_param, to, encoded_text
        );

        let full_url = format!("{}{}", url, params);

        let response = self.client
            .get(&full_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await?;
        
        let json: Value = serde_json::from_str(&body)?;
        
        if let Some(translations) = json.get(0).and_then(|v| v.as_array()) {
            let mut result = String::new();
            
            for translation in translations {
                if let Some(text) = translation.get(0).and_then(|v| v.as_str()) {
                    result.push_str(text);
                }
            }
            
            if result.is_empty() {
                return Err("Failed to extract translation from response".into());
            }
            
            Ok(result)
        } else {
            Err("Invalid response format from Google Translate".into())
        }
    }
}