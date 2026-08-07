/// Clipboard read/write and "copy selected text" via `arboard` + `xdotool`.
pub mod clipboard;
/// Global hotkey detection via `rdev` event listening, backed by [`xgrab`] for key grabbing.
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Process signal handling (Ctrl+C) and X11 threading initialization.
pub mod signals;
/// Terminal window show/hide/focus management via Xlib.
pub mod window;
/// X11 `XGrabKey`-based global key grabbing so hotkeys are consumed instead of forwarded.
pub mod xgrab;
