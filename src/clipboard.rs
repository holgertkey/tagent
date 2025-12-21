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

            // Wait for modifier keys to be physically released (up to 500ms)
            let start = std::time::Instant::now();
            loop {
                let alt_pressed = (GetAsyncKeyState(VK_MENU.0 as i32) & 0x8000u16 as i16) != 0;
                let shift_pressed = (GetAsyncKeyState(VK_SHIFT.0 as i32) & 0x8000u16 as i16) != 0;
                let lwin_pressed = (GetAsyncKeyState(VK_LWIN.0 as i32) & 0x8000u16 as i16) != 0;
                let rwin_pressed = (GetAsyncKeyState(VK_RWIN.0 as i32) & 0x8000u16 as i16) != 0;

                if !alt_pressed && !shift_pressed && !lwin_pressed && !rwin_pressed {
                    break; // All modifiers released
                }

                if start.elapsed() > std::time::Duration::from_millis(500) {
                    // Timeout - proceed anyway
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            // Additional small delay to ensure system processes the key releases
            std::thread::sleep(std::time::Duration::from_millis(20));

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