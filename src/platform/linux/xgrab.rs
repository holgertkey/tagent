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
            // the entire process, not just for XGrabManager's own calls —
            // WindowManager's Xlib calls go through it too. XGrabKey raises
            // BadAccess when some other app/WM already grabbed the same
            // combination; logging and continuing is what keeps that from being
            // fatal. Logging the real error_code/serial (rather than a fixed
            // string) is what lets a genuine WindowManager-side error be told
            // apart from an expected grab conflict.
            xlib::XSetErrorHandler(Some(log_x11_error));

            Some(Self {
                display,
                root,
                grabs: Vec::new(),
            })
        }
    }

    /// Grab a hotkey so it is consumed by tagent and not forwarded to other apps.
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
                    let altgr_mask = (mask & !(xlib::Mod1Mask as c_uint)) | (xlib::Mod5Mask as c_uint);
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
        let keysym = match vk_to_keysym(vk_code) {
            Some(ks) => ks,
            None => {
                eprintln!(
                    "XGrabManager: No X11 KeySym mapping for VK code {}",
                    vk_code
                );
                return;
            }
        };

        let keycode = unsafe { xlib::XKeysymToKeycode(self.display, keysym) };
        if keycode == 0 {
            eprintln!(
                "XGrabManager: XKeysymToKeycode returned 0 for keysym 0x{:x}",
                keysym
            );
            return;
        }

        let lock_mask = xlib::LockMask as c_uint; // CapsLock
        let num_lock_mask = xlib::Mod2Mask as c_uint; // NumLock (typically Mod2)

        // Grab with all combinations of CapsLock and NumLock
        let modifier_variants: [c_uint; 4] = [
            0,
            lock_mask,
            num_lock_mask,
            lock_mask | num_lock_mask,
        ];

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
    modifiers.iter().any(|m| matches!(*m, KEY_ALT | KEY_LALT | KEY_RALT))
}

/// X11 error handler installed process-wide by `XGrabManager::new()`. Replaces
/// Xlib's default handler (which calls `exit()` on any error) so a conflicting
/// `XGrabKey` (BadAccess) doesn't abort the whole process/desktop session.
/// Logs the real error_code/serial instead of a fixed string so a genuine
/// WindowManager-side error can be told apart from an expected grab conflict.
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

/// Convert abstract VK code to X11 KeySym
fn vk_to_keysym(vk_code: u32) -> Option<c_ulong> {
    match vk_code {
        // Letters A-Z: VK codes match ASCII uppercase
        0x41..=0x5A => {
            // X11 keysyms for lowercase letters are the ASCII lowercase values
            Some((vk_code + 32) as c_ulong) // 'A'(0x41) -> 'a'(0x61)
        }

        // Numbers 0-9: VK codes match ASCII
        0x30..=0x39 => Some(vk_code as c_ulong),

        // Function keys F1-F12: VK 112-123 -> XK_F1(0xFFBE)-XK_F12(0xFFC9)
        112..=123 => Some((0xFFBE + (vk_code - 112)) as c_ulong),

        // Special keys
        32 => Some(0x0020),  // Space -> XK_space
        9 => Some(0xFF09),   // Tab -> XK_Tab
        13 => Some(0xFF0D),  // Return -> XK_Return
        27 => Some(0xFF1B),  // Escape -> XK_Escape
        8 => Some(0xFF08),   // Backspace -> XK_BackSpace
        46 => Some(0xFFFF),  // Delete -> XK_Delete
        45 => Some(0xFF63),  // Insert -> XK_Insert
        36 => Some(0xFF50),  // Home -> XK_Home
        35 => Some(0xFF57),  // End -> XK_End
        33 => Some(0xFF55),  // PageUp -> XK_Page_Up
        34 => Some(0xFF56),  // PageDown -> XK_Page_Down

        // Arrow keys
        37 => Some(0xFF51),  // Left -> XK_Left
        39 => Some(0xFF53),  // Right -> XK_Right
        38 => Some(0xFF52),  // Up -> XK_Up
        40 => Some(0xFF54),  // Down -> XK_Down

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vk_to_keysym_letters() {
        assert_eq!(vk_to_keysym('A' as u32), Some(0x61));
        assert_eq!(vk_to_keysym('Q' as u32), Some(0x71));
        assert_eq!(vk_to_keysym('Z' as u32), Some(0x7A));
    }

    #[test]
    fn test_vk_to_keysym_numbers() {
        assert_eq!(vk_to_keysym('0' as u32), Some(0x30));
        assert_eq!(vk_to_keysym('9' as u32), Some(0x39));
    }

    #[test]
    fn test_vk_to_keysym_function_keys() {
        assert_eq!(vk_to_keysym(112), Some(0xFFBE));  // F1
        assert_eq!(vk_to_keysym(120), Some(0xFFC6));  // F9
        assert_eq!(vk_to_keysym(123), Some(0xFFC9));  // F12
    }

    #[test]
    fn test_vk_to_keysym_special_keys() {
        assert_eq!(vk_to_keysym(32), Some(0x0020));  // Space
        assert_eq!(vk_to_keysym(27), Some(0xFF1B));  // Escape
        assert_eq!(vk_to_keysym(13), Some(0xFF0D));  // Return
    }

    #[test]
    fn test_vk_to_keysym_unknown() {
        assert_eq!(vk_to_keysym(999), None);
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
