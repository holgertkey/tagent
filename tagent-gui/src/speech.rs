//! Text-to-speech playback for tagent-gui.
//!
//! Ported from `tagent-cli`'s `speech.rs` (Open Question 3, `.debug/tagent-gui
//! development plan.md`) with the terminal-specific pieces stripped: no Esc-key
//! monitor (`tagent-gui`'s Linux `platform::keycodes` deliberately has no
//! `is_key_pressed` -- a global Esc poll in a windowed app would swallow Esc
//! app-wide), no `print_speech_label`, no `ConfigManager` coupling. Callers
//! resolve the provider/text/language/stop-flag themselves and drive
//! cancellation from a UI button instead.

use rodio::{Decoder, OutputStreamBuilder, Sink};
use std::error::Error;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tagent::providers::TranslationProvider;

/// Speaks `text` in `lang_code` through the default audio output device,
/// chunked via [`TranslationProvider::split_for_speech`]. Checked against
/// `stop_flag` between chunks and while waiting for playback to finish, same
/// granularity as `tagent-cli`'s own `speak_text_with_cancel`.
pub async fn speak(
    provider: &dyn TranslationProvider,
    text: &str,
    lang_code: &str,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if text.trim().is_empty() {
        return Err("Text is empty".into());
    }

    let chunks = provider.split_for_speech(text);

    let builder = OutputStreamBuilder::from_default_device()
        .map_err(|e| format!("Failed to get default device: {}", e))?;
    let mut stream_handle = builder
        .open_stream()
        .map_err(|e| format!("Failed to open stream: {}", e))?;
    // Disable "Dropping OutputStream" warning message on drop.
    stream_handle.log_on_drop(false);

    let sink = Sink::connect_new(stream_handle.mixer());

    for chunk in chunks.iter() {
        if stop_flag.load(Ordering::Relaxed) {
            sink.stop();
            return Ok(());
        }

        if chunk.trim().is_empty() {
            continue;
        }

        let audio_bytes = provider.speak_chunk(chunk, lang_code).await?;

        let cursor = Cursor::new(audio_bytes);
        let source = Decoder::new(cursor).map_err(|e| format!("Failed to decode MP3: {}", e))?;
        sink.append(source);
    }

    while !sink.empty() {
        if stop_flag.load(Ordering::Relaxed) {
            sink.stop();
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}
