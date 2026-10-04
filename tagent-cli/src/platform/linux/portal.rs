//! Global hotkeys on Wayland through the `org.freedesktop.portal.GlobalShortcuts` portal.
//!
//! On a Wayland session no client sees another client's keys, so `XGrabKey` (see
//! [`super::xgrab`]) only works while an XWayland window has focus. The portal asks the
//! compositor to deliver the shortcuts instead:
//!
//! 1. `Registry.Register` with [`APP_ID`]: since `xdg-desktop-portal` 1.20 a
//!    non-sandboxed app has to say who it is before any other portal call, and GNOME
//!    accepts only a reverse-DNS id backed by an installed `.desktop` file (see
//!    `desktop_entry`).
//! 2. `CreateSession`, then `BindShortcuts` with the two shortcuts and their preferred
//!    triggers, converted from `translate_hotkey`/`speech_hotkey` by
//!    [`to_portal_trigger`]. GNOME shows a consent dialog the first time, where the user
//!    can also pick other keys; what is actually bound comes back as each shortcut's
//!    `trigger_description` and is reported through `on_status`.
//! 3. The `Activated` signal calls the same trigger callbacks as the X11 path; the
//!    `ShortcutsChanged` signal (the user changed a key in GNOME Settings) reports the new
//!    triggers.
//!
//! The session lives as long as [`listen`]'s loop, i.e. until `tagent-cli` exits: dropping
//! it unbinds the shortcuts.
//!
//! Duplicated from `tagent-gui`'s `platform/linux/portal.rs` (platform code isn't shared
//! between the apps); here it runs on `tagent-cli`'s own Tokio runtime. `ashpd` uses its
//! `async-io` feature all the same: Cargo unifies features across the workspace, and
//! `tagent-gui` needs `async-io` (see `Cargo.toml`).

use super::keycodes::{
    KEY_ALT, KEY_CONTROL, KEY_DELETE, KEY_ESCAPE, KEY_F1, KEY_F12, KEY_LALT, KEY_LCONTROL,
    KEY_LSHIFT, KEY_LWIN, KEY_RALT, KEY_RCONTROL, KEY_RSHIFT, KEY_RWIN, KEY_SHIFT,
};
use crate::config::HotkeyType;
use crate::desktop_entry::APP_ID;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};

/// Portal shortcut id of `translate_hotkey`.
const TRANSLATE_ID: &str = "translate";
/// Portal shortcut id of `speech_hotkey`.
const SPEECH_ID: &str = "speech";

/// What the desktop reports about the hotkeys it owns: the bound triggers (in the app's
/// notation, see [`display_trigger`]) or why they don't work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopHotkeys {
    /// The trigger bound for `translate_hotkey`.
    pub translate: Option<String>,
    /// The trigger bound for `speech_hotkey`.
    pub speech: Option<String>,
    /// Why the hotkeys don't work.
    pub problem: Option<String>,
}

/// The desktop has no GlobalShortcuts portal (e.g. a wlroots compositor); the caller
/// falls back to X11 grabs.
#[derive(Debug)]
pub struct NoPortal(pub String);

/// The XDG shortcuts-spec trigger for `hotkey` (`Alt+A` → `ALT+a`, `Ctrl+Shift+F5` →
/// `CTRL+SHIFT+F5`, `Win+T` → `LOGO+t`), or `None` when it has no such form: a double
/// press, or a key without a keysym name here.
pub fn to_portal_trigger(hotkey: &HotkeyType) -> Option<String> {
    match hotkey {
        HotkeyType::SingleKey { vk_code } => keysym_name(*vk_code),
        HotkeyType::ModifierCombo { modifiers, key } => {
            let key = keysym_name(*key)?;
            let mut names: Vec<&str> = Vec::new();
            let groups: [(&str, &[u32]); 4] = [
                ("CTRL", &[KEY_CONTROL, KEY_LCONTROL, KEY_RCONTROL]),
                ("ALT", &[KEY_ALT, KEY_LALT, KEY_RALT]),
                ("SHIFT", &[KEY_SHIFT, KEY_LSHIFT, KEY_RSHIFT]),
                ("LOGO", &[KEY_LWIN, KEY_RWIN]),
            ];
            for (name, codes) in groups {
                if modifiers.iter().any(|m| codes.contains(m)) {
                    names.push(name);
                }
            }
            names.push(&key);
            Some(names.join("+"))
        }
        HotkeyType::DoublePress { .. } => None,
    }
}

