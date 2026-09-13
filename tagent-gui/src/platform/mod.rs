// Platform abstraction layer, mirroring tagent-cli's `platform/mod.rs` shape:
// one directory per OS behind `#[cfg(target_os = "...")]`, each defining an
// identically-named `ClipboardManager` with the same methods. Not a
// `trait`/`dyn` object design — the compiler only ever compiles in one
// platform's module. Stage 5 (global hotkeys) will add a `keyboard.rs`
// alongside `clipboard.rs` in each of these same per-OS directories.

/// Linux platform implementation (clipboard via `arboard` + `xdotool`; full
/// feature parity requires X11 or XWayland).
#[cfg(target_os = "linux")]
pub mod linux;
/// macOS platform implementation. Currently a stub: clipboard access is not
/// yet implemented.
#[cfg(target_os = "macos")]
pub mod macos;
/// Windows platform implementation (clipboard via `clipboard-win` + `SendInput`).
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
#[cfg(target_os = "macos")]
pub use self::macos::clipboard::ClipboardManager;
#[cfg(target_os = "windows")]
pub use self::windows::clipboard::ClipboardManager;
