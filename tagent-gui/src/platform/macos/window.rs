//! Cursor position and foreground-window focus save/restore for the Stage 6
//! hotkey popup. Stub on macOS: since the global hotkey itself
//! (`platform::macos::keyboard::KeyboardHook`) never fires, this module's
//! functions are never actually called, but they exist with the same
//! signatures as Linux/Windows so `main.rs`'s hotkey-wiring code stays
//! OS-independent — same rationale as the macOS stub in `keycodes.rs`.

use std::error::Error;

/// Opaque handle to a previously-focused window. Never constructed on macOS.
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(());

/// Always `None` on macOS: not yet implemented.
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

/// Always `None` on macOS: not yet implemented (a remembered popup position is
/// then applied as-is, unclamped).
pub fn virtual_screen_bounds() -> Option<(i32, i32, i32, i32)> {
    None
}

/// Always `None` on macOS: not yet implemented.
pub fn foreground_window() -> Option<WindowHandle> {
    None
}

/// Always a no-op `Ok(())` on macOS: not yet implemented.
pub fn set_foreground_window(_handle: WindowHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
}
