// Platform abstraction layer, mirroring tagent-cli's `platform/mod.rs` shape:
// one directory per OS behind `#[cfg(target_os = "...")]`, each defining an
// identically-named `ClipboardManager`/`KeyboardHook` with the same methods.
// Not a `trait`/`dyn` object design — the compiler only ever compiles in one
// platform's module.

/// Linux platform implementation (clipboard via `arboard` + XTest; hotkeys via
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

/// What the desktop reports about the global hotkeys when it, not `tagent-gui`, owns
/// them: today only on a Wayland session, through the GlobalShortcuts portal (see
/// `linux::portal`). `KeyboardHook::spawn` passes it to its `on_desktop_hotkeys`
/// callback; X11 and Windows never call that, since there the configured hotkeys are
/// exactly what is registered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopHotkeys {
    /// The trigger the desktop bound for `translate_hotkey`, as it describes it.
    pub translate: Option<String>,
    /// The trigger the desktop bound for `speech_hotkey`.
    pub speech: Option<String>,
    /// Why the hotkeys don't work, for the transcript.
    pub problem: Option<String>,
}

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
#[cfg(target_os = "linux")]
pub use self::linux::keyboard::KeyboardHook;
#[cfg(target_os = "linux")]
pub use self::linux::keycodes;
#[cfg(target_os = "linux")]
pub use self::linux::window;

#[cfg(target_os = "macos")]
pub use self::macos::clipboard::ClipboardManager;
#[cfg(target_os = "macos")]
pub use self::macos::keyboard::KeyboardHook;
#[cfg(target_os = "macos")]
pub use self::macos::keycodes;
#[cfg(target_os = "macos")]
pub use self::macos::window;

#[cfg(target_os = "windows")]
pub use self::windows::clipboard::ClipboardManager;
#[cfg(target_os = "windows")]
pub use self::windows::keyboard::KeyboardHook;
#[cfg(target_os = "windows")]
pub use self::windows::keycodes;
#[cfg(target_os = "windows")]
pub use self::windows::window;
