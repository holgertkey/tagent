use std::error::Error;

/// macOS clipboard access. Currently a stub: every operation returns an error,
/// pending a real implementation (e.g. via `NSPasteboard`).
#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    /// Create a new clipboard manager.
    pub fn new() -> Self {
        Self
    }

    /// Get text from the clipboard. Always errors on macOS: not yet implemented.
    pub fn get_text(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Err("Clipboard not yet implemented for macOS".into())
    }

    /// Set text on the clipboard. Always errors on macOS: not yet implemented.
    pub fn set_text(&self, _text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        Err("Clipboard not yet implemented for macOS".into())
    }

    /// Simulate a copy of the current text selection. Always errors on macOS: not yet implemented.
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Err("Auto-copy not yet implemented for macOS".into())
    }

    /// Copy the current selection, then read it back from the clipboard. Always errors on
    /// macOS since [`ClipboardManager::copy_selected_text`] is not yet implemented.
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}
