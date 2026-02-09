use windows::Win32::UI::Input::KeyboardAndMouse::*;

// Key code constants for cross-platform hotkey configuration and validation
pub const KEY_CONTROL: u32 = VK_CONTROL.0 as u32;
pub const KEY_LCONTROL: u32 = VK_LCONTROL.0 as u32;
pub const KEY_RCONTROL: u32 = VK_RCONTROL.0 as u32;
pub const KEY_SHIFT: u32 = VK_SHIFT.0 as u32;
pub const KEY_LSHIFT: u32 = VK_LSHIFT.0 as u32;
pub const KEY_RSHIFT: u32 = VK_RSHIFT.0 as u32;
pub const KEY_ALT: u32 = VK_MENU.0 as u32;
pub const KEY_LALT: u32 = VK_LMENU.0 as u32;
pub const KEY_RALT: u32 = VK_RMENU.0 as u32;
pub const KEY_LWIN: u32 = VK_LWIN.0 as u32;
pub const KEY_RWIN: u32 = VK_RWIN.0 as u32;
pub const KEY_F1: u32 = VK_F1.0 as u32;
pub const KEY_F4: u32 = VK_F4.0 as u32;
pub const KEY_F12: u32 = VK_F12.0 as u32;
pub const KEY_ESCAPE: u32 = VK_ESCAPE.0 as u32;
pub const KEY_DELETE: u32 = VK_DELETE.0 as u32;

/// Convert key name to Windows virtual key code
pub fn key_name_to_vk(key_name: &str) -> Result<u32, String> {
    let key_lower = key_name.to_lowercase();

    match key_lower.as_str() {
        // Modifiers
        "ctrl" | "control" => Ok(VK_CONTROL.0 as u32),
        "lctrl" | "lcontrol" => Ok(VK_LCONTROL.0 as u32),
        "rctrl" | "rcontrol" => Ok(VK_RCONTROL.0 as u32),
        "alt" => Ok(VK_MENU.0 as u32),
        "lalt" => Ok(VK_LMENU.0 as u32),
        "ralt" => Ok(VK_RMENU.0 as u32),
        "shift" => Ok(VK_SHIFT.0 as u32),
        "lshift" => Ok(VK_LSHIFT.0 as u32),
        "rshift" => Ok(VK_RSHIFT.0 as u32),
        "win" | "windows" => Ok(VK_LWIN.0 as u32),
        "lwin" => Ok(VK_LWIN.0 as u32),
        "rwin" => Ok(VK_RWIN.0 as u32),

        // Function keys
        "f1" => Ok(VK_F1.0 as u32),
        "f2" => Ok(VK_F2.0 as u32),
        "f3" => Ok(VK_F3.0 as u32),
        "f4" => Ok(VK_F4.0 as u32),
        "f5" => Ok(VK_F5.0 as u32),
        "f6" => Ok(VK_F6.0 as u32),
        "f7" => Ok(VK_F7.0 as u32),
        "f8" => Ok(VK_F8.0 as u32),
        "f9" => Ok(VK_F9.0 as u32),
        "f10" => Ok(VK_F10.0 as u32),
        "f11" => Ok(VK_F11.0 as u32),
        "f12" => Ok(VK_F12.0 as u32),

        // Special keys
        "space" => Ok(VK_SPACE.0 as u32),
        "tab" => Ok(VK_TAB.0 as u32),
        "enter" | "return" => Ok(VK_RETURN.0 as u32),
        "esc" | "escape" => Ok(VK_ESCAPE.0 as u32),
        "backspace" => Ok(VK_BACK.0 as u32),
        "delete" | "del" => Ok(VK_DELETE.0 as u32),
        "insert" | "ins" => Ok(VK_INSERT.0 as u32),
        "home" => Ok(VK_HOME.0 as u32),
        "end" => Ok(VK_END.0 as u32),
        "pageup" | "pgup" => Ok(VK_PRIOR.0 as u32),
        "pagedown" | "pgdn" => Ok(VK_NEXT.0 as u32),

        // Arrow keys
        "left" => Ok(VK_LEFT.0 as u32),
        "right" => Ok(VK_RIGHT.0 as u32),
        "up" => Ok(VK_UP.0 as u32),
        "down" => Ok(VK_DOWN.0 as u32),

        // Letters (A-Z)
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
            let ch = s.chars().next().unwrap().to_ascii_uppercase();
            Ok(ch as u32)
        }

        // Numbers (0-9)
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
            let ch = s.chars().next().unwrap();
            Ok(ch as u32)
        }

        _ => Err(format!("Unknown key name: {}", key_name)),
    }
}

/// Normalize virtual key code (convert specific L/R codes to generic codes)
pub fn normalize_vk_code(vk_code: u32) -> u32 {
    match vk_code {
        162 | 163 => 17, // VK_LCONTROL/VK_RCONTROL -> VK_CONTROL
        164 | 165 => 18, // VK_LMENU/VK_RMENU -> VK_MENU
        160 | 161 => 16, // VK_LSHIFT/VK_RSHIFT -> VK_SHIFT
        _ => vk_code,
    }
}

/// Check if a key is currently pressed (Windows: GetAsyncKeyState)
pub fn is_key_pressed(vk_code: i32) -> bool {
    unsafe { GetAsyncKeyState(vk_code) as u16 & 0x8000 != 0 }
}
