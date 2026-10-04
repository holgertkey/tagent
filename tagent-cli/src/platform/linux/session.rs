//! Which kind of desktop session `tagent-cli` runs in, detected once on first use.
//!
//! `DISPLAY` is set on a Wayland session too (XWayland), so it doesn't tell X11 from
//! Wayland; everything that needs to know (hotkeys, selection, terminal management) asks
//! [`session`]. Duplicated from `tagent-gui`'s module of the same name (platform code isn't
//! shared between the apps), minus its `init`: `tagent-cli` has no windows to move to
//! XWayland.

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

/// The session type, detected on first use.
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
