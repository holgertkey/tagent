use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;
use windows::{
    Win32::UI::Input::KeyboardAndMouse::*,
};

#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error>> {
        match get_clipboard(formats::Unicode) {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Clipboard read error: {}", e).into()),
        }
    }

    /// Set text to clipboard
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        match set_clipboard(formats::Unicode, text) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Clipboard write error: {}", e).into()),
        }
    }

    /// Automatically copy selected text (simulate Ctrl+C)
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            // Release any pressed modifiers first (Alt, Shift, Win)
            // This ensures Ctrl+C is recognized correctly when triggered by hotkeys like Alt+Space
            keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);      // Alt up
            keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);     // Shift up
            keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);      // Win up
            keybd_event(VK_RWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);      // Win up (right)

            std::thread::sleep(std::time::Duration::from_millis(10));

            // Simulate Ctrl+C
            keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            keybd_event(b'C', 0, KEYBD_EVENT_FLAGS(0), 0);
            keybd_event(b'C', 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);

            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        Ok(())
    }

    /// Get text from clipboard with automatic copying
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}