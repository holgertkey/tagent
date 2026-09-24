use crate::config::HotkeyType;
use std::os::raw::{c_int, c_uint, c_ulong};
use x11::xlib;

/// Manages X11 key grabs to prevent hotkey events from reaching other applications.
///
/// Uses XGrabKey to intercept configured hotkey combinations globally.
/// Only supports SingleKey and ModifierCombo hotkey types — DoublePress
/// cannot be grabbed because XGrabKey cannot detect double-tap patterns.
pub struct XGrabManager {
    display: *mut xlib::Display,
    root: c_ulong,
    grabs: Vec<(c_int, c_uint)>, // (keycode, modifiers) pairs for cleanup
}

// XGrabManager is used only from the keyboard hook thread, and X11 Display
// pointers are safe to use from the thread that opened them.
unsafe impl Send for XGrabManager {}

impl XGrabManager {
    /// Open X11 display and get root window. Returns None if X11 is unavailable.
    pub fn new() -> Option<Self> {
        unsafe {
            let display = xlib::XOpenDisplay(std::ptr::null());
            if display.is_null() {
                eprintln!("XGrabManager: Failed to open X11 display");
                return None;
            }
            let root = xlib::XDefaultRootWindow(display);

            // Process-global: replaces Xlib's default error handler (which calls
            // exit() on any error, aborting the whole process/desktop session) for
            // the entire process. XGrabKey raises BadAccess when some other
            // app/WM already grabbed the same combination; logging and continuing
            // is what keeps that from being fatal. Logging the real
            // error_code/serial (rather than a fixed string) is what lets a
            // genuine error be told apart from an expected grab conflict.
            xlib::XSetErrorHandler(Some(log_x11_error));

            Some(Self {
                display,
                root,
                grabs: Vec::new(),
            })
        }
    }

    /// Grab a hotkey so it is consumed by tagent-gui and not forwarded to other apps.
    /// DoublePress hotkeys are silently skipped (cannot be grabbed via XGrabKey).
    pub fn grab_hotkey(&mut self, hotkey: &HotkeyType) {
        match hotkey {
            HotkeyType::SingleKey { vk_code } => {
                self.grab_key(*vk_code, 0);
            }
            HotkeyType::ModifierCombo { modifiers, key } => {
                let mask = vk_modifiers_to_x11_mask(modifiers);
                self.grab_key(*key, mask);

                // AltGr is Mod5 on many keyboard layouts, not Mod1 — grab the
                // same combo under Mod5 too so the keystroke is suppressed
                // (not just detected) regardless of how the active layout maps
                // the physical right-Alt key.
                if modifiers_contain_alt(modifiers) {
                    let altgr_mask =
                        (mask & !(xlib::Mod1Mask as c_uint)) | (xlib::Mod5Mask as c_uint);
                    self.grab_key(*key, altgr_mask);
                }
            }
            HotkeyType::DoublePress { .. } => {
                // Cannot grab double-press patterns with XGrabKey
            }
        }
    }

    /// Grab a single key with a modifier mask, including NumLock/CapsLock variants.
    fn grab_key(&mut self, vk_code: u32, base_mask: c_uint) {
        let keycode = match vk_to_x11_keycode(vk_code) {
            Some(kc) => kc,
            None => {
                eprintln!(
                    "XGrabManager: No X11 keycode mapping for VK code {}",
                    vk_code
                );
                return;
            }
        };

        let lock_mask = xlib::LockMask as c_uint; // CapsLock
        let num_lock_mask = xlib::Mod2Mask as c_uint; // NumLock (typically Mod2)

        // Grab with all combinations of CapsLock and NumLock
        let modifier_variants: [c_uint; 4] =
            [0, lock_mask, num_lock_mask, lock_mask | num_lock_mask];

        for extra in &modifier_variants {
            let mask = base_mask | extra;
            unsafe {
                xlib::XGrabKey(
                    self.display,
                    keycode as c_int,
                    mask,
                    self.root,
                    xlib::True,
                    xlib::GrabModeAsync,
                    xlib::GrabModeAsync,
                );
            }
            self.grabs.push((keycode as c_int, mask));
        }

        unsafe {
            xlib::XFlush(self.display);
        }
    }
}

