use crate::clipboard::ClipboardManager;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use url::form_urlencoded;

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

    /// Основная функция перевода текста из буфера обмена
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error>> {
        // Получаем текст из буфера обмена
        let original_text = match self.clipboard.get_text() {
            Ok(text) => {
                if text.trim().is_empty() {
                    println!("Буфер обмена пуст или содержит только пробелы");
                    return Ok(());
                }
                text.trim().to_string()
            }
            Err(e) => {
                println!("Ошибка чтения буфера обмена: {}", e);
                return Err(e);
            }
        };

        println!("\n--- Переводим текст ---");
        println!("Исходный текст: {}", original_text);

        // Определяем, является ли текст английским
        if !self.is_english_text(&original_text) {
            println!("Текст не является английским или содержит недопустимые символы");
            return Ok(());
        }

        // Переводим текст
        match self.translate_text(&original_text, "en", "ru").await {
            Ok(translated_text) => {
                println!("Перевод: {}", translated_text);
                
                // Копируем перевод в буфер обмена
                if let Err(e) = self.clipboard.set_text(&translated_text) {
                    println!("Ошибка записи перевода в буфер обмена: {}", e);
                } else {
                    println!("Перевод скопирован в буфер обмена");
                }
            }
            Err(e) => {
                println!("Ошибка перевода: {}", e);
            }
        }

        Ok(())
    }

    /// Проверяет, содержит ли текст английские символы
    fn is_english_text(&self, text: &str) -> bool {
        let english_chars = text
            .chars()
            .filter(|c| c.is_alphabetic())
            .count();
        
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();
        
        if total_chars == 0 {
            return false;
        }

        // Если более 70% символов - латинские, считаем текст английским
        let english_ratio = english_chars as f64 / total_chars as f64;
        
        english_ratio > 0.7 && text.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Переводит текст с использованием Google Translate API
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
        // Используем бесплатный API Google Translate
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
            return Err(format!("HTTP ошибка: {}", response.status()).into());
        }

        let body = response.text().await?;
        
        // Парсим ответ JSON
        let json: Value = serde_json::from_str(&body)?;
        
        if let Some(translations) = json.get(0).and_then(|v| v.as_array()) {
            let mut result = String::new();
            
            for translation in translations {
                if let Some(text) = translation.get(0).and_then(|v| v.as_str()) {
                    result.push_str(text);
                }
            }
            
            if result.is_empty() {
                return Err("Не удалось извлечь перевод из ответа".into());
            }
            
            Ok(result)
        } else {
            Err("Неверный формат ответа от Google Translate".into())
        }
    }
}