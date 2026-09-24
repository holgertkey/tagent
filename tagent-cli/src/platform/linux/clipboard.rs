use arboard::Clipboard;
use std::error::Error;
use std::os::raw::{c_char, c_uint, c_ulong};
use std::sync::Mutex;
use x11::{keysym, xlib, xtest};

/// Linux clipboard access, backed by `arboard` (get/set) and the X11 XTest extension
/// (simulating Ctrl+C to copy the current text selection).
#[derive(Clone)]
pub struct ClipboardManager;

// On X11, clipboard ownership is process-based: a background thread spawned by
// `Clipboard::new()` serves other apps' paste requests only as long as this `Clipboard`
// value stays alive. Creating one per call and dropping it right after `set_text` (the
// previous behavior) closed that thread before clipboard managers reliably picked up the
// contents, which is exactly what arboard's own "Clipboard was dropped very quickly after
// writing" warning flags. Keeping a single instance alive for the process lifetime fixes
// this, per arboard's own recommendation to keep `Clipboard` in more persistent state.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

/// How long [`ClipboardManager::copy_selected_text`] waits for the hotkey's
/// non-modifier key to be released before sending Ctrl+C anyway.
const KEY_RELEASE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

/// Every keycode bound to a modifier (Shift, Lock, Control, Mod1-Mod5) in the
/// current X11 modifier mapping -- Alt, Super and AltGr/ISO_Level3_Shift included,
/// whatever the active layout calls them.
unsafe fn modifier_keycodes(display: *mut xlib::Display) -> Vec<u8> {
    let map = xlib::XGetModifierMapping(display);
    if map.is_null() {
        return Vec::new();
    }
    let len = 8 * (*map).max_keypermod.max(0) as usize;
    let keycodes = std::slice::from_raw_parts((*map).modifiermap, len).to_vec();
    xlib::XFreeModifiermap(map);
    keycodes
}

/// Whether `keycode` is marked as pressed in an `XQueryKeymap` bit vector (bit
/// `keycode % 8` of byte `keycode / 8`).
fn is_keycode_down(keymap: &[c_char; 32], keycode: u8) -> bool {
    (keymap[(keycode / 8) as usize] as u8) & (1 << (keycode % 8)) != 0
}

/// Whether `keymap` reports any key held that isn't one of `modifier_keycodes`.
fn non_modifier_key_held(keymap: &[c_char; 32], modifier_keycodes: &[u8]) -> bool {
    (8..=255u8)
        .any(|keycode| is_keycode_down(keymap, keycode) && !modifier_keycodes.contains(&keycode))
}

