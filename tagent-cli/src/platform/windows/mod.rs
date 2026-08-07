/// Clipboard read/write and "copy selected text" via `clipboard-win` and `SendInput`.
pub mod clipboard;
/// Global hotkey detection via a low-level `WH_KEYBOARD_LL` Win32 keyboard hook.
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Process signal handling (disables the default Ctrl+C console handler).
pub mod signals;
/// Terminal window show/hide/focus management via the Win32 API.
pub mod window;
