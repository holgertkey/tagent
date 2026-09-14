/// Clipboard read/write and "copy selected text". Not yet implemented on macOS.
pub mod clipboard;
/// Global hotkey detection. Not yet implemented on macOS.
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Cursor position and foreground-window focus save/restore. Not yet implemented on macOS.
pub mod window;
