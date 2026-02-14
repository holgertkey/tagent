use std::error::Error;
use std::os::raw::{c_int, c_long, c_uchar, c_ulong};
use x11::xlib;

/// Platform-agnostic window handle wrapper
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(pub u64);

pub struct WindowManager {
    display: *mut xlib::Display,
    root: c_ulong,
    terminal_window: c_ulong,
}

// WindowManager is shared via Arc<WindowManager> across threads.
// X11 operations are serialized by the caller (translator).
unsafe impl Send for WindowManager {}
unsafe impl Sync for WindowManager {}

impl WindowManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        unsafe {
            let display = xlib::XOpenDisplay(std::ptr::null());
            if display.is_null() {
                return Err("Failed to open X11 display".into());
            }
            let root = xlib::XDefaultRootWindow(display);
            let pid = std::process::id();

            let terminal_window = find_window_by_pid(display, root, pid)
                .unwrap_or_else(|| {
                    // Fallback: use currently focused window at startup
                    get_active_window(display, root).unwrap_or(0)
                });

            if terminal_window == 0 {
                xlib::XCloseDisplay(display);
                return Err("Could not find terminal window".into());
            }

            Ok(Self {
                display,
                root,
                terminal_window,
            })
        }
    }

    /// Show and bring the terminal window to foreground
    pub fn show_terminal(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            // Map the window (un-iconify)
            xlib::XMapRaised(self.display, self.terminal_window);

            // Send _NET_ACTIVE_WINDOW client message to activate it
            send_active_window_message(self.display, self.root, self.terminal_window);

            xlib::XFlush(self.display);
        }
        Ok(())
    }

    /// Hide the terminal window (iconify/minimize)
    pub fn hide_terminal(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let screen = xlib::XDefaultScreen(self.display);
            xlib::XIconifyWindow(self.display, self.terminal_window, screen);
            xlib::XFlush(self.display);
        }
        Ok(())
    }

    /// Get the currently active (foreground) window
    pub fn get_foreground_window(&self) -> Option<WindowHandle> {
        unsafe {
            let active = get_active_window(self.display, self.root)?;
            if active != 0 && active != self.terminal_window {
                Some(WindowHandle(active))
            } else {
                None
            }
        }
    }

    /// Set the specified window as foreground
    pub fn set_foreground_window(&self, handle: WindowHandle) -> Result<(), Box<dyn Error>> {
        unsafe {
            xlib::XMapRaised(self.display, handle.0);
            send_active_window_message(self.display, self.root, handle.0);
            xlib::XFlush(self.display);
        }
        Ok(())
    }

    /// Check if mouse cursor is currently over the terminal window
    pub fn is_mouse_over_terminal(&self) -> bool {
        unsafe {
            let mut root_ret: c_ulong = 0;
            let mut child_ret: c_ulong = 0;
            let mut root_x: c_int = 0;
            let mut root_y: c_int = 0;
            let mut win_x: c_int = 0;
            let mut win_y: c_int = 0;
            let mut mask: c_ulong = 0;

            // Cast mask to the correct pointer type (*mut c_uint)
            let ok = xlib::XQueryPointer(
                self.display,
                self.root,
                &mut root_ret,
                &mut child_ret,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut mask as *mut c_ulong as *mut _,
            );
            if ok == 0 {
                return false;
            }

            // Get terminal window geometry
            let mut x: c_int = 0;
            let mut y: c_int = 0;
            let mut width: c_ulong = 0;
            let mut height: c_ulong = 0;
            let mut border: c_ulong = 0;
            let mut depth: c_ulong = 0;
            let mut root_win: c_ulong = 0;

            // Cast to the correct pointer types for XGetGeometry
            let ok = xlib::XGetGeometry(
                self.display,
                self.terminal_window,
                &mut root_win,
                &mut x,
                &mut y,
                &mut width as *mut c_ulong as *mut _,
                &mut height as *mut c_ulong as *mut _,
                &mut border as *mut c_ulong as *mut _,
                &mut depth as *mut c_ulong as *mut _,
            );
            if ok == 0 {
                return false;
            }

            // Translate window coordinates to root coordinates
            let mut abs_x: c_int = 0;
            let mut abs_y: c_int = 0;
            let mut child: c_ulong = 0;
            xlib::XTranslateCoordinates(
                self.display,
                self.terminal_window,
                self.root,
                0,
                0,
                &mut abs_x,
                &mut abs_y,
                &mut child,
            );

            root_x >= abs_x
                && root_x < abs_x + width as c_int
                && root_y >= abs_y
                && root_y < abs_y + height as c_int
        }
    }
}

