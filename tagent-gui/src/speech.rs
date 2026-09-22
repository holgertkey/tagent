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
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tagent::providers::SpeechProvider;

/// Asks the current playback, if any, to stop: sets the stop flag stored in
/// `current` (the one shared "who's speaking" slot). A no-op when nothing is
/// speaking (`None`). Returns whether there was playback to stop.
///
/// Every stop path shares this: a click on the active speaker button, Escape
/// seen by the global keyboard hook, and Escape pressed inside the main window.
/// The hook calls it on its own thread, so it must stay a quick lock-and-store.
pub fn request_stop(current: &Mutex<Option<Arc<AtomicBool>>>) -> bool {
    match current.lock().unwrap().as_ref() {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Speaks `text` in `lang_code` through the default audio output device,
/// chunked via [`SpeechProvider::split_for_speech`]. Checked against
/// `stop_flag` between chunks and while waiting for playback to finish, same
/// granularity as `tagent-cli`'s own `speak_text_with_cancel`.
pub async fn speak(
    provider: &dyn SpeechProvider,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_stop_sets_the_active_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        let current = Mutex::new(Some(flag.clone()));
        assert!(request_stop(&current));
        assert!(flag.load(Ordering::Relaxed));
        // The slot itself is left alone -- the speaking thread clears it when done.
        assert!(current.lock().unwrap().is_some());
    }

    #[test]
    fn request_stop_is_a_no_op_when_nothing_is_speaking() {
        let current = Mutex::new(None);
        assert!(!request_stop(&current));
        assert!(current.lock().unwrap().is_none());
    }

    #[test]
    fn request_stop_is_idempotent() {
        // Escape can reach both the window handler and the global hook for one
        // key press, so a second stop must be harmless.
        let flag = Arc::new(AtomicBool::new(false));
        let current = Mutex::new(Some(flag.clone()));
        assert!(request_stop(&current));
        assert!(request_stop(&current));
        assert!(flag.load(Ordering::Relaxed));
    }
}