impl Drop for XGrabManager {
    fn drop(&mut self) {
        unsafe {
            for (keycode, mask) in &self.grabs {
                xlib::XUngrabKey(self.display, *keycode, *mask, self.root);
            }
            xlib::XFlush(self.display);
            xlib::XCloseDisplay(self.display);
        }
    }
}

/// Returns true if any of the abstract modifier codes represents an Alt key
/// (generic or side-specific — side-specific values are currently unreachable
/// after `HotkeyParser::parse`'s L/R-modifier normalization, but kept for
/// clarity/defensiveness).
fn modifiers_contain_alt(modifiers: &[u32]) -> bool {
    use super::keycodes::*;
    modifiers
        .iter()
        .any(|m| matches!(*m, KEY_ALT | KEY_LALT | KEY_RALT))
}

/// X11 error handler installed process-wide by `XGrabManager::new()`. Replaces
/// Xlib's default handler (which calls `exit()` on any error) so a conflicting
/// `XGrabKey` (BadAccess) doesn't abort the whole process. Logs the real
/// error_code/serial instead of a fixed string so a genuine error can be told
/// apart from an expected grab conflict.
extern "C" fn log_x11_error(_display: *mut xlib::Display, event: *mut xlib::XErrorEvent) -> c_int {
    unsafe {
        eprintln!(
            "X11 error (continuing): error_code={} request_code={} serial={}",
            (*event).error_code,
            (*event).request_code,
            (*event).serial
        );
    }
    0
}

/// Convert abstract VK modifier codes to X11 modifier mask
fn vk_modifiers_to_x11_mask(modifiers: &[u32]) -> c_uint {
    use super::keycodes::*;

    let mut mask: c_uint = 0;
    for m in modifiers {
        match *m {
            KEY_CONTROL | KEY_LCONTROL | KEY_RCONTROL => mask |= xlib::ControlMask as c_uint,
            KEY_ALT | KEY_LALT | KEY_RALT => mask |= xlib::Mod1Mask as c_uint,
            KEY_SHIFT | KEY_LSHIFT | KEY_RSHIFT => mask |= xlib::ShiftMask as c_uint,
            KEY_LWIN | KEY_RWIN => mask |= xlib::Mod4Mask as c_uint,
            _ => {}
        }
    }
    mask
}

