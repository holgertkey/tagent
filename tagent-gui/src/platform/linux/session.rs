//! Which kind of desktop session `tagent-gui` runs in, detected once at startup.
//!
//! On a Wayland session the app runs its windows on XWayland (see [`init`]), so
//! `WAYLAND_DISPLAY` no longer tells the rest of the code where it is; everything that
//! needs to know (hotkeys, selection, popup placement) asks [`session`] instead.

use std::ffi::{OsStr, OsString};
use std::process::{Command, Stdio};
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

/// The `WAYLAND_DISPLAY` [`init`] removed, if it did.
static REMOVED_WAYLAND_DISPLAY: OnceLock<OsString> = OnceLock::new();

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
        if let Some(value) = std::env::var_os("WAYLAND_DISPLAY") {
            let _ = REMOVED_WAYLAND_DISPLAY.set(value);
        }
        std::env::remove_var("WAYLAND_DISPLAY");
    }
}

/// Opens `url` in the default browser, with the `WAYLAND_DISPLAY` [`init`] removed;
/// `false` when it removed nothing (the caller then uses Slint's `Platform.open-url`).
///
/// A browser started from the app inherits its environment, and one that can't use
/// X11 then fails to start, silently, since nobody waits for it (Snap's Firefox:
/// "cannot open display").
pub fn open_url(url: &str) -> bool {
    let Some(display) = REMOVED_WAYLAND_DISPLAY.get() else {
        return false;
    };
    match browser_command(url, display).spawn() {
        // `xdg-open` may run as long as the browser does; waiting also reaps it.
        Ok(mut child) => {
            let url = url.to_string();
            std::thread::spawn(move || match child.wait() {
                Ok(status) if !status.success() => {
                    eprintln!("Warning: couldn't open {url}: xdg-open exited with {status}")
                }
                Err(error) => eprintln!("Warning: couldn't open {url}: {error}"),
                Ok(_) => {}
            });
        }
        Err(error) => eprintln!("Warning: couldn't open {url}: xdg-open: {error}"),
    }
    true
}

/// `xdg-open url`, with `WAYLAND_DISPLAY` set to `wayland_display`.
fn browser_command(url: &str, wayland_display: &OsStr) -> Command {
    let mut command = Command::new("xdg-open");
    command
        .arg(url)
        .env("WAYLAND_DISPLAY", wayland_display)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_command_gets_wayland_display_back() {
        let command = browser_command("https://example.org", OsStr::new("wayland-0"));
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["https://example.org"]
        );
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "WAYLAND_DISPLAY"
                    && value == Some(OsStr::new("wayland-0")))
        );
    }

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
