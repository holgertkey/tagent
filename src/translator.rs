use crate::clipboard::ClipboardManager;
use crate::config::ConfigManager;
use crate::window::WindowManager;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use std::sync::Arc;
use url::form_urlencoded;
use std::io::{self, Write};
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;

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
        let config_manager = Arc::new(ConfigManager::new("tagent.conf")?);
        let window_manager = Arc::new(WindowManager::new()?);
        
        Ok(Self {
            client: Client::new(),
            clipboard: ClipboardManager::new(),
            config_manager,
            window_manager,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Save translation history to file in multi-line format
    fn save_translation_history(&self, original: &str, translated: &str, source_lang: &str, target_lang: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        if !config.save_translation_history {
            return Ok(()); // История отключена
        }

        let timestamp: DateTime<Utc> = Utc::now();
        let formatted_time = timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        
        let entry = format!(
            "[{}] {} -> {}\nIN:  {}\nOUT: {}\n---\n\n",
            formatted_time, source_lang, target_lang, original, translated
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.history_file)?;

        file.write_all(entry.as_bytes())?;
        file.flush()?; // Принудительно записываем на диск
        
        Ok(())
    }

    /// Check if text is a single word (no spaces, punctuation at edges allowed)
    fn is_single_word(&self, text: &str) -> bool {
        let cleaned = text.trim_matches(|c: char| !c.is_alphabetic());
        !cleaned.is_empty() && !cleaned.contains(' ') && cleaned.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
    }

    /// Copy text to clipboard if enabled in config
    fn copy_to_clipboard_if_enabled(&self, text: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        if config.copy_to_clipboard {
            self.clipboard.set_text(text)
        } else {
            Ok(())
        }
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
            match self.get_dictionary_entry(&original_text, &source_code, &target_code).await {
                Ok(dictionary_info) => {
                    // Clear any existing prompt and print on new line
                    print!("\r");
                    io::stdout().flush().ok();
                    println!("{}", dictionary_info);
                    println!(); // Add empty line after dictionary entry in GUI mode
                    
                    if let Err(e) = self.copy_to_clipboard_if_enabled(&dictionary_info, &config) {
                        println!("Dictionary clipboard write error: {}", e);
                    }

                    // Сохраняем словарную статью в историю
                    if let Err(e) = self.save_translation_history(&original_text, &dictionary_info, &source_code, &target_code, &config) {
                        println!("History save error: {}", e);
                    }
                }
                Err(_) => {
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
        // Clear any existing prompt and move to new line
        print!("\r");
        io::stdout().flush().ok();
        
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

        match self.translate_text_internal(text, source_code, target_code).await {
            Ok(translated_text) => {
                println!("[{}]: {}", config.target_language, translated_text);
                println!(); // Add empty line after translation result
                
                if let Err(e) = self.copy_to_clipboard_if_enabled(&translated_text, config) {
                    println!("Translation clipboard write error: {}", e);
                }

                // Сохраняем перевод в историю
                if let Err(e) = self.save_translation_history(text, &translated_text, source_code, target_code, config) {
                    println!("History save error: {}", e);
                }
            }
            Err(e) => {
                println!("Translation error: {}", e);
            }
        }
        
        Ok(())
    }

    /// Public method for CLI to get dictionary entry (without headers)
    pub async fn get_dictionary_entry_public(&self, word: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        self.get_dictionary_entry_cli(word, from, to).await
    }

    /// Public method for CLI to translate text
    pub async fn translate_text_public(&self, text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        self.translate_text_internal(text, from, to).await
    }

    /// Get dictionary entry for CLI (clean output)
    async fn get_dictionary_entry_cli(&self, word: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
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
        
        self.format_dictionary_response_cli(word, &json, to)
    }

    /// Get dictionary entry for a single word (GUI mode)
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
        
        self.format_dictionary_response(word, &json, to)
    }

    /// Format dictionary response for CLI (clean output without headers)
    fn format_dictionary_response_cli(&self, _word: &str, json: &Value, target_lang: &str) -> Result<String, Box<dyn Error>> {
        let mut result = Vec::new();
        
        // Don't add [Word]: header for CLI

        // Dictionary definitions (at index 1)
        if let Some(dict_data) = json.get(1).and_then(|v| v.as_array()) {
            for entry in dict_data {
                if let Some(entry_array) = entry.as_array() {
                    if entry_array.len() >= 3 {
                        // Part of speech (first element)
                        if let Some(pos) = entry_array.get(0).and_then(|v| v.as_str()) {
                            let pos_full = self.get_full_part_of_speech(pos, target_lang);
                            
                            // Detailed definitions with synonyms (third element)
                            if let Some(detailed_defs) = entry_array.get(2).and_then(|v| v.as_array()) {
                                let mut def_lines = Vec::new();
                                
                                for def in detailed_defs.iter().take(5) { // Limit to 5 definitions per part of speech
                                    if let Some(def_array) = def.as_array() {
                                        if def_array.len() >= 2 {
                                            if let Some(definition) = def_array.get(0).and_then(|v| v.as_str()) {
                                                // Get synonyms if available
                                                if let Some(synonyms) = def_array.get(1).and_then(|v| v.as_array()) {
                                                    let syn_list: Vec<String> = synonyms
                                                        .iter()
                                                        .filter_map(|s| s.as_str())
                                                        .map(|s| s.to_string())
                                                        .collect();
                                                    
                                                    if !syn_list.is_empty() {
                                                        def_lines.push(format!("  {} [{}]", definition, syn_list.join(", ")));
                                                    } else {
                                                        def_lines.push(format!("  {}", definition));
                                                    }
                                                } else {
                                                    def_lines.push(format!("  {}", definition));
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if !def_lines.is_empty() {
                                    result.push(pos_full.to_string());
                                    result.extend(def_lines);
                                }
                            }
                        }
                    }
                }
            }
        }

        if result.is_empty() {
            return Err("Limited dictionary information available".into());
        }

        Ok(result.join("\n"))
    }

    /// Format dictionary response into compact format (GUI mode)
    fn format_dictionary_response(&self, word: &str, json: &Value, target_lang: &str) -> Result<String, Box<dyn Error>> {
        let mut result = Vec::new();
        
        // Add the original word at the beginning for GUI mode
        result.push(format!("[Word]: {}", word));

        // Dictionary definitions (at index 1)
        if let Some(dict_data) = json.get(1).and_then(|v| v.as_array()) {
            for entry in dict_data {
                if let Some(entry_array) = entry.as_array() {
                    if entry_array.len() >= 3 {
                        // Part of speech (first element)
                        if let Some(pos) = entry_array.get(0).and_then(|v| v.as_str()) {
                            let pos_full = self.get_full_part_of_speech(pos, target_lang);
                            
                            // Detailed definitions with synonyms (third element)
                            if let Some(detailed_defs) = entry_array.get(2).and_then(|v| v.as_array()) {
                                let mut def_lines = Vec::new();
                                
                                for def in detailed_defs.iter().take(5) { // Limit to 5 definitions per part of speech
                                    if let Some(def_array) = def.as_array() {
                                        if def_array.len() >= 2 {
                                            if let Some(definition) = def_array.get(0).and_then(|v| v.as_str()) {
                                                // Get synonyms if available
                                                if let Some(synonyms) = def_array.get(1).and_then(|v| v.as_array()) {
                                                    let syn_list: Vec<String> = synonyms
                                                        .iter()
                                                        .filter_map(|s| s.as_str())
                                                        .map(|s| s.to_string())
                                                        .collect();
                                                    
                                                    if !syn_list.is_empty() {
                                                        def_lines.push(format!("  {} [{}]", definition, syn_list.join(", ")));
                                                    } else {
                                                        def_lines.push(format!("  {}", definition));
                                                    }
                                                } else {
                                                    def_lines.push(format!("  {}", definition));
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if !def_lines.is_empty() {
                                    result.push(pos_full.to_string());
                                    result.extend(def_lines);
                                }
                            }
                        }
                    }
                }
            }
        }

        if result.is_empty() {
            return Err("Limited dictionary information available".into());
        }

        Ok(result.join("\n"))
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
                _ => "Прочее"
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
                _ => "Otro"
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
                _ => "Autre"
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
                _ => "Andere"
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
                _ => "Altro"
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
                _ => "Outro"
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
                _ => "其他"
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
                _ => "Other"
            }
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
    async fn translate_text_internal(&self, text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
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