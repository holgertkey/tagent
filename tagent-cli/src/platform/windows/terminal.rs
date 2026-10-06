use std::sync::OnceLock;
use windows::core::HSTRING;
use windows::Win32::System::Console::{
    GetConsoleMode, GetConsoleTitleW, GetStdHandle, SetConsoleMode, SetConsoleTitleW, CONSOLE_MODE,
    ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
};

/// Whether the console shows ANSI escape sequences (colors), turning on its virtual
/// terminal processing on the first call if needed. Windows Terminal has it on; the
/// classic console host has it off by default, and one older than Windows 10 can't turn
/// it on, so this returns `false` there. Stdout that isn't a console (a pipe or a file)
/// returns `true`: escape sequences don't need a console there, and `colored` only
/// colors a terminal anyway unless `use_colors = "always"`.
pub fn ansi_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| unsafe {
        let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) else {
            return true;
        };
        let mut mode = CONSOLE_MODE(0);
        if GetConsoleMode(handle, &mut mode).is_err() {
            return true;
        }
        (mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING).0 != 0
            || SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING).is_ok()
    })
}

/// Sets the console window's title for as long as it lives, and restores the
/// previous one when dropped.
///
/// Uses `SetConsoleTitleW`, which works in both the classic console host and
/// Windows Terminal. Without a console (e.g. output redirected from a GUI
/// process) the calls just fail, and nothing happens.
pub struct TerminalTitle {
    original: Option<Vec<u16>>,
    current: Option<String>,
}

impl TerminalTitle {
    /// Saves the current title.
    pub fn new() -> Self {
        // Console titles are limited to well under this many UTF-16 units.
        let mut buffer = vec![0u16; 1024];
        let len = unsafe { GetConsoleTitleW(&mut buffer) } as usize;
        let original = (len > 0).then(|| buffer[..len.min(buffer.len())].to_vec());
        Self {
            original,
            current: None,
        }
    }

    /// Sets the title, skipping the call when it's unchanged.
    pub fn set(&mut self, title: &str) {
        if self.current.as_deref() == Some(title) {
            return;
        }
        let _ = unsafe { SetConsoleTitleW(&HSTRING::from(title)) };
        self.current = Some(title.to_string());
    }
}

impl Drop for TerminalTitle {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            if let Ok(title) = HSTRING::from_wide(original) {
                let _ = unsafe { SetConsoleTitleW(&title) };
            }
        }
    }
}