/// Convert an abstract VK code directly to its X11 **hardware keycode**, using
/// the standard evdev-based keycode numbering that's universal across
/// virtually all modern X11/XKB setups (it's what the X server's `evdev`/
/// `libinput` XKB rules assign, which is effectively every current Linux
/// distribution) — a hardware keycode identifies a *physical* key position,
/// the same key regardless of which character the active layout/group makes
/// it produce.
///
/// **Replaces a keysym-based lookup that was layout-dependent, a real bug
/// (not just a theoretical one)**: the previous implementation converted the
/// VK code to an ASCII/Latin `KeySym` (e.g. VK `'Q'` -> keysym `XK_q`) and
/// resolved *that* to a keycode via `XKeysymToKeycode`, which searches the
/// **currently active** keyboard mapping across all groups. On a layout with
/// no Latin group at all (e.g. a pure Russian layout, as opposed to a
/// combined `us,ru` one), the Latin keysym isn't bound to *any* keycode in
/// that mapping, `XKeysymToKeycode` returns 0, and the grab silently failed —
/// hotkey *detection* (via `rdev`, see `keyboard.rs`) kept working regardless
/// (it's keycode-based already, see below), but the keystroke was no longer
/// *suppressed*: it leaked through into whatever application had keyboard
/// focus instead of being consumed by tagent. Grabbing the fixed hardware
/// keycode directly removes that layout dependency entirely, and — as a
/// bonus — makes suppression use the exact same positional identification
/// detection already did, instead of two different philosophies.
///
/// The specific keycode numbers below match `rdev` 0.5's own internal Linux
/// keycode table (`rdev::linux::keycodes`, not part of its public API, hence
/// this independent copy — the same reason `rdev_key_to_vk` in `keyboard.rs`
/// exists rather than calling into rdev's internals directly) exactly, so
/// grabbing and detection agree on which physical key each abstract VK code
/// means.
pub(super) fn vk_to_x11_keycode(vk_code: u32) -> Option<c_uint> {
    match vk_code {
        // Letters A-Z: physical QWERTY key positions, not alphabetical order,
        // so no linear formula from the VK code -- listed individually.
        0x41 => Some(38), // A
        0x42 => Some(56), // B
        0x43 => Some(54), // C
        0x44 => Some(40), // D
        0x45 => Some(26), // E
        0x46 => Some(41), // F
        0x47 => Some(42), // G
        0x48 => Some(43), // H
        0x49 => Some(31), // I
        0x4A => Some(44), // J
        0x4B => Some(45), // K
        0x4C => Some(46), // L
        0x4D => Some(58), // M
        0x4E => Some(57), // N
        0x4F => Some(32), // O
        0x50 => Some(33), // P
        0x51 => Some(24), // Q
        0x52 => Some(27), // R
        0x53 => Some(39), // S
        0x54 => Some(28), // T
        0x55 => Some(30), // U
        0x56 => Some(55), // V
        0x57 => Some(25), // W
        0x58 => Some(53), // X
        0x59 => Some(29), // Y
        0x5A => Some(52), // Z

        // Numbers 0-9 (top row, not numpad): keycodes run 1..9,0 in physical
        // left-to-right order, not numeric order.
        0x30 => Some(19), // 0
        0x31 => Some(10), // 1
        0x32 => Some(11), // 2
        0x33 => Some(12), // 3
        0x34 => Some(13), // 4
        0x35 => Some(14), // 5
        0x36 => Some(15), // 6
        0x37 => Some(16), // 7
        0x38 => Some(17), // 8
        0x39 => Some(18), // 9

        // Function keys F1-F12: F1-F10 are sequential, but F11/F12 break the
        // pattern (95/96), so no linear formula covers all twelve.
        112 => Some(67), // F1
        113 => Some(68), // F2
        114 => Some(69), // F3
        115 => Some(70), // F4
        116 => Some(71), // F5
        117 => Some(72), // F6
        118 => Some(73), // F7
        119 => Some(74), // F8
        120 => Some(75), // F9
        121 => Some(76), // F10
        122 => Some(95), // F11
        123 => Some(96), // F12

        // Special keys
        32 => Some(65),  // Space
        9 => Some(23),   // Tab
        13 => Some(36),  // Return
        27 => Some(9),   // Escape
        8 => Some(22),   // Backspace
        46 => Some(119), // Delete
        45 => Some(118), // Insert
        36 => Some(110), // Home
        35 => Some(115), // End
        33 => Some(112), // PageUp
        34 => Some(117), // PageDown

        // Arrow keys
        37 => Some(113), // Left
        39 => Some(114), // Right
        38 => Some(111), // Up
        40 => Some(116), // Down

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vk_to_x11_keycode_letters() {
        // Expected values are physical QWERTY keycodes, cross-checked against
        // rdev 0.5.3's own internal Linux keycode table
        // (rdev-0.5.3/src/linux/keycodes.rs: KeyA=38, KeyQ=24, KeyZ=52).
        assert_eq!(vk_to_x11_keycode('A' as u32), Some(38));
        assert_eq!(vk_to_x11_keycode('Q' as u32), Some(24));
        assert_eq!(vk_to_x11_keycode('Z' as u32), Some(52));
    }

    #[test]
    fn test_vk_to_x11_keycode_numbers() {
        // rdev: Num0=19, Num9=18.
        assert_eq!(vk_to_x11_keycode('0' as u32), Some(19));
        assert_eq!(vk_to_x11_keycode('9' as u32), Some(18));
    }

    #[test]
    fn test_vk_to_x11_keycode_function_keys() {
        // rdev: F1=67, F9=75, F12=96 (F11/F12 break the otherwise-sequential
        // F1-F10 pattern -- specifically worth covering here).
        assert_eq!(vk_to_x11_keycode(112), Some(67)); // F1
        assert_eq!(vk_to_x11_keycode(120), Some(75)); // F9
        assert_eq!(vk_to_x11_keycode(123), Some(96)); // F12
    }

    #[test]
    fn test_vk_to_x11_keycode_special_keys() {
        // rdev: Space=65, Escape=9, Return=36.
        assert_eq!(vk_to_x11_keycode(32), Some(65)); // Space
        assert_eq!(vk_to_x11_keycode(27), Some(9)); // Escape
        assert_eq!(vk_to_x11_keycode(13), Some(36)); // Return
    }

    #[test]
    fn test_vk_to_x11_keycode_unknown() {
        assert_eq!(vk_to_x11_keycode(999), None);
    }

    #[test]
    fn test_vk_modifiers_to_x11_mask_alt() {
        use super::super::keycodes::KEY_ALT;
        let mask = vk_modifiers_to_x11_mask(&[KEY_ALT]);
        assert_eq!(mask, xlib::Mod1Mask as c_uint);
    }

    #[test]
    fn test_vk_modifiers_to_x11_mask_ctrl_shift() {
        use super::super::keycodes::{KEY_CONTROL, KEY_SHIFT};
        let mask = vk_modifiers_to_x11_mask(&[KEY_CONTROL, KEY_SHIFT]);
        assert_eq!(mask, (xlib::ControlMask | xlib::ShiftMask) as c_uint);
    }

    #[test]
    fn test_vk_modifiers_to_x11_mask_empty() {
        let mask = vk_modifiers_to_x11_mask(&[]);
        assert_eq!(mask, 0);
    }

    #[test]
    fn test_vk_modifiers_to_x11_mask_win() {
        use super::super::keycodes::KEY_LWIN;
        let mask = vk_modifiers_to_x11_mask(&[KEY_LWIN]);
        assert_eq!(mask, xlib::Mod4Mask as c_uint);
    }

    #[test]
    fn test_modifiers_contain_alt_true_for_generic_and_lr() {
        use super::super::keycodes::{KEY_ALT, KEY_LALT, KEY_RALT};
        assert!(modifiers_contain_alt(&[KEY_ALT]));
        assert!(modifiers_contain_alt(&[KEY_LALT]));
        assert!(modifiers_contain_alt(&[KEY_RALT]));
    }

    #[test]
    fn test_modifiers_contain_alt_false_for_non_alt() {
        use super::super::keycodes::{KEY_CONTROL, KEY_SHIFT};
        assert!(!modifiers_contain_alt(&[KEY_CONTROL, KEY_SHIFT]));
        assert!(!modifiers_contain_alt(&[]));
    }

    #[test]
    fn test_altgr_mask_swaps_mod1_for_mod5_keeps_other_bits() {
        use super::super::keycodes::{KEY_ALT, KEY_CONTROL};
        let mask = vk_modifiers_to_x11_mask(&[KEY_CONTROL, KEY_ALT]);
        let altgr_mask = (mask & !(xlib::Mod1Mask as c_uint)) | (xlib::Mod5Mask as c_uint);

        assert_eq!(altgr_mask, (xlib::ControlMask | xlib::Mod5Mask) as c_uint);
        assert_eq!(altgr_mask & (xlib::Mod1Mask as c_uint), 0);
    }
}
