use crate::clipboard::ClipboardManager;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use url::form_urlencoded;

#[derive(Clone)]
pub struct Translator {
    client: Client,
    clipboard: ClipboardManager,
}

impl Translator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            clipboard: ClipboardManager::new(),
        }
    }

    /// Main function for translating text from clipboard
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error>> {
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
        println!("EN: {}", original_text);

        if !self.is_english_text(&original_text) {
            println!("Text is not English or contains invalid characters");
            return Ok(());
        }

        match self.translate_text(&original_text, "en", "ru").await {
            Ok(translated_text) => {
                println!("RU: {}", translated_text);
                
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

    /// Translate text using Google Translate API
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        let url = "https://translate.googleapis.com/translate_a/single";
        
        let encoded_text = form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
        
        let params = format!(
            "?client=gtx&sl={}&tl={}&dt=t&q={}",
            from, to, encoded_text
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