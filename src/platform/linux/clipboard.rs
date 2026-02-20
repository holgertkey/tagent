use arboard::Clipboard;
use std::error::Error;

#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init error: {}", e))?;
        clipboard
            .get_text()
            .map_err(|e| format!("Clipboard read error: {}", e).into())
    }

    /// Set text to clipboard
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init error: {}", e))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| format!("Clipboard write error: {}", e).into())
    }

    /// Automatically copy selected text (simulate Ctrl+C via xdotool on X11)
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
            // Pure Wayland without XWayland: auto-copy not supported
            return Err("Auto-copy not supported on Wayland. Copy text manually before pressing hotkey.".into());
        }

        // Wait for user to release hotkey keys
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Release all modifier keys that might still be held.
        // Do NOT use --clearmodifiers: it restores modifiers after the command,
        // which causes "stuck" keys when the user has already physically released them.
        let release_result = std::process::Command::new("xdotool")
            .args([
                "keyup", "alt", "Alt_L", "Alt_R", "super", "Super_L", "Super_R",
                "ctrl", "Control_L", "Control_R", "shift", "Shift_L", "Shift_R",
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
                    return Err("xdotool not found. Install it with: sudo apt-get install xdotool".into());
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
}