/// The xkb keysym name (without `XKB_KEY_`) of an abstract key code.
fn keysym_name(vk_code: u32) -> Option<String> {
    let name = match vk_code {
        code @ KEY_F1..=KEY_F12 => return Some(format!("F{}", code - KEY_F1 + 1)),
        code if (u32::from(b'A')..=u32::from(b'Z')).contains(&code) => {
            return char::from_u32(code).map(|ch| ch.to_ascii_lowercase().to_string())
        }
        code if (u32::from(b'0')..=u32::from(b'9')).contains(&code) => {
            return char::from_u32(code).map(|ch| ch.to_string())
        }
        32 => "space",
        9 => "Tab",
        13 => "Return",
        KEY_ESCAPE => "Escape",
        8 => "BackSpace",
        KEY_DELETE => "Delete",
        45 => "Insert",
        36 => "Home",
        35 => "End",
        33 => "Page_Up",
        34 => "Page_Down",
        37 => "Left",
        39 => "Right",
        38 => "Up",
        40 => "Down",
        _ => return None,
    };
    Some(name.to_string())
}

/// What the desktop bound, as [`DesktopHotkeys`]: each shortcut's trigger description
/// (`None` for one that isn't bound), and a problem when nothing at all is.
fn status_from(shortcuts: &[Shortcut], register_error: Option<&str>) -> DesktopHotkeys {
    let trigger = |id: &str| {
        shortcuts
            .iter()
            .find(|shortcut| shortcut.id() == id)
            .map(|shortcut| display_trigger(shortcut.trigger_description()))
            .filter(|trigger| !trigger.is_empty())
    };
    let translate = trigger(TRANSLATE_ID);
    let speech = trigger(SPEECH_ID);
    let problem = (translate.is_none() && speech.is_none())
        .then(|| problem_text("the desktop bound no hotkeys", register_error));
    DesktopHotkeys {
        translate,
        speech,
        problem,
    }
}

/// A portal `trigger_description` in the app's own hotkey notation (`Alt+A`), in English.
///
/// The description is meant for display, and GNOME localizes it around a GTK accelerator
/// ("Press <Alt>a", "Нажмите <Alt>a"), which would put the system language into an
/// otherwise English UI. Every accelerator-looking word is converted
/// ([`accelerator_to_hotkey`]) and the rest dropped; a description with none is shown
/// as it is.
fn display_trigger(description: &str) -> String {
    let keys: Vec<String> = description
        .split_whitespace()
        .filter_map(accelerator_to_hotkey)
        .collect();
    if keys.is_empty() {
        description.trim().to_string()
    } else {
        keys.join(", ")
    }
}

/// A GTK accelerator (`<Control><Shift>t`, `<Alt>a`, `F9`) as the app's hotkey notation
/// (`Ctrl+Shift+T`, `Alt+A`, `F9`), or `None` for a word that isn't one. Without a
/// modifier, only a single character, an F-key or a known key name counts, so a
/// localized "Press" isn't taken for a key.
fn accelerator_to_hotkey(word: &str) -> Option<String> {
    let word = word.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '«' | '»' | '“' | '”' | '„' | ',' | '.' | ':' | '(' | ')'
        )
    });
    let mut parts = Vec::new();
    let mut rest = word;
    while let Some(after) = rest.strip_prefix('<') {
        let end = after.find('>')?;
        parts.push(
            match after[..end].to_ascii_lowercase().as_str() {
                "control" | "ctrl" | "primary" => "Ctrl",
                "alt" | "mod1" => "Alt",
                "shift" => "Shift",
                "super" | "meta" | "hyper" | "mod4" => "Super",
                _ => return None,
            }
            .to_string(),
        );
        rest = &after[end + 1..];
    }
    let key = match rest {
        "space" => "Space".to_string(),
        "Return" | "KP_Enter" => "Enter".to_string(),
        "Tab" => "Tab".to_string(),
        "Escape" => "Esc".to_string(),
        "BackSpace" => "Backspace".to_string(),
        "Delete" | "Insert" | "Home" | "End" | "Left" | "Right" | "Up" | "Down" => rest.to_string(),
        "Page_Up" | "Prior" => "PageUp".to_string(),
        "Page_Down" | "Next" => "PageDown".to_string(),
        key if key.len() == 1 && key.chars().all(|ch| ch.is_ascii_alphanumeric()) => {
            key.to_ascii_uppercase()
        }
        key if key.len() >= 2
            && key.starts_with('F')
            && key[1..].chars().all(|ch| ch.is_ascii_digit()) =>
        {
            key.to_string()
        }
        key if !parts.is_empty() && !key.is_empty() && !key.contains(['<', '>']) => key.to_string(),
        _ => return None,
    };
    parts.push(key);
    Some(parts.join("+"))
}

