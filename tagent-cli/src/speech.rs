use crate::config::ConfigManager;
use crate::platform::keycodes;
use colored::Colorize;
use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::error::Error;
use std::io::Cursor;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tagent::providers::{create_provider, resolve_source_language, TranslationProvider};

/// Plays back text as speech via a [`tagent::providers::TranslationProvider`].
///
/// Text is split into provider-sized chunks internally (see
/// [`TranslationProvider::split_for_speech`]).
pub struct SpeechManager;

impl Default for SpeechManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechManager {
    /// Create a new speech manager.
    pub fn new() -> Self {
        Self
    }

    /// Speak text with cancellation support
    pub async fn speak_text_with_cancel(
        &self,
        provider: &dyn TranslationProvider,
        text: &str,
        lang_code: &str,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if text.trim().is_empty() {
            return Err("Text is empty".into());
        }

        let chunks = provider.split_for_speech(text);

        // Create audio output stream once for all chunks
        let builder = OutputStreamBuilder::from_default_device()
            .map_err(|e| format!("Failed to get default device: {}", e))?;

        let mut stream_handle = builder
            .open_stream()
            .map_err(|e| format!("Failed to open stream: {}", e))?;

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
            let audio_bytes = provider.speak_chunk(chunk, lang_code).await?;

            // Decode MP3 and add to sink
            let cursor = Cursor::new(audio_bytes);
            let source =
                Decoder::new(cursor).map_err(|e| format!("Failed to decode MP3: {}", e))?;

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
        provider: &dyn TranslationProvider,
        text: &str,
        lang_code: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
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
            .speak_text_with_cancel(provider, text, lang_code, stop_flag.clone())
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

        let provider = create_provider(&config.translate_provider)
            .map_err(|e| format!("Speech error: {}", e))?;

        // Detect language
        let speech_lang = resolve_source_language(provider.as_ref(), text, &source_code).await;

        // Print speech label
        Self::print_speech_label(text, Some(&config.target_prompt_color));
        io::stdout().flush().ok();

        // Speak with Esc monitoring
        self.speak_with_esc_monitor(provider.as_ref(), text, &speech_lang)
            .await
            .map_err(|e| format!("Speech error: {}", e))
    }
}
