use arboard::Clipboard;
use std::error::Error;

#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error>> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init error: {}", e))?;
        clipboard
            .get_text()
            .map_err(|e| format!("Clipboard read error: {}", e).into())
    }

    /// Set text to clipboard
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init error: {}", e))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| format!("Clipboard write error: {}", e).into())
    }

    /// Automatically copy selected text (simulate Ctrl+C via xdotool on X11)
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error>> {
        if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
            // Pure Wayland without XWayland: auto-copy not supported
            return Err("Auto-copy not supported on Wayland. Copy text manually before pressing hotkey.".into());
        }

        // X11 or XWayland: use xdotool to simulate Ctrl+C
        std::thread::sleep(std::time::Duration::from_millis(100));

        let output = std::process::Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+c"])
            .output();

        match output {
            Ok(result) => {
                if !result.status.success() {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    return Err(format!("xdotool failed: {}", stderr).into());
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Err("xdotool not found. Install it with: sudo apt-get install xdotool".into());
                }
                return Err(format!("Failed to run xdotool: {}", e).into());
            }
        }

        // Wait for clipboard to update
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(())
    }

    /// Get text from clipboard with automatic copying
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error>> {
        self.copy_selected_text()?;
        self.get_text()
    }
}
