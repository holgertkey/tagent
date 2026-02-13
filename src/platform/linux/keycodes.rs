// Abstract key codes (matching Windows VK code values for internal consistency).
// The keyboard hook implementation maps OS-specific events to these abstract codes.

use std::collections::HashMap;
use std::sync::Mutex;

pub const KEY_CONTROL: u32 = 17;
pub const KEY_LCONTROL: u32 = 162;
pub const KEY_RCONTROL: u32 = 163;
pub const KEY_SHIFT: u32 = 16;
pub const KEY_LSHIFT: u32 = 160;
pub const KEY_RSHIFT: u32 = 161;
pub const KEY_ALT: u32 = 18;
pub const KEY_LALT: u32 = 164;
pub const KEY_RALT: u32 = 165;
pub const KEY_LWIN: u32 = 91;
pub const KEY_RWIN: u32 = 92;
pub const KEY_F1: u32 = 112;
pub const KEY_F4: u32 = 115;
pub const KEY_F12: u32 = 123;
pub const KEY_ESCAPE: u32 = 27;
pub const KEY_DELETE: u32 = 46;

/// Shared key state updated by the keyboard hook
static KEY_STATES: Mutex<Option<HashMap<i32, bool>>> = Mutex::new(None);

/// Convert key name to abstract virtual key code
pub fn key_name_to_vk(key_name: &str) -> Result<u32, String> {
    let key_lower = key_name.to_lowercase();

    match key_lower.as_str() {
        // Modifiers
        "ctrl" | "control" => Ok(KEY_CONTROL),
        "lctrl" | "lcontrol" => Ok(KEY_LCONTROL),
        "rctrl" | "rcontrol" => Ok(KEY_RCONTROL),
        "alt" => Ok(KEY_ALT),
        "lalt" => Ok(KEY_LALT),
        "ralt" => Ok(KEY_RALT),
        "shift" => Ok(KEY_SHIFT),
        "lshift" => Ok(KEY_LSHIFT),
        "rshift" => Ok(KEY_RSHIFT),
        "win" | "windows" | "super" => Ok(KEY_LWIN),
        "lwin" => Ok(KEY_LWIN),
        "rwin" => Ok(KEY_RWIN),

        // Function keys
        "f1" => Ok(112),
        "f2" => Ok(113),
        "f3" => Ok(114),
        "f4" => Ok(115),
        "f5" => Ok(116),
        "f6" => Ok(117),
        "f7" => Ok(118),
        "f8" => Ok(119),
        "f9" => Ok(120),
        "f10" => Ok(121),
        "f11" => Ok(122),
        "f12" => Ok(123),

        // Special keys
        "space" => Ok(32),
        "tab" => Ok(9),
        "enter" | "return" => Ok(13),
        "esc" | "escape" => Ok(KEY_ESCAPE),
        "backspace" => Ok(8),
        "delete" | "del" => Ok(KEY_DELETE),
        "insert" | "ins" => Ok(45),
        "home" => Ok(36),
        "end" => Ok(35),
        "pageup" | "pgup" => Ok(33),
        "pagedown" | "pgdn" => Ok(34),

        // Arrow keys
        "left" => Ok(37),
        "right" => Ok(39),
        "up" => Ok(38),
        "down" => Ok(40),

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
        162 | 163 => 17, // LCONTROL/RCONTROL -> CONTROL
        164 | 165 => 18, // LALT/RALT -> ALT
        160 | 161 => 16, // LSHIFT/RSHIFT -> SHIFT
        _ => vk_code,
    }
}

/// Set key state (called by keyboard hook)
pub fn set_key_state(vk_code: i32, pressed: bool) {
    if let Ok(mut states) = KEY_STATES.lock() {
        let map = states.get_or_insert_with(HashMap::new);
        map.insert(vk_code, pressed);
    }
}

/// Check if a key is currently pressed (updated by keyboard hook)
pub fn is_key_pressed(vk_code: i32) -> bool {
    if let Ok(states) = KEY_STATES.lock() {
        if let Some(map) = states.as_ref() {
            return map.get(&vk_code).copied().unwrap_or(false);
        }
    }
    false
}