impl Drop for WindowManager {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

/// Send _NET_ACTIVE_WINDOW client message to activate a window
unsafe fn send_active_window_message(
    display: *mut xlib::Display,
    root: c_ulong,
    window: c_ulong,
) {
    let net_active_window = xlib::XInternAtom(
        display,
        b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const _,
        xlib::False,
    );

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

/// Read _NET_ACTIVE_WINDOW property from root window
unsafe fn get_active_window(display: *mut xlib::Display, root: c_ulong) -> Option<c_ulong> {
    let net_active_window = xlib::XInternAtom(
        display,
        b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const _,
        xlib::True,
    );
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
    }
    if !prop.is_null() {
        xlib::XFree(prop as *mut _);
    }
    None
}

/// Find a window matching the given PID by walking the X11 window tree
unsafe fn find_window_by_pid(
    display: *mut xlib::Display,
    root: c_ulong,
    target_pid: u32,
) -> Option<c_ulong> {
    let net_wm_pid = xlib::XInternAtom(
        display,
        b"_NET_WM_PID\0".as_ptr() as *const _,
        xlib::True,
    );
    if net_wm_pid == 0 {
        return None;
    }

    find_window_recursive(display, root, net_wm_pid, target_pid)
}

/// Recursively search the window tree for a window with matching _NET_WM_PID
unsafe fn find_window_recursive(
    display: *mut xlib::Display,
    window: c_ulong,
    pid_atom: c_ulong,
    target_pid: u32,
) -> Option<c_ulong> {
    // Check this window's PID
    if let Some(pid) = get_window_pid(display, window, pid_atom) {
        if pid == target_pid {
            return Some(window);
        }
    }

    // Recurse into children
    let mut root_ret: c_ulong = 0;
    let mut parent_ret: c_ulong = 0;
    let mut children: *mut c_ulong = std::ptr::null_mut();
    let mut nchildren: c_ulong = 0;

    // Cast nchildren to the correct type (*mut c_uint)
    let status = xlib::XQueryTree(
        display,
        window,
        &mut root_ret,
        &mut parent_ret,
        &mut children,
        &mut nchildren as *mut c_ulong as *mut _,
    );

    if status == 0 || children.is_null() {
        return None;
    }

    let mut result = None;
    for i in 0..nchildren as usize {
        let child = *children.add(i);
        if let Some(found) = find_window_recursive(display, child, pid_atom, target_pid) {
            result = Some(found);
            break;
        }
    }

    xlib::XFree(children as *mut _);
    result
}

/// Get the _NET_WM_PID property of a window
unsafe fn get_window_pid(
    display: *mut xlib::Display,
    window: c_ulong,
    pid_atom: c_ulong,
) -> Option<u32> {
    let mut actual_type: c_ulong = 0;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut prop: *mut c_uchar = std::ptr::null_mut();

    let status = xlib::XGetWindowProperty(
        display,
        window,
        pid_atom,
        0,
        1,
        xlib::False,
        xlib::XA_CARDINAL,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut prop,
    );

    if status == 0 && !prop.is_null() && nitems > 0 {
        let pid = *(prop as *const u32);
        xlib::XFree(prop as *mut _);
        Some(pid)
    } else {
        if !prop.is_null() {
            xlib::XFree(prop as *mut _);
        }
        None
    }
}