/// A problem line for the transcript; mentions the desktop entry when registering the
/// app id failed, the usual reason the portal turns an app down.
fn problem_text(what: &str, register_error: Option<&str>) -> String {
    match register_error {
        Some(err) => format!(
            "Global hotkeys are off: {what}. They need the desktop entry: run \
             \"tagent-cli --install-desktop\" once and restart ({err})."
        ),
        None => format!("Global hotkeys are off: {what}."),
    }
}

/// Binds the hotkeys through the portal and calls `on_translate`/`on_speech` on each
/// activation, until `should_exit` is set.
///
/// Returns `Err(NoPortal)` right away when the desktop has no GlobalShortcuts portal, so
/// the caller can fall back to X11. Every other failure (no app id, the user declined
/// the dialog) is reported through `on_status` and returns `Ok` once `should_exit` is set,
/// hotkeys off.
pub async fn listen(
    translate_hotkey: Option<&HotkeyType>,
    speech_hotkey: Option<&HotkeyType>,
    on_translate: impl Fn(),
    on_speech: impl Fn(),
    on_status: impl Fn(DesktopHotkeys),
    should_exit: &AtomicBool,
) -> Result<(), NoPortal> {
    let result = bind_and_listen(
        translate_hotkey,
        speech_hotkey,
        on_translate,
        on_speech,
        on_status,
        should_exit,
    )
    .await;
    if result.is_ok() {
        // Hotkeys off after a problem: wait for the exit like the X11 path does.
        while !should_exit.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    result
}

async fn bind_and_listen(
    translate_hotkey: Option<&HotkeyType>,
    speech_hotkey: Option<&HotkeyType>,
    on_translate: impl Fn(),
    on_speech: impl Fn(),
    on_status: impl Fn(DesktopHotkeys),
    should_exit: &AtomicBool,
) -> Result<(), NoPortal> {
    if translate_hotkey.is_none() && speech_hotkey.is_none() {
        return Ok(()); // both turned off or unusable; already reported
    }
    let off = |what: &str, register_error: Option<&str>| {
        on_status(DesktopHotkeys {
            problem: Some(problem_text(what, register_error)),
            ..DesktopHotkeys::default()
        })
    };

    // First, before any other portal call on this connection (ashpd shares one).
    let register_error = match APP_ID.parse() {
        Ok(app_id) => ashpd::register_host_app(app_id)
            .await
            .err()
            .map(|err| err.to_string()),
        Err(err) => Some(err.to_string()),
    };
    if let Some(err) = &register_error {
        eprintln!("Warning: cannot register the app id {APP_ID} with the portal: {err}");
    }

    let portal = GlobalShortcuts::new()
        .await
        .map_err(|err| NoPortal(err.to_string()))?;

    let session = match portal.create_session(Default::default()).await {
        Ok(session) => session,
        Err(err) => {
            eprintln!("Warning: GlobalShortcuts.CreateSession failed: {err}");
            off(
                &format!("the desktop refused a shortcuts session: {err}"),
                register_error.as_deref(),
            );
            return Ok(());
        }
    };

    let mut shortcuts = Vec::new();
    if let Some(translate_hotkey) = translate_hotkey {
        shortcuts.push(
            NewShortcut::new(TRANSLATE_ID, "Translate the selection")
                .preferred_trigger(to_portal_trigger(translate_hotkey).as_deref()),
        );
    }
    if let Some(speech_hotkey) = speech_hotkey {
        shortcuts.push(
            NewShortcut::new(SPEECH_ID, "Speak the selection, or stop speaking")
                .preferred_trigger(to_portal_trigger(speech_hotkey).as_deref()),
        );
    }

    // Subscribed before binding, so no activation or change is missed.
    let (mut activated, mut changed) = match (
        portal.receive_activated().await,
        portal.receive_shortcuts_changed().await,
    ) {
        (Ok(activated), Ok(changed)) => (activated, changed),
        (Err(err), _) | (_, Err(err)) => {
            off(&format!("cannot listen to the portal: {err}"), None);
            return Ok(());
        }
    };

    let bound = portal
        .bind_shortcuts(&session, &shortcuts, None, Default::default())
        .await
        .and_then(|request| request.response());
    match bound {
        Ok(bound) => on_status(status_from(bound.shortcuts(), register_error.as_deref())),
        Err(err) => {
            eprintln!("Warning: GlobalShortcuts.BindShortcuts failed: {err}");
            off(
                &format!("the hotkeys weren't bound ({err})"),
                register_error.as_deref(),
            );
            return Ok(());
        }
    }

    loop {
        tokio::select! {
            Some(event) = activated.next() => match event.shortcut_id() {
                TRANSLATE_ID => on_translate(),
                SPEECH_ID => on_speech(),
                _ => {}
            },
            Some(event) = changed.next() => {
                on_status(status_from(event.shortcuts(), register_error.as_deref()));
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if should_exit.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
    }
    drop(session);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HotkeyParser;

    fn trigger(hotkey: &str) -> Option<String> {
        to_portal_trigger(&HotkeyParser::parse(hotkey).unwrap())
    }

    #[test]
    fn modifier_combos_become_spec_triggers() {
        assert_eq!(trigger("Alt+A").as_deref(), Some("ALT+a"));
        assert_eq!(trigger("Alt+S").as_deref(), Some("ALT+s"));
        assert_eq!(trigger("Ctrl+Shift+T").as_deref(), Some("CTRL+SHIFT+t"));
        assert_eq!(trigger("Win+T").as_deref(), Some("LOGO+t"));
        assert_eq!(trigger("Alt+Space").as_deref(), Some("ALT+space"));
        assert_eq!(trigger("Ctrl+Alt+5").as_deref(), Some("CTRL+ALT+5"));
        assert_eq!(trigger("Ctrl+F5").as_deref(), Some("CTRL+F5"));
    }

    #[test]
    fn modifier_order_is_canonical() {
        assert_eq!(trigger("Shift+Ctrl+T").as_deref(), Some("CTRL+SHIFT+t"));
    }

    #[test]
    fn single_function_keys_have_a_trigger() {
        assert_eq!(trigger("F9").as_deref(), Some("F9"));
        assert_eq!(trigger("F12").as_deref(), Some("F12"));
    }

    #[test]
    fn double_presses_have_no_trigger() {
        assert_eq!(trigger("Ctrl+Ctrl"), None);
        assert_eq!(trigger("F8+F8"), None);
    }

    #[test]
    fn special_keys_use_xkb_keysym_names() {
        for (vk, name) in [
            (13, "Return"),
            (33, "Page_Up"),
            (34, "Page_Down"),
            (37, "Left"),
            (8, "BackSpace"),
        ] {
            assert_eq!(keysym_name(vk).as_deref(), Some(name));
        }
        assert_eq!(keysym_name(0xFFFF), None);
    }

    #[test]
    fn trigger_descriptions_lose_their_localized_words() {
        assert_eq!(display_trigger("Нажмите <Alt>a"), "Alt+A");
        assert_eq!(display_trigger("Press <Alt>s"), "Alt+S");
        assert_eq!(display_trigger("Press <Control><Shift>t"), "Ctrl+Shift+T");
        assert_eq!(display_trigger("Drücken Sie <Super>space"), "Super+Space");
        assert_eq!(display_trigger("«<Primary>Return» drücken"), "Ctrl+Enter");
    }

    #[test]
    fn bare_keys_count_only_when_they_look_like_keys() {
        assert_eq!(display_trigger("Press F9"), "F9");
        assert_eq!(display_trigger("Нажмите F12"), "F12");
        assert_eq!(display_trigger("Press Page_Up"), "PageUp");
    }

    #[test]
    fn a_description_without_an_accelerator_is_kept() {
        assert_eq!(display_trigger("  Unassigned "), "Unassigned");
        assert_eq!(display_trigger(""), "");
    }

    #[test]
    fn problem_text_points_at_the_desktop_entry_only_after_a_register_failure() {
        assert!(problem_text("x", Some("App info not found")).contains("--install-desktop"));
        assert!(!problem_text("x", None).contains("--install-desktop"));
    }
}
