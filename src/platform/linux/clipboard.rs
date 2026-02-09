use std::error::Error;

#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error>> {
        Err("Clipboard not yet implemented for Linux".into())
    }

    /// Set text to clipboard
    pub fn set_text(&self, _text: &str) -> Result<(), Box<dyn Error>> {
        Err("Clipboard not yet implemented for Linux".into())
    }

    /// Automatically copy selected text
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error>> {
        Err("Auto-copy not yet implemented for Linux".into())
    }

    /// Get text from clipboard with automatic copying
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}
