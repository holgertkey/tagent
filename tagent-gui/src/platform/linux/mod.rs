/// Clipboard read/write and "copy selected text" (via `arboard` + the X11 XTest extension).
pub mod clipboard;
/// Global hotkey detection, driven by `rdev` key events and X11 key grabbing (see [`xgrab`]).
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Cursor position and foreground-window focus save/restore, for the Stage 6 popup.
pub mod window;
/// X11 key grabbing (`XGrabKey`) so hotkeys are consumed rather than just observed.
pub mod xgrab;
