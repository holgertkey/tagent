use crate::config::ConfigManager;
use crate::providers::create_provider;
use colored::Colorize;
use reqwest::Client;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::io::Cursor;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::platform::keycodes;

const TTS_API_URL: &str = "https://translate.google.com/translate_tts";
const MAX_TEXT_LENGTH: usize = 100;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

/// Rounds `index` down to the nearest UTF-8 character boundary in `s`.
///
/// Clamps to `s.len()` when `index` is out of range, so callers can pass an
/// unclamped `start + MAX_TEXT_LENGTH` directly.
fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Errors that can occur during text-to-speech playback.
#[derive(Debug)]
pub enum SpeechError {
    /// A network or HTTP error prevented the audio from being fetched.
    NetworkError(String),
    /// The audio data could not be decoded or played back.
    AudioError(String),
    /// The input text exceeded the maximum allowed length for TTS.
    TextTooLong(String),
}

impl std::fmt::Display for SpeechError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeechError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            SpeechError::AudioError(msg) => write!(f, "Audio playback error: {}", msg),
            SpeechError::TextTooLong(msg) => write!(f, "Text too long: {}", msg),
        }
    }
}

impl std::error::Error for SpeechError {}

/// Plays back text as speech using Google TTS.
///
/// Text is limited to 100 characters per request. For longer input,
/// split the text into smaller chunks before calling `speak`.
pub struct SpeechManager {
    client: Client,
}

