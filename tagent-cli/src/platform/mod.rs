// Platform abstraction layer for cross-platform support.
// Each platform provides implementations with the same public API.
// Consumer code uses `crate::platform::*` types regardless of platform.

/// Linux platform implementation (full feature parity under X11 via `rdev` + `XGrabKey`; on
/// Wayland, hotkeys through the GlobalShortcuts portal and no terminal show/hide).
#[cfg(target_os = "linux")]
pub mod linux;
/// macOS platform implementation. Currently a stub: clipboard, keyboard hook, and window
/// management are no-ops, so only interactive/CLI mode works on macOS.
#[cfg(target_os = "macos")]
pub mod macos;
/// Windows platform implementation (full feature parity: clipboard, low-level keyboard hook, window management).
#[cfg(target_os = "windows")]
pub mod windows;

/// What the desktop reports about the hotkeys it owns (Wayland, the GlobalShortcuts
/// portal): the bound triggers, in the app's notation (`Alt+A`), or why they don't work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopHotkeys {
    /// The trigger bound for `translate_hotkey`.
    pub translate: Option<String>,
    /// The trigger bound for `speech_hotkey`.
    pub speech: Option<String>,
    /// Why the hotkeys don't work.
    pub problem: Option<String>,
}

/// What the banner's "Active Hotkeys" block shows.
// Only Linux's Wayland path builds the desktop variants.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyBanner {
    /// The configured hotkeys are what is registered (X11, Windows, macOS, and Wayland's
    /// X11 fallback).
    Configured,
    /// Wayland: the desktop hasn't answered the bind yet.
    Waiting,
    /// Wayland: what the desktop bound, or why not.
    Desktop(DesktopHotkeys),
}

// Re-export platform-specific implementations under common names

#[cfg(target_os = "windows")]
pub use self::windows::clipboard::ClipboardManager;
#[cfg(target_os = "windows")]
pub use self::windows::keyboard::{hotkey_banner, wait_for_hotkey_banner, KeyboardHook};
#[cfg(target_os = "windows")]
pub use self::windows::keycodes;
#[cfg(target_os = "windows")]
pub use self::windows::signals;
#[cfg(target_os = "windows")]
pub use self::windows::terminal::{ansi_supported, TerminalTitle};
#[cfg(target_os = "windows")]
pub use self::windows::window::{WindowHandle, WindowManager};

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
#[cfg(target_os = "linux")]
pub use self::linux::keyboard::{hotkey_banner, wait_for_hotkey_banner, KeyboardHook};
#[cfg(target_os = "linux")]
pub use self::linux::keycodes;
#[cfg(target_os = "linux")]
pub use self::linux::signals;
#[cfg(target_os = "linux")]
pub use self::linux::terminal::{ansi_supported, TerminalTitle};
#[cfg(target_os = "linux")]
pub use self::linux::window::{WindowHandle, WindowManager};

#[cfg(target_os = "macos")]
pub use self::macos::clipboard::ClipboardManager;
#[cfg(target_os = "macos")]
pub use self::macos::keyboard::{hotkey_banner, wait_for_hotkey_banner, KeyboardHook};
#[cfg(target_os = "macos")]
pub use self::macos::keycodes;
#[cfg(target_os = "macos")]
pub use self::macos::signals;
#[cfg(target_os = "macos")]
pub use self::macos::terminal::{ansi_supported, TerminalTitle};
#[cfg(target_os = "macos")]
pub use self::macos::window::{WindowHandle, WindowManager};
