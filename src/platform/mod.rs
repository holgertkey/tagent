// Platform abstraction layer for cross-platform support.
// Each platform provides implementations with the same public API.
// Consumer code uses `crate::platform::*` types regardless of platform.

/// Windows platform implementation (full feature parity: clipboard, low-level keyboard hook, window management).
#[cfg(target_os = "windows")]
pub mod windows;
/// Linux platform implementation (full feature parity under X11/XWayland via `rdev` + `XGrabKey`; pure Wayland falls back to interactive/CLI-only).
#[cfg(target_os = "linux")]
pub mod linux;
/// macOS platform implementation. Currently a stub: clipboard, keyboard hook, and window
/// management are no-ops, so only interactive/CLI mode works on macOS.
#[cfg(target_os = "macos")]
pub mod macos;

// Re-export platform-specific implementations under common names

#[cfg(target_os = "windows")]
pub use self::windows::clipboard::ClipboardManager;
#[cfg(target_os = "windows")]
pub use self::windows::keyboard::KeyboardHook;
#[cfg(target_os = "windows")]
pub use self::windows::keycodes;
#[cfg(target_os = "windows")]
pub use self::windows::signals;
#[cfg(target_os = "windows")]
pub use self::windows::window::{WindowHandle, WindowManager};

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
#[cfg(target_os = "linux")]
pub use self::linux::keyboard::KeyboardHook;
#[cfg(target_os = "linux")]
pub use self::linux::keycodes;
#[cfg(target_os = "linux")]
pub use self::linux::signals;
#[cfg(target_os = "linux")]
pub use self::linux::window::{WindowHandle, WindowManager};

#[cfg(target_os = "macos")]
pub use self::macos::clipboard::ClipboardManager;
#[cfg(target_os = "macos")]
pub use self::macos::keyboard::KeyboardHook;
#[cfg(target_os = "macos")]
pub use self::macos::keycodes;
#[cfg(target_os = "macos")]
pub use self::macos::signals;
#[cfg(target_os = "macos")]
pub use self::macos::window::{WindowHandle, WindowManager};