/// The keycodes among `candidates` that `keymap` reports as held, without duplicates
/// (several keysyms can share one keycode) and skipping `0`, which
/// `XKeysymToKeycode` returns for a keysym the keymap doesn't have.
fn held_keycodes(keymap: &[c_char; 32], candidates: &[u8]) -> Vec<u8> {
    let mut held = Vec::new();
    for &keycode in candidates {
        if keycode != 0 && is_keycode_down(keymap, keycode) && !held.contains(&keycode) {
            held.push(keycode);
        }
    }
    held
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardManager {
    /// Create a new clipboard manager. Cheap: the underlying `arboard::Clipboard`
    /// is lazily initialized on first use and kept alive for the process lifetime.
    pub fn new() -> Self {
        Self
    }

    /// Run `f` against the shared, lazily-initialized clipboard context.
    fn with_clipboard<T>(
        f: impl FnOnce(&mut Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, Box<dyn Error + Send + Sync>> {
        let mut guard = CLIPBOARD.lock().map_err(|_| "Clipboard lock poisoned")?;
        if guard.is_none() {
            *guard = Some(Clipboard::new().map_err(|e| format!("Clipboard init error: {}", e))?);
        }
        let clipboard = guard.as_mut().expect("just initialized above");
        f(clipboard).map_err(|e| format!("Clipboard error: {}", e).into())
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        Self::with_clipboard(|clipboard| clipboard.get_text())
    }

    /// Set text to clipboard
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let text = text.to_string();
        Self::with_clipboard(move |clipboard| clipboard.set_text(text))
    }

    /// Automatically copy the current text selection by simulating Ctrl+C through the
    /// XTest extension (X11/XWayland only).
    ///
    /// Talks to XTest directly rather than running `xdotool`: for every key it sends,
    /// `xdotool` looks the keysym up in the keymap and, when it lives in another XKB
    /// group than the active one (`c` and the modifiers sit in the Latin group, so any
    /// time a Russian or other non-Latin layout is active), locks that group for the
    /// keystroke and then locks the original one back. On GNOME that made the copy take
    /// seconds and kept the popup/terminal from showing up. Here keys are sent
    /// by hardware keycode, so the layout is never touched: with a non-Latin layout
    /// active the target app gets Ctrl plus that layout's character on the C key
    /// (e.g. Ctrl+Cyrillic_es), exactly as when the user presses Ctrl+C by hand.
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
            // Pure Wayland without XWayland: auto-copy not supported
            return Err(
                "Auto-copy not supported on Wayland. Copy text manually before pressing hotkey."
                    .into(),
            );
        }

        // Wait for user to release hotkey keys
        std::thread::sleep(std::time::Duration::from_millis(100));

        let copy_keycode = super::xgrab::vk_to_x11_keycode('C' as u32)
            .ok_or("No hardware keycode for the C key")? as c_uint;

        unsafe {
            let display = xlib::XOpenDisplay(std::ptr::null());
            if display.is_null() {
                return Err("Failed to open X11 display".into());
            }
            let result = Self::send_copy_keystroke(display, copy_keycode);
            xlib::XCloseDisplay(display);
            result?;
        }

        // Wait for clipboard to update
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(())
    }

    /// Releases the modifiers still held from the hotkey, then sends Ctrl+`copy_keycode`
    /// on `display`.
    unsafe fn send_copy_keystroke(
        display: *mut xlib::Display,
        copy_keycode: c_uint,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (mut event_base, mut error_base, mut major, mut minor) = (0, 0, 0, 0);
        if xtest::XTestQueryExtension(
            display,
            &mut event_base,
            &mut error_base,
            &mut major,
            &mut minor,
        ) == 0
        {
            return Err("The X server has no XTest extension; can't simulate Ctrl+C".into());
        }

        let modifier_keycodes = modifier_keycodes(display);
        let mut keymap: [c_char; 32] = [0; 32];

        // Wait for the hotkey's own non-modifier key (the `A` of Alt+A, an F-key, ...)
        // to be released. Pressing a hotkey grabbed with `XGrabKey` (see `xgrab.rs`)
        // turns into an active keyboard grab that lasts until that key goes up, and
        // while it lasts every key event -- including the fake Ctrl+C below -- goes to
        // the grabbing client instead of the app with the selection. (`xdotool`'s own
        // start-up time used to hide this.) Modifiers don't hold the grab, so they're
        // not waited for, just released below.
        let deadline = std::time::Instant::now() + KEY_RELEASE_TIMEOUT;
        loop {
            xlib::XQueryKeymap(display, keymap.as_mut_ptr());
            if !non_modifier_key_held(&keymap, &modifier_keycodes)
                || std::time::Instant::now() >= deadline
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Release the modifiers the user may still be holding from the hotkey, or the
        // target app would see e.g. Alt+Ctrl+C instead of Ctrl+C. Only keys that are
        // actually down get a release: faking one for a key that's already up is what
        // `xdotool --clearmodifiers` got wrong (it re-pressed them afterwards, leaving
        // "stuck" keys once the user had physically let go).
        for keycode in held_keycodes(&keymap, &modifier_keycodes) {
            xtest::XTestFakeKeyEvent(display, keycode as c_uint, xlib::False, xlib::CurrentTime);
        }
        xlib::XSync(display, xlib::False);

        // Small delay to let X11 process the key releases
        std::thread::sleep(std::time::Duration::from_millis(50));

        let control = match xlib::XKeysymToKeycode(display, keysym::XK_Control_L as c_ulong) {
            0 => return Err("No keycode for Control_L in the current keymap".into()),
            keycode => keycode as c_uint,
        };
        xtest::XTestFakeKeyEvent(display, control, xlib::True, xlib::CurrentTime);
        xtest::XTestFakeKeyEvent(display, copy_keycode, xlib::True, xlib::CurrentTime);
        xtest::XTestFakeKeyEvent(display, copy_keycode, xlib::False, xlib::CurrentTime);
        xtest::XTestFakeKeyEvent(display, control, xlib::False, xlib::CurrentTime);
        xlib::XSync(display, xlib::False);
        Ok(())
    }

    /// Get text from clipboard with automatic copying
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keymap_with(keycodes: &[u8]) -> [c_char; 32] {
        let mut keymap: [c_char; 32] = [0; 32];
        for &keycode in keycodes {
            keymap[(keycode / 8) as usize] |= (1u8 << (keycode % 8)) as c_char;
        }
        keymap
    }

    #[test]
    fn is_keycode_down_reads_the_xquerykeymap_bit_layout() {
        // 64 = Alt_L, 255 = the highest keycode (top bit of the last byte).
        let keymap = keymap_with(&[64, 255]);
        assert!(is_keycode_down(&keymap, 64));
        assert!(is_keycode_down(&keymap, 255));
        assert!(!is_keycode_down(&keymap, 65));
        assert!(!is_keycode_down(&keymap, 37));
    }

    #[test]
    fn held_keycodes_releases_only_modifiers_that_are_down() {
        // Regression test for the non-Latin-layout fix: only held modifiers get a
        // release, and unknown keysyms (keycode 0) and shared keycodes are skipped.
        let keymap = keymap_with(&[64, 50, 0]);
        assert_eq!(
            held_keycodes(&keymap, &[64, 108, 37, 50, 64, 0]),
            vec![64, 50]
        );
        assert!(held_keycodes(&keymap_with(&[]), &[64, 108, 37]).is_empty());
    }

    #[test]
    fn non_modifier_key_held_ignores_modifiers() {
        // Regression test: Ctrl+C must wait until the hotkey's own key (here `A`,
        // keycode 38) is up, since it holds `XGrabKey`'s active grab; a still-held
        // modifier (Alt_L, 64) must not make it wait.
        let modifiers = [50, 62, 37, 105, 64, 108];
        assert!(non_modifier_key_held(&keymap_with(&[64, 38]), &modifiers));
        assert!(!non_modifier_key_held(&keymap_with(&[64]), &modifiers));
        assert!(!non_modifier_key_held(&keymap_with(&[]), &modifiers));
    }

    #[test]
    fn copy_keystroke_uses_the_physical_c_key() {
        // The simulated Ctrl+C must go by hardware keycode (evdev `KEY_C` = 54), never
        // a keysym lookup: that lookup is what switched the XKB group under a
        // non-Latin layout when this went through `xdotool`.
        assert_eq!(super::super::xgrab::vk_to_x11_keycode('C' as u32), Some(54));
    }

    #[test]
    fn test_pure_wayland_returns_error() {
        // Simulate pure Wayland environment (WAYLAND_DISPLAY set, DISPLAY not set)
        // Save current env vars
        let orig_wayland = std::env::var("WAYLAND_DISPLAY").ok();
        let orig_display = std::env::var("DISPLAY").ok();

        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        std::env::remove_var("DISPLAY");

        let clipboard = ClipboardManager::new();
        let result = clipboard.copy_selected_text();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Auto-copy not supported on Wayland"));

        // Restore env vars
        match orig_wayland {
            Some(val) => std::env::set_var("WAYLAND_DISPLAY", val),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
        match orig_display {
            Some(val) => std::env::set_var("DISPLAY", val),
            None => std::env::remove_var("DISPLAY"),
        }
    }

    #[test]
    fn test_clipboard_manager_creation() {
        let _clipboard = ClipboardManager::new();
        // ClipboardManager is a zero-sized struct, just verify it can be created and cloned
        let _cloned = _clipboard.clone();
    }

    #[test]
    fn test_set_text_reuses_persistent_clipboard_instance() {
        // Regression test: `set_text` must reuse the shared `CLIPBOARD` static instead of
        // creating and immediately dropping an `arboard::Clipboard`, or clipboard managers
        // may miss the write (arboard's "dropped very quickly after writing" warning).
        let clipboard = ClipboardManager::new();
        if clipboard
            .set_text("tagent-clipboard-persistence-test")
            .is_err()
        {
            eprintln!("Skipping: no working clipboard in this test environment");
            return;
        }

        assert!(
            CLIPBOARD.lock().unwrap().is_some(),
            "set_text should keep the arboard::Clipboard instance alive in CLIPBOARD, not drop it"
        );
        assert_eq!(
            clipboard.get_text().unwrap(),
            "tagent-clipboard-persistence-test"
        );
    }
}
