//! Which kind of desktop session `tagent-gui` runs in, detected once at startup.
//!
//! On a Wayland session the app runs its windows on XWayland (see [`init`]), so
//! `WAYLAND_DISPLAY` no longer tells the rest of the code where it is; everything that
//! needs to know (hotkeys, selection, popup placement) asks [`session`] instead.

use std::sync::OnceLock;

/// The desktop session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// An X11 session: global key grabs and simulated keystrokes reach every app.
    X11,
    /// A Wayland session (GNOME, KDE, wlroots, ...): global hotkeys go through the
    /// GlobalShortcuts portal and the selection is read from PRIMARY.
    Wayland,
    /// No display server found.
    Headless,
}

static SESSION: OnceLock<Session> = OnceLock::new();

/// The session type from the environment: `XDG_SESSION_TYPE`, then which display
/// variables are set. Pure, for testing; [`session`] calls it with the real values.
pub fn session_from(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> Session {
    let set = |value: Option<&str>| value.is_some_and(|v| !v.is_empty());
    match xdg_session_type {
        Some(kind) if kind.eq_ignore_ascii_case("wayland") => Session::Wayland,
        Some(kind) if kind.eq_ignore_ascii_case("x11") && set(display) => Session::X11,
        _ if set(wayland_display) => Session::Wayland,
        _ if set(display) => Session::X11,
        _ => Session::Headless,
    }
}

/// The session type, detected on first use (see [`init`]).
pub fn session() -> Session {
    *SESSION.get_or_init(|| {
        let var = |name| std::env::var(name).ok();
        session_from(
            var("XDG_SESSION_TYPE").as_deref(),
            var("WAYLAND_DISPLAY").as_deref(),
            var("DISPLAY").as_deref(),
        )
    })
}

/// Detects the session and, on Wayland with XWayland available, makes Slint open its
/// windows on XWayland by removing `WAYLAND_DISPLAY` from the environment (winit picks
/// Wayland whenever that variable is set).
///
/// Native Wayland windows can't place themselves, stay above other windows or read the
/// pointer position, and the popup needs all three. Must run before Slint creates its
/// platform and before any thread is spawned (the environment is process-wide);
/// `main()` calls it right after detaching from the terminal.
pub fn init() {
    if session() == Session::Wayland && std::env::var_os("DISPLAY").is_some() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_session_type_wins() {
        assert_eq!(
            session_from(Some("wayland"), None, Some(":0")),
            Session::Wayland
        );
        assert_eq!(
            session_from(Some("x11"), Some("wayland-0"), Some(":0")),
            Session::X11
        );
    }

    #[test]
    fn display_variables_decide_without_a_session_type() {
        assert_eq!(
            session_from(None, Some("wayland-0"), Some(":0")),
            Session::Wayland
        );
        assert_eq!(session_from(Some("tty"), None, Some(":0")), Session::X11);
        assert_eq!(session_from(None, None, Some(":1")), Session::X11);
        assert_eq!(session_from(None, Some(""), Some("")), Session::Headless);
        assert_eq!(session_from(None, None, None), Session::Headless);
    }

    #[test]
    fn x11_session_type_without_display_falls_back_to_the_variables() {
        assert_eq!(
            session_from(Some("x11"), Some("wayland-0"), None),
            Session::Wayland
        );
    }
}
