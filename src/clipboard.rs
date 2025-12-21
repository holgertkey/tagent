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
            // Wait a bit to allow user to release modifier keys
            // This is important for Alt+ combinations which are blocked in the hook
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Create input array for SendInput
            let mut inputs: Vec<INPUT> = Vec::new();

            // Release any pressed modifiers first (Alt, Shift, Win)
            // This ensures Ctrl+C is recognized correctly when triggered by hotkeys like Alt+Space

            // Release Alt (both left and right)
            inputs.push(Self::create_key_input(VK_MENU.0 as u16, true));
            inputs.push(Self::create_key_input(VK_LMENU.0 as u16, true));
            inputs.push(Self::create_key_input(VK_RMENU.0 as u16, true));

            // Release Shift (both left and right)
            inputs.push(Self::create_key_input(VK_SHIFT.0 as u16, true));
            inputs.push(Self::create_key_input(VK_LSHIFT.0 as u16, true));
            inputs.push(Self::create_key_input(VK_RSHIFT.0 as u16, true));

            // Release Win (both left and right)
            inputs.push(Self::create_key_input(VK_LWIN.0 as u16, true));
            inputs.push(Self::create_key_input(VK_RWIN.0 as u16, true));

            // Send all key releases at once
            if !inputs.is_empty() {
                SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            }

            // Delay to ensure modifiers are processed
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Simulate Ctrl+C using SendInput
            let mut ctrl_c_inputs: Vec<INPUT> = Vec::new();

            // Ctrl down
            ctrl_c_inputs.push(Self::create_key_input(VK_CONTROL.0 as u16, false));
            // C down
            ctrl_c_inputs.push(Self::create_key_input(b'C' as u16, false));
            // C up
            ctrl_c_inputs.push(Self::create_key_input(b'C' as u16, true));
            // Ctrl up
            ctrl_c_inputs.push(Self::create_key_input(VK_CONTROL.0 as u16, true));

            SendInput(&ctrl_c_inputs, std::mem::size_of::<INPUT>() as i32);

            // Wait for clipboard to update
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(())
    }

    /// Helper function to create keyboard input structure for SendInput
    unsafe fn create_key_input(vk_code: u16, is_keyup: bool) -> INPUT {
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;

        let mut ki = KEYBDINPUT::default();
        ki.wVk = VIRTUAL_KEY(vk_code);
        ki.dwFlags = if is_keyup { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };

        input.Anonymous.ki = ki;
        input
    }

    /// Get text from clipboard with automatic copying
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}