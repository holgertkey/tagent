//! Cursor position and foreground-window focus save/restore, used by the
//! Stage 6 hotkey popup to place itself next to the mouse cursor and to hand
//! keyboard focus straight back to whatever was focused before it appeared
//! (so it never steals focus, even while visible — see the doc comment on
//! `TranslationPopup`'s usage in `main.rs`).
//!
//! Trimmed down from `tagent-cli`'s `WindowManager`
//! (`tagent-cli/src/platform/windows/window.rs`): no console-window tracking
//! and no `is_mouse_over_terminal` (the popup is a Slint window `tagent-gui`
//! owns directly, so hover detection is handled entirely in `app.slint` via
//! `TouchArea.has-hover`, no platform code needed for that part).

use std::error::Error;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Opaque handle to a previously-focused window, as returned by
/// [`foreground_window`] and consumed by [`set_foreground_window`].
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(HWND);

/// Returns the current mouse cursor position, in physical screen coordinates,
/// or `None` if the query fails.
pub fn cursor_position() -> Option<(i32, i32)> {
    unsafe {
        let mut cursor_pos = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut cursor_pos).is_err() {
            return None;
        }
        Some((cursor_pos.x, cursor_pos.y))
    }
}

/// Returns the bounding box of the whole desktop (all monitors) as
/// `(x, y, width, height)`, in physical screen coordinates, or `None` if the
/// query reports an empty area. `x`/`y` can be negative when a monitor sits left
/// of or above the primary one. It's the bounding box, not the union of the
/// monitors' own rectangles: on a layout with differently-sized monitors, a point
/// in the empty corner between them still counts as "inside".
pub fn virtual_screen_bounds() -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (width > 0 && height > 0).then_some((x, y, width, height))
    }
}

/// Returns the currently focused (foreground) window, if any.
pub fn foreground_window() -> Option<WindowHandle> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 != 0 {
            Some(WindowHandle(hwnd))
        } else {
            None
        }
    }
}

/// Restores keyboard focus to a window previously captured by
/// [`foreground_window`].
pub fn set_foreground_window(handle: WindowHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
    unsafe {
        if IsIconic(handle.0).as_bool() {
            ShowWindow(handle.0, SW_RESTORE);
        }
        SetForegroundWindow(handle.0);
    }
    Ok(())
}
