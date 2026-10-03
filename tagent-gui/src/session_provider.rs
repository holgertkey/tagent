//! The main window's provider menu: a choice for this run only on each provider axis, on
//! top of `translate_provider`, `dictionary_provider` and `speech_provider` in
//! `tagent-gui.json`.
//!
//! A choice lasts until the app exits or that axis's configured value changes (a Settings
//! save or a hand-edit of the file: the user picked a new default, so the window follows
//! it), and is dropped when its profile disappears from the config. Picking the configured
//! value drops it too, so "overridden" always means "differs from the default". The axes
//! are independent: a choice on one never ends or changes another.
//!
//! Pure logic (no Slint types, no globals), so it's unit-tested here; `main.rs` keeps the
//! one live [`SessionChoices`] behind a `Mutex`, since the hotkey and speech paths read it
//! from their own threads.

use tagent::providers::ProviderAxis;

/// A provider picked in the main window for this run, on one axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChoice {
    /// The picked provider or profile name.
    chosen: String,
    /// The axis's configured provider when it was picked; a different value there ends the
    /// choice.
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

/// The provider to use now on one axis: this run's choice while it still applies,
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

/// This run's choices, one per provider axis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionChoices {
    /// Indexed like [`ProviderAxis::ALL`].
    choices: [Option<SessionChoice>; 3],
}

impl SessionChoices {
    /// No choices: every axis uses its configured provider.
    pub const fn new() -> Self {
        Self {
            choices: [None, None, None],
        }
    }

    fn slot(&mut self, axis: ProviderAxis) -> &mut Option<SessionChoice> {
        let index = match axis {
            ProviderAxis::Translation => 0,
            ProviderAxis::Dictionary => 1,
            ProviderAxis::Speech => 2,
        };
        &mut self.choices[index]
    }

    /// [`select`] on `axis`; the other axes keep their choices.
    pub fn select(&mut self, axis: ProviderAxis, picked: &str, configured: &str) {
        select(self.slot(axis), picked, configured);
    }

    /// [`resolve`] on `axis`; the other axes keep their choices.
    pub fn resolve(
        &mut self,
        axis: ProviderAxis,
        configured: &str,
        available: impl Fn(&str) -> bool,
    ) -> String {
        resolve(self.slot(axis), configured, available)
    }
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
        assert_eq!(
            resolve(&mut choice, "google", |name| name != "work"),
            "google"
        );
        assert_eq!(choice, None);
    }

    #[test]
    fn axes_are_independent() {
        let mut choices = SessionChoices::new();
        choices.select(ProviderAxis::Translation, "deepl", "google");
        choices.select(ProviderAxis::Dictionary, "ollama", "google");

        assert_eq!(
            choices.resolve(ProviderAxis::Translation, "google", all),
            "deepl"
        );
        assert_eq!(
            choices.resolve(ProviderAxis::Dictionary, "google", all),
            "ollama"
        );
        assert_eq!(
            choices.resolve(ProviderAxis::Speech, "google", all),
            "google"
        );
    }

    /// A speech pick, and dropping it, leave the translation choice alone.
    #[test]
    fn a_speech_pick_does_not_touch_the_translation_choice() {
        let mut choices = SessionChoices::new();
        choices.select(ProviderAxis::Translation, "deepl", "google");
        choices.select(ProviderAxis::Speech, "work", "google");
        choices.select(ProviderAxis::Speech, "google", "google");

        assert_eq!(
            choices.resolve(ProviderAxis::Speech, "google", all),
            "google"
        );
        assert_eq!(
            choices.resolve(ProviderAxis::Translation, "google", all),
            "deepl"
        );
    }

    /// Each reset rule ends only its own axis's choice.
    #[test]
    fn reset_rules_work_per_axis() {
        let mut choices = SessionChoices::new();
        choices.select(ProviderAxis::Translation, "deepl", "google");
        choices.select(ProviderAxis::Dictionary, "ollama", "google");
        choices.select(ProviderAxis::Speech, "work", "google");

        // A new configured dictionary provider ends the dictionary choice only.
        assert_eq!(
            choices.resolve(ProviderAxis::Dictionary, "openai", all),
            "openai"
        );
        // A vanished profile ends the speech choice only.
        assert_eq!(
            choices.resolve(ProviderAxis::Speech, "google", |name| name != "work"),
            "google"
        );
        assert_eq!(
            choices.resolve(ProviderAxis::Translation, "google", all),
            "deepl"
        );
        assert_eq!(
            choices.resolve(ProviderAxis::Dictionary, "google", all),
            "google",
            "the ended choice doesn't come back with the old default"
        );
    }
}
