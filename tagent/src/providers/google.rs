use super::{Definition, DictionaryEntry, PartOfSpeechEntry, TranslationProvider};
use crate::error::Error;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use url::form_urlencoded;

/// Base URL for Google's unofficial text-to-speech endpoint.
const TTS_API_URL: &str = "https://translate.google.com/translate_tts";
/// Maximum characters accepted per TTS request; longer text must be split first
/// via [`GoogleTranslateProvider::split_for_speech`].
const MAX_TTS_TEXT_LENGTH: usize = 100;
/// Shared User-Agent sent with every request to Google's translate/TTS endpoints.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

/// Rounds `index` down to the nearest UTF-8 character boundary in `s`.
///
/// Clamps to `s.len()` when `index` is out of range, so callers can pass an
/// unclamped `start + MAX_TTS_TEXT_LENGTH` directly.
fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// [`TranslationProvider`] implementation backed by the unofficial Google Translate
/// web API (`translate.googleapis.com/translate_a/single`) and Google's unofficial
/// text-to-speech endpoint (`translate.google.com/translate_tts`).
pub struct GoogleTranslateProvider {
    client: Client,
}

impl Default for GoogleTranslateProvider {
    fn default() -> Self {
        Self::new()
    }
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
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Error> {
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
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!("HTTP error: {}", response.status())));
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
                return Err(Error::Decode(
                    "failed to extract translation from response".to_string(),
                ));
            }

            Ok(result)
        } else {
            Err(Error::Decode(
                "invalid response format from Google Translate".to_string(),
            ))
        }
    }

    async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Error> {
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
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!("HTTP error: {}", response.status())));
        }

        let body = response.text().await?;
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
                .header("User-Agent", USER_AGENT)
                .send()
                .await?;

            if retry_response.status().is_success() {
                let retry_body = retry_response.text().await?;
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
                return Err(Error::Api(format!(
                    "HTTP error on retry: {}",
                    retry_response.status()
                )));
            }
        }

        Ok(None)
    }

    async fn detect_language(&self, text: &str) -> Result<String, Error> {
        let url = "https://translate.googleapis.com/translate_a/single";

        let encoded_text = form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();

        // Use auto-detect (sl=auto) and request language detection
        let params = format!("?client=gtx&sl=auto&tl=en&dt=t&q={}", encoded_text);

        let full_url = format!("{}{}", url, params);

        let response = self
            .client
            .get(&full_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!("HTTP error: {}", response.status())));
        }

        let body = response.text().await?;
        let json: Value = serde_json::from_str(&body)?;

        // Detected language is at index 2 in the response
        if let Some(detected_lang) = json.get(2).and_then(|v| v.as_str()) {
            Ok(detected_lang.to_string())
        } else {
            eprintln!("Language detection: unexpected response shape, defaulting to 'en'");
            Ok("en".to_string())
        }
    }

    fn split_for_speech(&self, text: &str) -> Vec<String> {
        // Text within the per-request limit is sent verbatim (preserving punctuation
        // exactly as given) rather than run through sentence-splitting below, which
        // trims and rejoins sentences and would otherwise alter short input.
        if text.len() <= MAX_TTS_TEXT_LENGTH {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        // Split by sentences first (by . ! ?)
        let sentences: Vec<&str> = text
            .split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();

        for sentence in sentences {
            let sentence = sentence.trim();

            // If single sentence is too long, split by words
            if sentence.len() > MAX_TTS_TEXT_LENGTH {
                let words: Vec<&str> = sentence.split_whitespace().collect();
                for word in words {
                    // If word itself is too long, split it by character chunks
                    if word.len() > MAX_TTS_TEXT_LENGTH {
                        let mut word_start = 0;
                        while word_start < word.len() {
                            // A UTF-8 char is at most 4 bytes, so floor_char_boundary can only
                            // roll back a few bytes from word_start + MAX_TTS_TEXT_LENGTH (100),
                            // guaranteeing forward progress.
                            let word_end =
                                floor_char_boundary(word, word_start + MAX_TTS_TEXT_LENGTH);
                            debug_assert!(word_end > word_start);
                            let word_chunk = &word[word_start..word_end];

                            if current_chunk.len() + word_chunk.len() + 1 > MAX_TTS_TEXT_LENGTH
                                && !current_chunk.is_empty()
                            {
                                chunks.push(current_chunk.clone());
                                current_chunk.clear();
                            }
                            if !current_chunk.is_empty() {
                                current_chunk.push(' ');
                            }
                            current_chunk.push_str(word_chunk);
                            word_start = word_end;
                        }
                    } else {
                        if current_chunk.len() + word.len() + 1 > MAX_TTS_TEXT_LENGTH
                            && !current_chunk.is_empty()
                        {
                            chunks.push(current_chunk.clone());
                            current_chunk.clear();
                        }
                        if !current_chunk.is_empty() {
                            current_chunk.push(' ');
                        }
                        current_chunk.push_str(word);
                    }
                }
            } else {
                // Check if adding this sentence would exceed limit
                if current_chunk.len() + sentence.len() + 2 > MAX_TTS_TEXT_LENGTH
                    && !current_chunk.is_empty()
                {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }

                if !current_chunk.is_empty() {
                    current_chunk.push_str(". ");
                }
                current_chunk.push_str(sentence);
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        // If no chunks created, just split by max length
        if chunks.is_empty() && !text.is_empty() {
            let mut start = 0;
            while start < text.len() {
                // Same forward-progress guarantee as above: char boundaries are at most
                // 3 bytes back from start + MAX_TTS_TEXT_LENGTH.
                let end = floor_char_boundary(text, start + MAX_TTS_TEXT_LENGTH);
                debug_assert!(end > start);
                chunks.push(text[start..end].to_string());
                start = end;
            }
        }

        chunks
    }

    async fn speak_chunk(&self, text: &str, lang: &str) -> Result<Vec<u8>, Error> {
        if text.is_empty() {
            return Err(Error::EmptyText);
        }

        if text.len() > MAX_TTS_TEXT_LENGTH {
            return Err(Error::TextTooLong {
                len: text.len(),
                max: MAX_TTS_TEXT_LENGTH,
            });
        }

        let url = format!(
            "{}?ie=UTF-8&client=tw-ob&q={}&tl={}",
            TTS_API_URL,
            urlencoding::encode(text),
            lang
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!(
                "Google TTS API returned status: {}",
                response.status()
            )));
        }

        let audio_bytes = response.bytes().await?;

        Ok(audio_bytes.to_vec())
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
            [[
                "adjective",
                null,
                [["жестокий", ["violent"], null, null, null, null, null, []]]
            ]]
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
            [[
                "adjective",
                null,
                [["жестокий", ["violent"], null, null, null, null, null, []]]
            ]]
        ]);

        let entry = provider.parse_dictionary_response(&response);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().corrected_word, None);
    }

    #[test]
    fn test_split_text_short() {
        let provider = GoogleTranslateProvider::new();
        let text = "Hello world";
        let chunks = provider.split_for_speech(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello world");
    }

    #[test]
    fn test_split_text_short_preserves_trailing_punctuation() {
        // Text within MAX_TTS_TEXT_LENGTH hits the early-return in split_for_speech and
        // must be sent verbatim, not run through sentence-splitting (which trims/rejoins
        // and would otherwise drop the period).
        let provider = GoogleTranslateProvider::new();
        let text = "Hello world.";
        let chunks = provider.split_for_speech(text);
        assert_eq!(chunks, vec!["Hello world.".to_string()]);
    }

    #[test]
    fn test_split_text_long() {
        let provider = GoogleTranslateProvider::new();
        let text = "a".repeat(250);
        let chunks = provider.split_for_speech(&text);
        assert!(chunks.len() >= 3);
        for chunk in chunks {
            assert!(chunk.len() <= MAX_TTS_TEXT_LENGTH);
        }
    }

    #[test]
    fn test_split_text_sentences() {
        let provider = GoogleTranslateProvider::new();
        // Long enough to exceed MAX_TTS_TEXT_LENGTH (100 bytes) so this actually exercises
        // sentence-splitting instead of the short-text verbatim early-return.
        let text = "First sentence is here. Second sentence follows along. \
                     Third sentence wraps it up nicely. Fourth sentence for good measure.";
        assert!(text.len() > MAX_TTS_TEXT_LENGTH);
        let chunks = provider.split_for_speech(text);
        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {:?}",
            chunks
        );
        for chunk in chunks {
            assert!(chunk.len() <= MAX_TTS_TEXT_LENGTH);
        }
    }

    #[test]
    fn test_floor_char_boundary_ascii() {
        let s = "Hello world";
        assert_eq!(floor_char_boundary(s, 5), 5);
    }

    #[test]
    fn test_floor_char_boundary_multibyte() {
        // "д" (Cyrillic) is a 2-byte character starting at byte offset 0.
        let s = "дом";
        // Index 1 is in the middle of "д" (bytes 0..2), so it must round down to 0.
        assert_eq!(floor_char_boundary(s, 1), 0);
    }

    #[test]
    fn test_floor_char_boundary_out_of_range() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 100), s.len());
    }

    #[test]
    fn test_split_text_multibyte_no_whitespace() {
        let provider = GoogleTranslateProvider::new();
        // Cyrillic text longer than MAX_TTS_TEXT_LENGTH bytes with no whitespace/punctuation.
        let text = "слово".repeat(30);
        let chunks = provider.split_for_speech(&text);

        assert!(!chunks.is_empty());
        let mut rebuilt = String::new();
        for chunk in &chunks {
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= MAX_TTS_TEXT_LENGTH);
            assert!(std::str::from_utf8(chunk.as_bytes()).is_ok());
            rebuilt.push_str(chunk);
        }
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn test_split_text_mixed_ascii_multibyte() {
        let provider = GoogleTranslateProvider::new();
        let text = "Hello мир this is тест of mixed текст content здесь and more слов \
                     to pad it out well past the single-chunk limit for this test to mean anything";
        assert!(text.len() > MAX_TTS_TEXT_LENGTH);
        let chunks = provider.split_for_speech(text);

        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {:?}",
            chunks
        );
        for chunk in &chunks {
            assert!(chunk.len() <= MAX_TTS_TEXT_LENGTH);
        }
    }

    /// Integration test: checks that both spell-correction scenarios work end-to-end.
    /// Run with: cargo test test_spell_correction_integration -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_spell_correction_integration() {
        let provider = GoogleTranslateProvider::new();

        // Scenario A: badly misspelled — no dict entries in primary response, json[7][1] has suggestion
        let result_a = provider.get_dictionary_entry("vialent", "en", "ru").await;
        println!(
            "Scenario A (vialent): {:?}",
            result_a
                .as_ref()
                .map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word)))
        );
        if let Ok(Some(entry)) = &result_a {
            assert_eq!(
                entry.corrected_word.as_deref().map(|s| s.to_lowercase()),
                Some("violent".to_string())
            );
        }

        // Scenario B: slightly misspelled — Google auto-corrects, json[0][0][1] has the correction
        let result_b = provider.get_dictionary_entry("violnt", "en", "ru").await;
        println!(
            "Scenario B (violnt): {:?}",
            result_b
                .as_ref()
                .map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word)))
        );
        if let Ok(Some(entry)) = &result_b {
            assert_eq!(
                entry.corrected_word.as_deref().map(|s| s.to_lowercase()),
                Some("violent".to_string())
            );
        }

        // Correctly spelled — corrected_word should equal the input (no notice will be shown)
        let result_c = provider.get_dictionary_entry("violent", "en", "ru").await;
        println!(
            "Scenario C (violent): {:?}",
            result_c
                .as_ref()
                .map(|e| e.as_ref().map(|x| (&x.word, &x.corrected_word)))
        );
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

        let mapped: Error = err.into();
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
        let result = provider
            .detect_language("Guten Tag, wie geht es Ihnen?")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "de");
    }

    /// Integration test: requires network access to Google Translate API.
    /// Run with: cargo test test_detect_language_french -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detect_language_french() {
        let provider = GoogleTranslateProvider::new();
        let result = provider
            .detect_language("Bonjour, comment allez-vous?")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fr");
    }

    #[test]
    fn test_speak_chunk_rejects_text_over_limit() {
        let provider = GoogleTranslateProvider::new();
        let text = "a".repeat(MAX_TTS_TEXT_LENGTH + 1);
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(provider.speak_chunk(&text, "en"));
        match result {
            Err(Error::TextTooLong { len, max }) => {
                assert_eq!(len, text.len());
                assert_eq!(max, MAX_TTS_TEXT_LENGTH);
            }
            other => panic!("expected TextTooLong, got {:?}", other),
        }
    }
}
