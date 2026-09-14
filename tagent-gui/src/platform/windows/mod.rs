/// Clipboard read/write and "copy selected text" (via `clipboard-win` + `SendInput`).
pub mod clipboard;
/// Global hotkey detection, via a low-level `WH_KEYBOARD_LL` hook.
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Cursor position and foreground-window focus save/restore, for the Stage 6 popup.
pub mod window;
