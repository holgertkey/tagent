use std::sync::atomic::{AtomicBool, Ordering};
use x11::xlib;

/// Shared flag set by the signal handler when Ctrl+C is received.
static CTRL_C_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Set up Linux-specific signal handling (Ctrl+C).
///
/// The handler does not exit the process. On Unix, rustyline's raw-mode terminal setup
/// deliberately leaves `ISIG` enabled, so every Ctrl+C still raises a real, process-wide
/// `SIGINT` that interrupts `Editor::readline()`'s blocked read with `EINTR`; rustyline turns
/// that into `ReadlineError::Interrupted`, giving `InteractiveMode::start()` a normal, in-band
/// way to react on the main thread. Exiting there (rather than force-exiting from this signal
/// handler with `std::process::exit`, which skips `Drop`) lets rustyline's terminal-mode guard
/// run and restore canonical mode before the process ends.
pub fn setup() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Must run before any other Xlib call in the process: WindowManager and
    // XGrabManager each open their own X11 Display from different threads
    // (main thread vs. the keyboard-hook task), so Xlib needs to be told to
    // lock internally before either one touches the X server.
    unsafe {
        xlib::XInitThreads();
    }

    ctrlc::set_handler(move || {
        CTRL_C_RECEIVED.store(true, Ordering::Relaxed);
    })?;
    Ok(())
}

/// Check if Ctrl+C was received
#[allow(dead_code)]
pub fn was_interrupted() -> bool {
    CTRL_C_RECEIVED.load(Ordering::Relaxed)
}
