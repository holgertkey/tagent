use reqwest::Client;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::io::Cursor;
use std::time::Duration;

const TTS_API_URL: &str = "http://translate.google.com/translate_tts";
const MAX_TEXT_LENGTH: usize = 100;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

#[derive(Debug)]
pub enum SpeechError {
    NetworkError(String),
    AudioError(String),
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
            .split(|c| c == '.' || c == '!' || c == '?')
            .filter(|s| !s.trim().is_empty())
            .collect();

        for sentence in sentences {
            let sentence = sentence.trim();

            // If single sentence is too long, split by words
            if sentence.len() > MAX_TEXT_LENGTH {
                let words: Vec<&str> = sentence.split_whitespace().collect();
                for word in words {
                    if current_chunk.len() + word.len() + 1 > MAX_TEXT_LENGTH {
                        if !current_chunk.is_empty() {
                            chunks.push(current_chunk.clone());
                            current_chunk.clear();
                        }
                    }
                    if !current_chunk.is_empty() {
                        current_chunk.push(' ');
                    }
                    current_chunk.push_str(word);
                }
            } else {
                // Check if adding this sentence would exceed limit
                if current_chunk.len() + sentence.len() + 2 > MAX_TEXT_LENGTH {
                    if !current_chunk.is_empty() {
                        chunks.push(current_chunk.clone());
                        current_chunk.clear();
                    }
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
                let end = std::cmp::min(start + MAX_TEXT_LENGTH, text.len());
                chunks.push(text[start..end].to_string());
                start = end;
            }
        }

        chunks
    }

    /// Speak text using Google Translate TTS
    pub async fn speak_text(&self, text: &str, lang_code: &str) -> Result<(), SpeechError> {
        if text.trim().is_empty() {
            return Err(SpeechError::TextTooLong("Text is empty".to_string()));
        }

        // Split text into chunks if needed
        let chunks = if text.len() > MAX_TEXT_LENGTH {
            self.split_text_for_tts(text)
        } else {
            vec![text.to_string()]
        };

        println!("Speaking {} chunks of text...", chunks.len());

        // Play each chunk sequentially
        for (i, chunk) in chunks.iter().enumerate() {
            if chunk.trim().is_empty() {
                continue;
            }

            println!("Chunk {}/{}: {} chars", i + 1, chunks.len(), chunk.len());

            // Fetch audio for this chunk
            let audio_bytes = self.fetch_tts_audio(chunk, lang_code).await?;

            // Create audio output stream for each chunk
            let builder = OutputStreamBuilder::from_default_device()
                .map_err(|e| SpeechError::AudioError(format!("Failed to get default device: {}", e)))?;

            let stream_handle = builder.open_stream()
                .map_err(|e| SpeechError::AudioError(format!("Failed to open stream: {}", e)))?;

            // Create sink for playback
            let sink = Sink::connect_new(stream_handle.mixer());

            // Decode MP3 and play
            let cursor = Cursor::new(audio_bytes);
            let source = Decoder::new(cursor)
                .map_err(|e| SpeechError::AudioError(format!("Failed to decode MP3: {}", e)))?;

            sink.append(source);
            sink.sleep_until_end();
        }

        println!("Speech completed.");
        Ok(())
    }

    /// Speak text in a separate thread to avoid blocking
    pub fn speak_text_async(text: String, lang_code: String) {
        tokio::spawn(async move {
            let manager = SpeechManager::new();
            match manager.speak_text(&text, &lang_code).await {
                Ok(_) => println!("Speech completed successfully."),
                Err(e) => eprintln!("Speech error: {}", e),
            }
        });
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
}
