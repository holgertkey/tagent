use crate::clipboard::ClipboardManager;
use crate::config::ConfigManager;
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
}

impl Translator {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let config_manager = Arc::new(ConfigManager::new("translator.conf")?);
        
        Ok(Self {
            client: Client::new(),
            clipboard: ClipboardManager::new(),
            config_manager,
        })
    }

    /// Main function for translating text from clipboard
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error>> {
        // Check if config file was modified and reload if necessary
        if let Err(e) = self.config_manager.check_and_reload() {
            println!("Config reload error: {}", e);
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

        println!("\n--- Translating text ---");
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();
        
        // Show source language info
        let source_display = if source_code == "auto" {
            "Auto-detect".to_string()
        } else {
            config.source_language.clone()
        };
        
        println!("Source ({}): {}", source_display, original_text);

        // If source language is not Auto, check if text matches expected language
        if source_code != "auto" && !self.is_expected_language(&original_text, &source_code) {
            println!("Text does not appear to be in {} language", config.source_language);
            return Ok(());
        }

        match self.translate_text(&original_text, &source_code, &target_code).await {
            Ok(translated_text) => {
                println!("Target ({}): {}", config.target_language, translated_text);
                
                if let Err(e) = self.clipboard.set_text(&translated_text) {
                    println!("Translation clipboard write error: {}", e);
                } else {
                    // println!("Translation copied to clipboard successfully!");
                }
            }
            Err(e) => {
                println!("Translation error: {}", e);
            }
        }

        Ok(())
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