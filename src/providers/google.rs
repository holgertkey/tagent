use super::{Definition, DictionaryEntry, PartOfSpeechEntry, TranslationProvider};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use url::form_urlencoded;

pub struct GoogleTranslateProvider {
    client: Client,
}

impl GoogleTranslateProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Parse Google Translate dictionary response into DictionaryEntry
    fn parse_dictionary_response(&self, json: &Value) -> Option<DictionaryEntry> {
        let mut definitions = Vec::new();

        // Dictionary definitions (at index 1)
        if let Some(dict_data) = json.get(1).and_then(|v| v.as_array()) {
            for entry in dict_data {
                if let Some(entry_array) = entry.as_array() {
                    if entry_array.len() >= 3 {
                        // Part of speech (first element)
                        if let Some(pos) = entry_array.first().and_then(|v| v.as_str()) {
                            // Detailed definitions with synonyms (third element)
                            if let Some(detailed_defs) =
                                entry_array.get(2).and_then(|v| v.as_array())
                            {
                                let mut defs = Vec::new();

                                for def in detailed_defs.iter().take(5) {
                                    // Limit to 5 definitions per part of speech
                                    if let Some(def_array) = def.as_array() {
                                        if def_array.len() >= 2 {
                                            if let Some(definition) =
                                                def_array.first().and_then(|v| v.as_str())
                                            {
                                                // Get synonyms if available
                                                let synonyms = if let Some(syn_array) =
                                                    def_array.get(1).and_then(|v| v.as_array())
                                                {
                                                    syn_array
                                                        .iter()
                                                        .filter_map(|s| s.as_str())
                                                        .map(|s| s.to_string())
                                                        .collect()
                                                } else {
                                                    Vec::new()
                                                };

                                                defs.push(Definition {
                                                    text: definition.to_string(),
                                                    synonyms,
                                                });
                                            }
                                        }
                                    }
                                }

                                if !defs.is_empty() {
                                    definitions.push(PartOfSpeechEntry {
                                        part_of_speech: pos.to_string(),
                                        definitions: defs,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if definitions.is_empty() {
            None
        } else {
            // Get the word from translation (index 0)
            let word = json
                .get(0)
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.get(0))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            Some(DictionaryEntry { word, definitions })
        }
    }
}

#[async_trait]
impl TranslationProvider for GoogleTranslateProvider {
    async fn translate_text(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error>> {
        let url = "https://translate.googleapis.com/translate_a/single";

        let encoded_text = form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();

        let from_param = if from == "auto" { "auto" } else { from };

        let params = format!(
            "?client=gtx&sl={}&tl={}&dt=t&q={}",
            from_param, to, encoded_text
        );

        let full_url = format!("{}{}", url, params);

        let response = self
            .client
            .get(&full_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
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

    async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Box<dyn Error>> {
        let url = "https://translate.googleapis.com/translate_a/single";

        let encoded_word = form_urlencoded::byte_serialize(word.as_bytes()).collect::<String>();
        let from_param = if from == "auto" { "auto" } else { from };

        // Request additional data types for dictionary information
        let params = format!(
            "?client=gtx&sl={}&tl={}&dt=t&dt=bd&dt=ex&dt=ld&dt=md&dt=qca&dt=rw&dt=rm&dt=ss&q={}",
            from_param, to, encoded_word
        );

        let full_url = format!("{}{}", url, params);

        let response = self
            .client
            .get(&full_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await?;
        let json: Value = serde_json::from_str(&body)?;

        Ok(self.parse_dictionary_response(&json))
    }

    fn name(&self) -> &str {
        "Google Translate"
    }
}
