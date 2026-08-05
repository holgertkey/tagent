// Abstract key codes (matching Windows VK code values for internal consistency)

/// Generic Control key (either side), used after normalizing [`KEY_LCONTROL`]/[`KEY_RCONTROL`].
pub const KEY_CONTROL: u32 = 17;
/// Left Control key.
pub const KEY_LCONTROL: u32 = 162;
/// Right Control key.
pub const KEY_RCONTROL: u32 = 163;
/// Generic Shift key (either side), used after normalizing [`KEY_LSHIFT`]/[`KEY_RSHIFT`].
pub const KEY_SHIFT: u32 = 16;
/// Left Shift key.
pub const KEY_LSHIFT: u32 = 160;
/// Right Shift key.
pub const KEY_RSHIFT: u32 = 161;
/// Generic Alt key (either side), used after normalizing [`KEY_LALT`]/[`KEY_RALT`].
pub const KEY_ALT: u32 = 18;
/// Left Alt key.
pub const KEY_LALT: u32 = 164;
/// Right Alt key (Option on macOS keyboards).
pub const KEY_RALT: u32 = 165;
/// Left Windows/Super/Command key.
pub const KEY_LWIN: u32 = 91;
/// Right Windows/Super/Command key.
pub const KEY_RWIN: u32 = 92;
/// F1 function key; start of the F1-F12 range allowed as a standalone [`crate::config::HotkeyType::SingleKey`].
pub const KEY_F1: u32 = 112;
/// F4 function key.
pub const KEY_F4: u32 = 115;
/// F12 function key; end of the F1-F12 range allowed as a standalone [`crate::config::HotkeyType::SingleKey`].
pub const KEY_F12: u32 = 123;
/// Escape key.
pub const KEY_ESCAPE: u32 = 27;
/// Delete key.
pub const KEY_DELETE: u32 = 46;

/// Convert a key name (e.g. `"F9"`, `"Alt"`, `"Space"`, `"Ctrl"`) to an abstract virtual key code.
pub fn key_name_to_vk(key_name: &str) -> Result<u32, String> {
    let key_lower = key_name.to_lowercase();

    match key_lower.as_str() {
        "ctrl" | "control" => Ok(KEY_CONTROL),
        "lctrl" | "lcontrol" => Ok(KEY_LCONTROL),
        "rctrl" | "rcontrol" => Ok(KEY_RCONTROL),
        "alt" => Ok(KEY_ALT),
        "lalt" => Ok(KEY_LALT),
        "ralt" => Ok(KEY_RALT),
        "shift" => Ok(KEY_SHIFT),
        "lshift" => Ok(KEY_LSHIFT),
        "rshift" => Ok(KEY_RSHIFT),
        "win" | "windows" | "super" | "command" | "cmd" => Ok(KEY_LWIN),
        "lwin" => Ok(KEY_LWIN),
        "rwin" => Ok(KEY_RWIN),
        "f1" => Ok(112), "f2" => Ok(113), "f3" => Ok(114), "f4" => Ok(115),
        "f5" => Ok(116), "f6" => Ok(117), "f7" => Ok(118), "f8" => Ok(119),
        "f9" => Ok(120), "f10" => Ok(121), "f11" => Ok(122), "f12" => Ok(123),
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
        "left" => Ok(37), "right" => Ok(39), "up" => Ok(38), "down" => Ok(40),
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
            Ok(s.chars().next().unwrap().to_ascii_uppercase() as u32)
        }
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
            Ok(s.chars().next().unwrap() as u32)
        }
        _ => Err(format!("Unknown key name: {}", key_name)),
    }
}

/// Normalize a virtual key code, converting specific left/right variants (e.g. `KEY_LCONTROL`)
/// to their generic form (e.g. [`KEY_CONTROL`]).
pub fn normalize_vk_code(vk_code: u32) -> u32 {
    match vk_code {
        162 | 163 => 17,
        164 | 165 => 18,
        160 | 161 => 16,
        _ => vk_code,
    }
}

/// Check if a key is currently pressed. Always returns `false` on macOS: not yet implemented.
pub fn is_key_pressed(_vk_code: i32) -> bool {
    false
}
