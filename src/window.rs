use std::error::Error;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::{Win32::Foundation::*, Win32::System::Console::*, Win32::UI::WindowsAndMessaging::*};

#[cfg(debug_assertions)]
use std::io::Write;

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
            // Show the window if it's hidden
            ShowWindow(self.console_window, SW_SHOW);

            // Bring it to the foreground
            SetForegroundWindow(self.console_window);

            // Make sure it's not minimized
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

    /// Check if terminal window is visible
    #[allow(dead_code)]
    pub fn is_terminal_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.console_window).as_bool() }
    }

    /// Get the currently active (foreground) window
    pub fn get_foreground_window(&self) -> Option<HWND> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 != 0 && hwnd != self.console_window {
                Some(hwnd)
            } else {
                None
            }
        }
    }

    /// Set the specified window as foreground
    pub fn set_foreground_window(&self, hwnd: HWND) -> Result<(), Box<dyn Error>> {
        unsafe {
            // First show the window if it's minimized
            if IsIconic(hwnd).as_bool() {
                ShowWindow(hwnd, SW_RESTORE);
            }

            // Then bring it to foreground
            SetForegroundWindow(hwnd);
        }
        Ok(())
    }

    /// Get the title of the currently active window
    #[allow(dead_code)]
    pub fn get_active_window_title(&self) -> Result<String, Box<dyn Error>> {
        unsafe {
            let active_window = GetForegroundWindow();
            if active_window.0 == 0 {
                return Ok("Unknown".to_string());
            }

            let mut buffer = [0u16; 512];
            let length = GetWindowTextW(active_window, &mut buffer);

            if length > 0 {
                let os_string = OsString::from_wide(&buffer[..length as usize]);
                Ok(os_string.to_string_lossy().into_owned())
            } else {
                Ok("Unknown".to_string())
            }
        }
    }

    /// Get console window handle (for external use)
    #[allow(dead_code)]
    pub fn get_console_handle(&self) -> HWND {
        self.console_window
    }

    /// Set window position and size
    #[allow(dead_code)]
    pub fn set_window_position(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            SetWindowPos(
                self.console_window,
                HWND_TOP,
                x,
                y,
                width,
                height,
                SWP_SHOWWINDOW | SWP_NOZORDER,
            )?;
        }
        Ok(())
    }

    /// Flash the window to get user attention without bringing it to front
    #[allow(dead_code)]
    pub fn flash_window(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let flash_info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.console_window,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 3,
                dwTimeout: 0,
            };

            FlashWindowEx(&flash_info);
        }
        Ok(())
    }

    /// Check if mouse cursor is currently over the terminal window
    ///
    /// Returns true if the cursor is over the console window or any of its child windows.
    /// Includes debug output when compiled in debug mode.
    #[allow(dead_code)]
    pub fn is_mouse_over_terminal(&self) -> bool {
        unsafe {
            // Get current cursor position
            let mut cursor_pos = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut cursor_pos).is_err() {
                #[cfg(debug_assertions)]
                {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[DEBUG] is_mouse_over_terminal: Failed to get cursor position"
                    );
                }
                return false;
            }

            #[cfg(debug_assertions)]
            {
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Cursor position = ({}, {})",
                    cursor_pos.x,
                    cursor_pos.y
                );
            }

            // Get window at cursor position
            let window_at_cursor = WindowFromPoint(cursor_pos);

            #[cfg(debug_assertions)]
            {
                // Get window titles for debugging
                let mut buffer = [0u16; 256];

                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Window at cursor = {:?}",
                    window_at_cursor
                );

                GetWindowTextW(window_at_cursor, &mut buffer);
                let cursor_title = OsString::from_wide(
                    &buffer[..buffer.iter().position(|&x| x == 0).unwrap_or(0)],
                )
                .to_string_lossy()
                .into_owned();
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Window at cursor title = '{}'",
                    cursor_title
                );

                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Console window = {:?}",
                    self.console_window
                );

                GetWindowTextW(self.console_window, &mut buffer);
                let console_title = OsString::from_wide(
                    &buffer[..buffer.iter().position(|&x| x == 0).unwrap_or(0)],
                )
                .to_string_lossy()
                .into_owned();
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Console window title = '{}'",
                    console_title
                );
            }

            // Direct match
            if window_at_cursor == self.console_window {
                #[cfg(debug_assertions)]
                {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[DEBUG] is_mouse_over_terminal: Direct match - cursor is over terminal"
                    );
                }
                return true;
            }

            // Check multiple ancestor/parent relationships
            let root_window = GetAncestor(window_at_cursor, GA_ROOT);
            let root_owner = GetAncestor(window_at_cursor, GA_ROOTOWNER);
            let parent_window = GetParent(window_at_cursor);

            // Also check if console is a child of the window at cursor
            let console_root = GetAncestor(self.console_window, GA_ROOT);
            let console_parent = GetParent(self.console_window);

            #[cfg(debug_assertions)]
            {
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Root window = {:?}",
                    root_window
                );
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Root owner = {:?}",
                    root_owner
                );
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Parent window = {:?}",
                    parent_window
                );
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Console root = {:?}",
                    console_root
                );
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Console parent = {:?}",
                    console_parent
                );
            }

            // Check various relationships
            let is_over = root_window == self.console_window
                || root_owner == self.console_window
                || parent_window == self.console_window
                || window_at_cursor == console_root
                || window_at_cursor == console_parent
                || root_window == console_root;

            #[cfg(debug_assertions)]
            {
                let _ = writeln!(
                    std::io::stderr(),
                    "[DEBUG] is_mouse_over_terminal: Result = {}",
                    is_over
                );
            }

            is_over
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_creation() {
        // Test that WindowManager::new() doesn't panic
        // Note: This may fail in some test environments without a console window
        let result = WindowManager::new();
        match result {
            Ok(_) => {
                // Success - we have a console window
                assert!(true);
            }
            Err(e) => {
                // In some test environments, there might not be a console window
                // This is acceptable - just ensure we got a meaningful error
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
        // Test that the function doesn't panic when called
        let wm = WindowManager::new();
        if let Ok(window_manager) = wm {
            // Just call the function to ensure it doesn't panic
            // We can't test the actual return value without manual mouse positioning
            let _ = window_manager.is_mouse_over_terminal();
            // If we got here without panicking, the test passes
            assert!(true);
        }
    }

    #[test]
    fn test_console_handle_is_valid() {
        // Test that we get a valid console handle
        let wm = WindowManager::new();
        if let Ok(window_manager) = wm {
            let handle = window_manager.get_console_handle();
            // HWND(0) is invalid, any other value should be valid
            assert_ne!(handle.0, 0, "Console handle should be non-zero");
        }
    }

    #[test]
    fn test_is_terminal_visible() {
        // Test that we can check terminal visibility without panicking
        let wm = WindowManager::new();
        if let Ok(window_manager) = wm {
            // The terminal should be visible when we're running tests
            let is_visible = window_manager.is_terminal_visible();
            // We expect the terminal to be visible during test execution
            assert!(
                is_visible,
                "Terminal should be visible during test execution"
            );
        }
    }
}
