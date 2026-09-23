/// Clipboard read/write and "copy selected text" (via `clipboard-win` + `SendInput`).
pub mod clipboard;
/// Re-attaches `println!`/`eprintln!` output to the launching terminal, for the
/// console-less GUI-subsystem executable.
pub mod console;
/// Global hotkey detection, via a low-level `WH_KEYBOARD_LL` hook.
pub mod keyboard;
/// Abstract virtual-key code constants and name/code conversion helpers.
pub mod keycodes;
/// Slint renderer selection (software by default, to avoid an OpenGL-driver deadlock).
pub mod renderer;
/// Cursor position and foreground-window focus save/restore, for the Stage 6 popup.
pub mod window;
