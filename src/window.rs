use std::error::Error;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::{
    Win32::Foundation::*,
    Win32::System::Console::*,
    Win32::UI::WindowsAndMessaging::*,
};

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
        unsafe {
            IsWindowVisible(self.console_window).as_bool()
        }
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
    pub fn set_window_position(&self, x: i32, y: i32, width: i32, height: i32) -> Result<(), Box<dyn Error>> {
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
            let mut flash_info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.console_window,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 3,
                dwTimeout: 0,
            };
            
            FlashWindowEx(&mut flash_info);
        }
        Ok(())
    }
}