use std::sync::atomic::{AtomicBool, Ordering};

/// Shared exit flag that can be set by the signal handler
static CTRL_C_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Set up Linux-specific signal handling (Ctrl+C)
pub fn setup() -> Result<(), Box<dyn std::error::Error>> {
    ctrlc::set_handler(move || {
        // If Ctrl+C was already received once, force exit
        if CTRL_C_RECEIVED.load(Ordering::Relaxed) {
            std::process::exit(1);
        }
        CTRL_C_RECEIVED.store(true, Ordering::Relaxed);
        // First Ctrl+C: allow graceful shutdown via the normal exit path
        // The interactive mode reads stdin and will detect EOF or the exit flag
    })?;
    Ok(())
}

/// Check if Ctrl+C was received
#[allow(dead_code)]
pub fn was_interrupted() -> bool {
    CTRL_C_RECEIVED.load(Ordering::Relaxed)
}
