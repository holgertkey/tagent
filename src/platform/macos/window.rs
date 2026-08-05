use std::error::Error;

/// Platform-agnostic window handle wrapper.
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(pub u64);

/// Window manager for macOS. Currently a stub: all operations are no-ops, pending
/// a real implementation (e.g. via Accessibility/AppKit APIs).
pub struct WindowManager;

impl WindowManager {
    /// Create a new window manager.
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self)
    }

    /// Show and bring the terminal window to foreground. No-op on macOS.
    pub fn show_terminal(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    /// Hide the terminal window. No-op on macOS.
    pub fn hide_terminal(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    /// Get the currently active (foreground) window. Always `None` on macOS: not yet implemented.
    pub fn get_foreground_window(&self) -> Option<WindowHandle> {
        None
    }

    /// Set the specified window as foreground. No-op on macOS.
    pub fn set_foreground_window(&self, _handle: WindowHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    /// Check if the mouse cursor is currently over the terminal window. Always `false` on macOS: not yet implemented.
    pub fn is_mouse_over_terminal(&self) -> bool {
        false
    }
}
