use arboard::Clipboard;
use std::error::Error;
use std::sync::Mutex;

/// Linux clipboard access, backed by `arboard` (get/set) and `xdotool` (simulating Ctrl+C
/// to copy the current text selection).
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

    /// Automatically copy selected text (simulate Ctrl+C via xdotool on X11)
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

        // Release all modifier keys that might still be held.
        // Do NOT use --clearmodifiers: it restores modifiers after the command,
        // which causes "stuck" keys when the user has already physically released them.
        let release_result = std::process::Command::new("xdotool")
            .args([
                "keyup",
                "alt",
                "Alt_L",
                "Alt_R",
                "super",
                "Super_L",
                "Super_R",
                "ctrl",
                "Control_L",
                "Control_R",
                "shift",
                "Shift_L",
                "Shift_R",
            ])
            .output();

        match &release_result {
            Ok(result) => {
                if !result.status.success() {
                    // Non-fatal: some key names might not exist on this system
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    eprintln!("Warning: xdotool keyup partial failure: {}", stderr);
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Err(
                        "xdotool not found. Install it with: sudo apt-get install xdotool".into(),
                    );
                }
                return Err(format!("Failed to run xdotool: {}", e).into());
            }
        }

        // Small delay to let X11 process the key releases
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Simulate Ctrl+C without --clearmodifiers
        let output = std::process::Command::new("xdotool")
            .args(["key", "ctrl+c"])
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    return Err(format!("xdotool failed: {}", stderr).into());
                }
            }
            Err(e) => {
                return Err(format!("Failed to run xdotool: {}", e).into());
            }
        }

        // Wait for clipboard to update
        std::thread::sleep(std::time::Duration::from_millis(100));

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
