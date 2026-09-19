//! Cursor position and foreground-window focus save/restore, used by the
//! Stage 6 hotkey popup to place itself next to the mouse cursor and to hand
//! keyboard focus straight back to whatever was focused before it appeared
//! (so it never steals focus, even while visible — see the doc comment on
//! `TranslationPopup`'s usage in `main.rs`).
//!
//! Trimmed down from `tagent-cli`'s `WindowManager`
//! (`tagent-cli/src/platform/linux/window.rs`): no terminal-window tracking
//! and no mouse-over-window geometry math (`is_mouse_over_terminal`) — the
//! popup is a Slint window `tagent-gui` owns directly, so hover detection is
//! handled entirely in `app.slint` via `TouchArea.has-hover`, no platform
//! code needed for that part. Free functions rather than a struct with a
//! cached `Display` connection: each is called once per hotkey trigger, not
//! a hot path, so opening/closing its own short-lived connection is simpler
//! than keeping one alive for the process's lifetime.

use std::error::Error;
use std::os::raw::{c_int, c_long, c_uchar, c_ulong};
use x11::xlib;

/// Opaque handle to a previously-focused X11 window, as returned by
/// [`foreground_window`] and consumed by [`set_foreground_window`].
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(c_ulong);

/// Returns the current mouse cursor position, in physical screen coordinates,
/// or `None` if the X11 display can't be opened or the query fails.
pub fn cursor_position() -> Option<(i32, i32)> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = xlib::XDefaultRootWindow(display);

        let mut root_ret: c_ulong = 0;
        let mut child_ret: c_ulong = 0;
        let mut root_x: c_int = 0;
        let mut root_y: c_int = 0;
        let mut win_x: c_int = 0;
        let mut win_y: c_int = 0;
        let mut mask: c_ulong = 0;

        let ok = xlib::XQueryPointer(
            display,
            root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask as *mut c_ulong as *mut _,
        );

        xlib::XCloseDisplay(display);

        if ok == 0 {
            None
        } else {
            Some((root_x, root_y))
        }
    }
}

/// Returns the bounding box of the whole desktop as `(x, y, width, height)`, in
/// physical screen coordinates, or `None` if the X11 display can't be opened.
///
/// On X11 the default screen already spans every monitor (RandR merges them into
/// one virtual screen), so this is its full size anchored at the origin. It's the
/// bounding box, not the union of the monitors' own rectangles: on a layout with
/// differently-sized monitors, a point in the empty corner between them still
/// counts as "inside".
pub fn virtual_screen_bounds() -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let screen = xlib::XDefaultScreen(display);
        let width = xlib::XDisplayWidth(display, screen);
        let height = xlib::XDisplayHeight(display, screen);
        xlib::XCloseDisplay(display);
        (width > 0 && height > 0).then_some((0, 0, width, height))
    }
}

/// Returns the currently focused (foreground) window, if any window manager
/// reports one via `_NET_ACTIVE_WINDOW`.
pub fn foreground_window() -> Option<WindowHandle> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = xlib::XDefaultRootWindow(display);
        let active = get_active_window(display, root);
        xlib::XCloseDisplay(display);
        active.filter(|&w| w != 0).map(WindowHandle)
    }
}

/// Restores keyboard focus to a window previously captured by
/// [`foreground_window`].
pub fn set_foreground_window(handle: WindowHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return Err("Failed to open X11 display".into());
        }
        let root = xlib::XDefaultRootWindow(display);
        xlib::XMapRaised(display, handle.0);
        send_active_window_message(display, root, handle.0);
        xlib::XFlush(display);
        xlib::XCloseDisplay(display);
    }
    Ok(())
}

/// Sends a `_NET_ACTIVE_WINDOW` client message asking the window manager to
/// activate `window`. Ported verbatim from `tagent-cli`'s
/// `send_active_window_message`.
unsafe fn send_active_window_message(display: *mut xlib::Display, root: c_ulong, window: c_ulong) {
    let net_active_window = xlib::XInternAtom(display, c"_NET_ACTIVE_WINDOW".as_ptr(), xlib::False);

    let mut event: xlib::XClientMessageEvent = std::mem::zeroed();
    event.type_ = xlib::ClientMessage;
    event.window = window;
    event.message_type = net_active_window;
    event.format = 32;
    event.data.set_long(0, 1); // source indication: normal application
    event.data.set_long(1, xlib::CurrentTime as c_long);
    event.data.set_long(2, 0);

    xlib::XSendEvent(
        display,
        root,
        xlib::False,
        (xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask) as c_long,
        &mut event as *mut xlib::XClientMessageEvent as *mut xlib::XEvent,
    );
}

/// Reads the `_NET_ACTIVE_WINDOW` property off the root window. Ported
/// verbatim from `tagent-cli`'s `get_active_window`.
unsafe fn get_active_window(display: *mut xlib::Display, root: c_ulong) -> Option<c_ulong> {
    let net_active_window = xlib::XInternAtom(display, c"_NET_ACTIVE_WINDOW".as_ptr(), xlib::True);
    if net_active_window == 0 {
        return None;
    }

    let mut actual_type: c_ulong = 0;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut prop: *mut c_uchar = std::ptr::null_mut();

    let status = xlib::XGetWindowProperty(
        display,
        root,
        net_active_window,
        0,
        1,
        xlib::False,
        xlib::XA_WINDOW,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut prop,
    );

    if status == 0 && !prop.is_null() && nitems > 0 {
        let window = *(prop as *const c_ulong);
        xlib::XFree(prop as *mut _);
        if window != 0 {
            return Some(window);
        }
        return None;
    }
    if !prop.is_null() {
        xlib::XFree(prop as *mut _);
    }
    None
}