impl SpeechManager {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client for speech"),
        }
    }

    /// Fetch TTS audio from Google Translate API
    async fn fetch_tts_audio(&self, text: &str, lang_code: &str) -> Result<Vec<u8>, SpeechError> {
        if text.is_empty() {
            return Err(SpeechError::TextTooLong("Text is empty".to_string()));
        }

        if text.len() > MAX_TEXT_LENGTH {
            return Err(SpeechError::TextTooLong(format!(
                "Text is too long ({} chars). Maximum is {} chars",
                text.len(),
                MAX_TEXT_LENGTH
            )));
        }

        let url = format!(
            "{}?ie=UTF-8&client=tw-ob&q={}&tl={}",
            TTS_API_URL,
            urlencoding::encode(text),
            lang_code
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| SpeechError::NetworkError(format!("Failed to fetch TTS audio: {}", e)))?;

        if !response.status().is_success() {
            return Err(SpeechError::NetworkError(format!(
                "Google TTS API returned status: {}",
                response.status()
            )));
        }

        let audio_bytes = response
            .bytes()
            .await
            .map_err(|e| SpeechError::NetworkError(format!("Failed to read audio data: {}", e)))?;

        Ok(audio_bytes.to_vec())
    }

    /// Split text into chunks suitable for TTS (max 100 chars)
    fn split_text_for_tts(&self, text: &str) -> Vec<String> {
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
            if sentence.len() > MAX_TEXT_LENGTH {
                let words: Vec<&str> = sentence.split_whitespace().collect();
                for word in words {
                    // If word itself is too long, split it by character chunks
                    if word.len() > MAX_TEXT_LENGTH {
                        let mut word_start = 0;
                        while word_start < word.len() {
                            // A UTF-8 char is at most 4 bytes, so floor_char_boundary can only
                            // roll back a few bytes from word_start + MAX_TEXT_LENGTH (100),
                            // guaranteeing forward progress.
                            let word_end = floor_char_boundary(word, word_start + MAX_TEXT_LENGTH);
                            debug_assert!(word_end > word_start);
                            let word_chunk = &word[word_start..word_end];

                            if current_chunk.len() + word_chunk.len() + 1 > MAX_TEXT_LENGTH
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
                        if current_chunk.len() + word.len() + 1 > MAX_TEXT_LENGTH
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
                if current_chunk.len() + sentence.len() + 2 > MAX_TEXT_LENGTH
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
                // 3 bytes back from start + MAX_TEXT_LENGTH.
                let end = floor_char_boundary(text, start + MAX_TEXT_LENGTH);
                debug_assert!(end > start);
                chunks.push(text[start..end].to_string());
                start = end;
            }
        }

        chunks
    }

    /// Speak text with cancellation support
    pub async fn speak_text_with_cancel(
        &self,
        text: &str,
        lang_code: &str,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<(), SpeechError> {
        if text.trim().is_empty() {
            return Err(SpeechError::TextTooLong("Text is empty".to_string()));
        }

        // Split text into chunks if needed
        let chunks = if text.len() > MAX_TEXT_LENGTH {
            self.split_text_for_tts(text)
        } else {
            vec![text.to_string()]
        };

        // Create audio output stream once for all chunks
        let builder = OutputStreamBuilder::from_default_device()
            .map_err(|e| SpeechError::AudioError(format!("Failed to get default device: {}", e)))?;

        let mut stream_handle = builder
            .open_stream()
            .map_err(|e| SpeechError::AudioError(format!("Failed to open stream: {}", e)))?;

        // Disable "Dropping OutputStream" warning message on drop
        stream_handle.log_on_drop(false);

        // Create sink for playback
        let sink = Sink::connect_new(stream_handle.mixer());

        // Play each chunk sequentially
        for chunk in chunks.iter() {
            // Check if speech should be stopped
            if stop_flag.load(Ordering::Relaxed) {
                sink.stop();
                return Ok(());
            }

            if chunk.trim().is_empty() {
                continue;
            }

            // Fetch audio for this chunk
            let audio_bytes = self.fetch_tts_audio(chunk, lang_code).await?;

            // Decode MP3 and add to sink
            let cursor = Cursor::new(audio_bytes);
            let source = Decoder::new(cursor)
                .map_err(|e| SpeechError::AudioError(format!("Failed to decode MP3: {}", e)))?;

            sink.append(source);
        }

        // Wait for all playback to finish or stop flag
        while !sink.empty() {
            if stop_flag.load(Ordering::Relaxed) {
                sink.stop();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Ok(())
    }

    /// Detect speech language - uses auto-detection if source_code is "auto"
    pub async fn detect_speech_language(
        &self,
        text: &str,
        source_code: &str,
        provider_name: &str,
    ) -> String {
        if source_code == "auto" {
            match create_provider(provider_name) {
                Ok(provider) => match provider.detect_language(text).await {
                    Ok(detected) => detected,
                    Err(e) => {
                        eprintln!("Language detection failed: {}, using 'en'", e);
                        "en".to_string()
                    }
                },
                Err(e) => {
                    eprintln!("Failed to create provider for language detection: {}", e);
                    "en".to_string()
                }
            }
        } else {
            source_code.to_string()
        }
    }

    /// Print speech label with optional color
    pub fn print_speech_label(text: &str, label_color: Option<&str>) {
        let speech_label = "[Speech]: ";
        if let Some(color) = label_color.and_then(ConfigManager::parse_color) {
            print!("{}", speech_label.color(color));
        } else {
            print!("{}", speech_label);
        }
        println!("{}", text);
    }

    /// Speak text with Esc key monitoring for cancellation
    /// Returns true if speech was cancelled by user, false otherwise
    pub async fn speak_with_esc_monitor(
        &self,
        text: &str,
        lang_code: &str,
    ) -> Result<bool, SpeechError> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Spawn task to monitor Esc key
        let esc_monitor = tokio::spawn(async move {
            loop {
                if keycodes::is_key_pressed(keycodes::KEY_ESCAPE as i32) {
                    stop_flag_clone.store(true, Ordering::Relaxed);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        // Start speech with cancellation support
        let speech_result = self
            .speak_text_with_cancel(text, lang_code, stop_flag.clone())
            .await;

        // Cancel the Esc monitor task
        esc_monitor.abort();

        match speech_result {
            Ok(_) => {
                let was_cancelled = stop_flag.load(Ordering::Relaxed);
                if was_cancelled {
                    println!("Speech cancelled by user (Esc)");
                }
                Ok(was_cancelled)
            }
            Err(e) => Err(e),
        }
    }

    /// High-level speak function that handles language detection, label printing, and Esc monitoring
    /// This is the main entry point for speech in interactive/CLI modes
    pub async fn speak_text_full(
        &self,
        text: &str,
        config_manager: &ConfigManager,
    ) -> Result<bool, String> {
        if text.trim().is_empty() {
            return Err("Empty text provided".to_string());
        }

        config_manager.check_and_reload().ok();
        let (source_code, _) = config_manager.get_language_codes();
        let config = config_manager.get_config();

        // Detect language
        let speech_lang = self
            .detect_speech_language(text, &source_code, &config.translate_provider)
            .await;

        // Print speech label
        Self::print_speech_label(text, Some(&config.target_prompt_color));
        io::stdout().flush().ok();

        // Speak with Esc monitoring
        self.speak_with_esc_monitor(text, &speech_lang)
            .await
            .map_err(|e| format!("Speech error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_text_short() {
        let manager = SpeechManager::new();
        let text = "Hello world";
        let chunks = manager.split_text_for_tts(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello world");
    }

    #[test]
    fn test_split_text_long() {
        let manager = SpeechManager::new();
        let text = "a".repeat(250);
        let chunks = manager.split_text_for_tts(&text);
        assert!(chunks.len() >= 3);
        for chunk in chunks {
            assert!(chunk.len() <= MAX_TEXT_LENGTH);
        }
    }

    #[test]
    fn test_split_text_sentences() {
        let manager = SpeechManager::new();
        let text = "First sentence. Second sentence. Third sentence.";
        let chunks = manager.split_text_for_tts(text);
        assert!(chunks.len() >= 1);
        for chunk in chunks {
            assert!(chunk.len() <= MAX_TEXT_LENGTH);
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
        let manager = SpeechManager::new();
        // Cyrillic text longer than MAX_TEXT_LENGTH bytes with no whitespace/punctuation.
        let text = "слово".repeat(30);
        let chunks = manager.split_text_for_tts(&text);

        assert!(!chunks.is_empty());
        let mut rebuilt = String::new();
        for chunk in &chunks {
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= MAX_TEXT_LENGTH);
            assert!(std::str::from_utf8(chunk.as_bytes()).is_ok());
            rebuilt.push_str(chunk);
        }
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn test_split_text_mixed_ascii_multibyte() {
        let manager = SpeechManager::new();
        let text = "Hello мир this is тест of mixed текст content здесь and more слов to pad it out";
        let chunks = manager.split_text_for_tts(text);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= MAX_TEXT_LENGTH);
        }
    }
}
