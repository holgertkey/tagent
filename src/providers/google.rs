use super::{Definition, DictionaryEntry, PartOfSpeechEntry, TranslationProvider};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use std::time::Duration;
use url::form_urlencoded;

/// Turns a timed-out request into a clear message instead of raw `reqwest::Error` text.
/// reqwest's configured timeout spans the whole request (including body read), so this
/// is applied at both the `.send()` and the following `.text()` call at each request site.
fn map_request_error(e: reqwest::Error) -> Box<dyn Error + Send + Sync> {
    if e.is_timeout() {
        "Translation request timed out. Check your internet connection.".into()
    } else {
        Box::new(e)
    }
}

/// [`TranslationProvider`] implementation backed by the unofficial Google Translate
/// web API (`translate.googleapis.com/translate_a/single`).
pub struct GoogleTranslateProvider {
    client: Client,
}

impl GoogleTranslateProvider {
    /// Create a new provider with a fresh HTTP client (10s request timeout).
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client for Google Translate"),
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

            // json[0][0][1] is the actual source word Google used for translation.
            // When Google silently auto-corrects a misspelling (e.g. "violnt" → "violent"),
            // this field holds the corrected word, which differs from the original input.
            let corrected_word = json
                .get(0)
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            Some(DictionaryEntry {
                word,
                corrected_word,
                definitions,
            })
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
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
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
            .await
            .map_err(map_request_error)?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await.map_err(map_request_error)?;

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
    ) -> Result<Option<DictionaryEntry>, Box<dyn Error + Send + Sync>> {
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
            .await
            .map_err(map_request_error)?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await.map_err(map_request_error)?;
        let json: Value = serde_json::from_str(&body)?;

        // Try parsing dictionary from the primary response.
        if let Some(entry) = self.parse_dictionary_response(&json) {
            return Ok(Some(entry));
        }

        // No dictionary entries found (badly misspelled or unknown word).
        // Check json[7] for a spell-correction suggestion.
        // Structure when present: json[7] = ["<b><i>word</i></b>", "word", [flag]]
        //   json[7][1] = clean corrected word
        let suggestion = json
            .get(7)
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.get(1))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.to_lowercase() != word.to_lowercase());

        if let Some(corrected) = suggestion {
            // Retry the dictionary lookup with the corrected word.
            let encoded_corrected =
                form_urlencoded::byte_serialize(corrected.as_bytes()).collect::<String>();
            let retry_params = format!(
                "?client=gtx&sl={}&tl={}&dt=t&dt=bd&dt=ex&dt=ld&dt=md&dt=qca&dt=rw&dt=rm&dt=ss&q={}",
                from_param, to, encoded_corrected
            );
            let retry_url = format!("{}{}", url, retry_params);

            let retry_response = self
                .client
                .get(&retry_url)
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .send()
                .await
                .map_err(map_request_error)?;

            if retry_response.status().is_success() {
                let retry_body = retry_response.text().await.map_err(map_request_error)?;
                let retry_json: Value = serde_json::from_str(&retry_body)?;
                if let Some(mut entry) = self.parse_dictionary_response(&retry_json) {
                    // Override corrected_word with the explicit suggestion (more reliable
                    // than what parse_dictionary_response would extract from retry_json).
                    entry.corrected_word = Some(corrected);
                    return Ok(Some(entry));
                }
                // Retry succeeded but still no dictionary entries for the corrected
                // word — a genuine "not found", not an error.
            } else {
                return Err(format!("HTTP error on retry: {}", retry_response.status()).into());
            }
        }

        Ok(None)
    }

    async fn detect_language(&self, text: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = "https://translate.googleapis.com/translate_a/single";

        let encoded_text = form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();

        // Use auto-detect (sl=auto) and request language detection
        let params = format!("?client=gtx&sl=auto&tl=en&dt=t&q={}", encoded_text);

        let full_url = format!("{}{}", url, params);

        let response = self
            .client
            .get(&full_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(map_request_error)?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await.map_err(map_request_error)?;
        let json: Value = serde_json::from_str(&body)?;

        // Detected language is at index 2 in the response
        if let Some(detected_lang) = json.get(2).and_then(|v| v.as_str()) {
            Ok(detected_lang.to_string())
        } else {
            eprintln!("Language detection: unexpected response shape, defaulting to 'en'");
            Ok("en".to_string())
        }
    }

    fn name(&self) -> &str {
        "Google Translate"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_corrected_word_silent_correction() {
        let provider = GoogleTranslateProvider::new();
        // Scenario B: Google silently corrected "violnt" -> "violent" and returned dict entries.
        // json[0][0][1] = "violent" (the word Google actually translated).
        let response = json!([
            [["жестокий", "violent", null, null, 10]],
            [["adjective", null, [
                ["жестокий", ["violent"], null, null, null, null, null, []]
            ]]]
        ]);

        let entry = provider.parse_dictionary_response(&response);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        // corrected_word comes from json[0][0][1]
        assert_eq!(entry.corrected_word, Some("violent".to_string()));
    }

    #[test]
    fn test_parse_corrected_word_no_source_field() {
        let provider = GoogleTranslateProvider::new();
        // Response where json[0][0][1] is absent — corrected_word should be None
        let response = json!([
            [["жестокий"]],
            [["adjective", null, [
                ["жестокий", ["violent"], null, null, null, null, null, []]
            ]]]
        ]);

        let entry = provider.parse_dictionary_response(&response);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().corrected_word, None);
    }

    #[test]
    fn test_correction_notice_russian() {
        use crate::translator::Translator;
        let notice = Translator::correction_notice("violent", "ru");
        assert_eq!(notice, "Показан перевод слова violent");
    }

    #[test]
    fn test_correction_notice_english() {
        use crate::translator::Translator;
        let notice = Translator::correction_notice("violent", "en");
        assert_eq!(notice, "Showing translation for word violent");
    }

    /// Integration test: checks that both spell-correction scenarios work end-to-end.
    /// Run with: cargo test test_spell_correction_integration -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_spell_correction_integration() {
        let provider = GoogleTranslateProvider::new();

        // Scenario A: badly misspelled — no dict entries in primary response, json[7][1] has suggestion
        let result_a = provider.get_dictionary_entry("vialent", "en", "ru").await;
        println!("Scenario A (vialent): {:?}", result_a.as_ref().map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word))));
        if let Ok(Some(entry)) = &result_a {
            assert_eq!(entry.corrected_word.as_deref().map(|s| s.to_lowercase()), Some("violent".to_string()));
        }

        // Scenario B: slightly misspelled — Google auto-corrects, json[0][0][1] has the correction
        let result_b = provider.get_dictionary_entry("violnt", "en", "ru").await;
        println!("Scenario B (violnt): {:?}", result_b.as_ref().map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word))));
        if let Ok(Some(entry)) = &result_b {
            assert_eq!(entry.corrected_word.as_deref().map(|s| s.to_lowercase()), Some("violent".to_string()));
        }

        // Correctly spelled — corrected_word should equal the input (no notice will be shown)
        let result_c = provider.get_dictionary_entry("violent", "en", "ru").await;
        println!("Scenario C (violent): {:?}", result_c.as_ref().map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word))));
    }

    /// Integration test: requires network access (points at a non-routable address so the
    /// request never completes, exercising reqwest's timeout path).
    /// Run with: cargo test test_request_to_unroutable_address_times_out_with_clear_message -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_request_to_unroutable_address_times_out_with_clear_message() {
        let client = Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();

        // TEST-NET-1 (RFC 5737): reserved for documentation, guaranteed unroutable.
        let result = client.get("http://192.0.2.0/").send().await;
        let err = result.expect_err("request to a non-routable address should fail");
        assert!(err.is_timeout(), "expected a timeout error, got: {:?}", err);

        let mapped = map_request_error(err);
        assert!(mapped.to_string().contains("timed out"));
    }

    /// Integration test: requires network access to Google Translate API.
    /// Run with: cargo test test_detect_language_english -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detect_language_english() {
        let provider = GoogleTranslateProvider::new();
        let result = provider.detect_language("Hello, how are you?").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "en");
    }

    /// Integration test: requires network access to Google Translate API.
    /// Run with: cargo test test_detect_language_russian -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detect_language_russian() {
        let provider = GoogleTranslateProvider::new();
        let result = provider.detect_language("Привет, как дела?").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "ru");
    }

    /// Integration test: requires network access to Google Translate API.
    /// Run with: cargo test test_detect_language_german -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detect_language_german() {
        let provider = GoogleTranslateProvider::new();
        let result = provider.detect_language("Guten Tag, wie geht es Ihnen?").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "de");
    }

    /// Integration test: requires network access to Google Translate API.
    /// Run with: cargo test test_detect_language_french -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detect_language_french() {
        let provider = GoogleTranslateProvider::new();
        let result = provider.detect_language("Bonjour, comment allez-vous?").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fr");
    }
}
