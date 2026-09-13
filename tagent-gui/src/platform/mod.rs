// Platform abstraction layer, mirroring tagent-cli's `platform/mod.rs` shape:
// one directory per OS behind `#[cfg(target_os = "...")]`, each defining an
// identically-named `ClipboardManager`/`KeyboardHook` with the same methods.
// Not a `trait`/`dyn` object design — the compiler only ever compiles in one
// platform's module.

/// Linux platform implementation (clipboard via `arboard` + `xdotool`; hotkeys via
/// `rdev` + `XGrabKey`; full feature parity requires X11 or XWayland).
#[cfg(target_os = "linux")]
pub mod linux;
/// macOS platform implementation. Currently a stub: clipboard access and hotkeys
/// are not yet implemented.
#[cfg(target_os = "macos")]
pub mod macos;
/// Windows platform implementation (clipboard via `clipboard-win` + `SendInput`;
/// hotkeys via a low-level `WH_KEYBOARD_LL` hook).
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
#[cfg(target_os = "linux")]
pub use self::linux::keyboard::KeyboardHook;
#[cfg(target_os = "linux")]
pub use self::linux::keycodes;

#[cfg(target_os = "macos")]
pub use self::macos::clipboard::ClipboardManager;
#[cfg(target_os = "macos")]
pub use self::macos::keyboard::KeyboardHook;
#[cfg(target_os = "macos")]
pub use self::macos::keycodes;

#[cfg(target_os = "windows")]
pub use self::windows::clipboard::ClipboardManager;
#[cfg(target_os = "windows")]
pub use self::windows::keyboard::KeyboardHook;
#[cfg(target_os = "windows")]
pub use self::windows::keycodes;
