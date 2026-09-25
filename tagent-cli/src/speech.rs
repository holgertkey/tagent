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
use tagent::providers::{
    create_provider, create_speech_provider, resolve_source_language, SpeechProvider,
};

/// Plays back text as speech via a [`tagent::providers::SpeechProvider`].
///
/// Text is split into provider-sized chunks internally (see
/// [`SpeechProvider::split_for_speech`]).
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
        provider: &dyn SpeechProvider,
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
    /// Resolves the language to speak `text` in, given the configured `source_code`.
    ///
    /// A concrete `source_code` is returned as-is without touching any translate
    /// provider: speech only needs one when language auto-detection (`"auto"`) is
    /// requested, so a translate provider that fails to construct never blocks speech
    /// in the common non-`"auto"` case. If construction fails for `"auto"`, falls back
    /// to `"en"` with a warning, matching [`resolve_source_language`]'s own
    /// detection-failure fallback.
    pub async fn resolve_speech_language(
        translate_provider_name: &str,
        text: &str,
        source_code: &str,
    ) -> String {
        if source_code != "auto" {
            return source_code.to_string();
        }
        match create_provider(translate_provider_name) {
            Ok(translate_provider) => {
                resolve_source_language(translate_provider.as_ref(), text, "auto").await
            }
            Err(e) => {
                eprintln!(
                    "Language detection unavailable: {}; using 'en'",
                    crate::config::provider_error_message(
                        &e,
                        "TranslateProvider",
                        tagent::providers::TRANSLATION_PROVIDERS
                    )
                );
                "en".to_string()
            }
        }
    }

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
        provider: &dyn SpeechProvider,
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
        config_manager.check_and_reload().ok();
        let (source_code, _) = config_manager.get_language_codes();
        self.speak_text_in(text, &source_code, config_manager).await
    }

    /// Like [`speak_text_full`](Self::speak_text_full), but speaks in `lang_code` instead
    /// of the configured source language. `"auto"` is resolved by language detection.
    pub async fn speak_text_in(
        &self,
        text: &str,
        lang_code: &str,
        config_manager: &ConfigManager,
    ) -> Result<bool, String> {
        if text.trim().is_empty() {
            return Err("Empty text provided".to_string());
        }

        config_manager.check_and_reload().ok();
        let config = config_manager.get_config();

        let provider = create_speech_provider(&config.speech_provider).map_err(|e| {
            format!(
                "Speech error: {}",
                crate::config::provider_error_message(
                    &e,
                    "SpeechProvider",
                    tagent::providers::SPEECH_PROVIDERS
                )
            )
        })?;

        // Detect language (constructs a translate provider only for "auto")
        let speech_lang =
            Self::resolve_speech_language(&config.translate_provider, text, lang_code).await;

        // Print speech label
        Self::print_speech_label(text, Some(&config.target_prompt_color));
        io::stdout().flush().ok();

        // Speak with Esc monitoring
        self.speak_with_esc_monitor(provider.as_ref(), text, &speech_lang)
            .await
            .map_err(|e| format!("Speech error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A translate provider name that fails to construct must not block speech when the
    // source language is concrete -- the whole point of constructing it lazily.
    #[tokio::test]
    async fn resolve_speech_language_concrete_code_never_builds_translate_provider() {
        let lang = SpeechManager::resolve_speech_language("no-such-provider", "Привет", "ru").await;
        assert_eq!(lang, "ru");
    }

    #[tokio::test]
    async fn resolve_speech_language_auto_with_unknown_translate_provider_falls_back_to_en() {
        let lang =
            SpeechManager::resolve_speech_language("no-such-provider", "Привет", "auto").await;
        assert_eq!(lang, "en");
    }
}
