use std::error::Error;
use windows::{Win32::Foundation::*, Win32::System::Console::*, Win32::UI::WindowsAndMessaging::*};

/// Platform-agnostic window handle wrapper
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(HWND);

pub struct WindowManager {
    console_window: HWND,
}

impl WindowManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        unsafe {
            let console_window = GetConsoleWindow();
            if console_window.0 == 0 {
                return Err("Failed to get console window handle".into());
            }

            Ok(Self { console_window })
        }
    }

    /// Show and bring the terminal window to foreground
    pub fn show_terminal(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            ShowWindow(self.console_window, SW_SHOW);
            SetForegroundWindow(self.console_window);

            if IsIconic(self.console_window).as_bool() {
                ShowWindow(self.console_window, SW_RESTORE);
            }
        }

        Ok(())
    }

    /// Hide the terminal window
    pub fn hide_terminal(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            ShowWindow(self.console_window, SW_HIDE);
        }
        Ok(())
    }

    /// Get the currently active (foreground) window
    pub fn get_foreground_window(&self) -> Option<WindowHandle> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 != 0 && hwnd != self.console_window {
                Some(WindowHandle(hwnd))
            } else {
                None
            }
        }
    }

    /// Set the specified window as foreground
    pub fn set_foreground_window(&self, handle: WindowHandle) -> Result<(), Box<dyn Error>> {
        unsafe {
            if IsIconic(handle.0).as_bool() {
                ShowWindow(handle.0, SW_RESTORE);
            }
            SetForegroundWindow(handle.0);
        }
        Ok(())
    }

    /// Check if mouse cursor is currently over the terminal window
    pub fn is_mouse_over_terminal(&self) -> bool {
        unsafe {
            let mut cursor_pos = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut cursor_pos).is_err() {
                return false;
            }

            let window_at_cursor = WindowFromPoint(cursor_pos);

            // Direct match
            if window_at_cursor == self.console_window {
                return true;
            }

            // Check multiple ancestor/parent relationships
            let root_window = GetAncestor(window_at_cursor, GA_ROOT);
            let root_owner = GetAncestor(window_at_cursor, GA_ROOTOWNER);
            let parent_window = GetParent(window_at_cursor);

            let console_root = GetAncestor(self.console_window, GA_ROOT);
            let console_parent = GetParent(self.console_window);

            root_window == self.console_window
                || root_owner == self.console_window
                || parent_window == self.console_window
                || window_at_cursor == console_root
                || window_at_cursor == console_parent
                || root_window == console_root
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_creation() {
        let result = WindowManager::new();
        match result {
            Ok(_) => {
                assert!(true);
            }
            Err(e) => {
                assert!(
                    e.to_string().contains("console"),
                    "Error should mention console: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_is_mouse_over_terminal_does_not_panic() {
        let wm = WindowManager::new();
        if let Ok(window_manager) = wm {
            let _ = window_manager.is_mouse_over_terminal();
            assert!(true);
        }
    }
}
