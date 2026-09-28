//! The main window's translation provider picker: a choice for this run only, on top of
//! `translate_provider` in `tagent-gui.json`.
//!
//! A choice lasts until the app exits or the configured value changes (a Settings save or
//! a hand-edit of the file: the user picked a new default, so the window follows it), and
//! is dropped when its profile disappears from the config. Picking the configured value
//! drops it too, so "overridden" always means "differs from the default".
//!
//! Pure logic (no Slint types, no globals), so it's unit-tested here; `main.rs` keeps the
//! one live instance behind a `Mutex`, since the hotkey and speech paths read it from
//! their own threads.

/// A translation provider picked in the main window for this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChoice {
    /// The picked provider or profile name.
    chosen: String,
    /// `translate_provider` when it was picked; a different value there ends the choice.
    configured: String,
}

/// Records `picked` as this run's choice over `configured`, or drops the choice when
/// `picked` is the configured value itself.
pub fn select(choice: &mut Option<SessionChoice>, picked: &str, configured: &str) {
    *choice = if picked.eq_ignore_ascii_case(configured) {
        None
    } else {
        Some(SessionChoice {
            chosen: picked.to_string(),
            configured: configured.to_string(),
        })
    };
}

/// The translation provider to use now: this run's choice while it still applies,
/// otherwise `configured`. A choice that no longer applies (the configured value changed,
/// or `available` rejects it) is dropped here.
pub fn resolve(
    choice: &mut Option<SessionChoice>,
    configured: &str,
    available: impl Fn(&str) -> bool,
) -> String {
    if let Some(current) = choice {
        if current.configured == configured && available(&current.chosen) {
            return current.chosen.clone();
        }
        *choice = None;
    }
    configured.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: &str) -> bool {
        true
    }

    #[test]
    fn no_choice_uses_the_configured_provider() {
        let mut choice = None;
        assert_eq!(resolve(&mut choice, "google", all), "google");
    }

    #[test]
    fn a_choice_wins_while_the_configured_value_is_unchanged() {
        let mut choice = None;
        select(&mut choice, "deepl", "google");
        assert_eq!(resolve(&mut choice, "google", all), "deepl");
        assert_eq!(resolve(&mut choice, "google", all), "deepl");
    }

    #[test]
    fn picking_the_configured_value_drops_the_choice() {
        let mut choice = None;
        select(&mut choice, "deepl", "google");
        select(&mut choice, "Google", "google");
        assert_eq!(choice, None);
        assert_eq!(resolve(&mut choice, "google", all), "google");
    }

    #[test]
    fn a_new_configured_value_ends_the_choice_for_good() {
        let mut choice = None;
        select(&mut choice, "deepl", "google");
        assert_eq!(resolve(&mut choice, "work", all), "work");
        assert_eq!(choice, None);
        // Setting the old default back doesn't bring the choice back.
        assert_eq!(resolve(&mut choice, "google", all), "google");
    }

    #[test]
    fn a_vanished_profile_falls_back_to_the_configured_provider() {
        let mut choice = None;
        select(&mut choice, "work", "google");
        assert_eq!(resolve(&mut choice, "google", |name| name != "work"), "google");
        assert_eq!(choice, None);
    }
}
