//! The main window's provider menu (the button next to ⚙): one section per provider axis,
//! listing what that axis can use, with the provider in effect checked.
//!
//! Pure logic (no Slint types, no globals), so it's unit-tested here; `main.rs` turns the
//! sections into the menu's models and records a pick as this run's choice
//! (`session_provider`).

use crate::provider_form;
use tagent::providers::{ProviderAxis, ProviderProfiles};

/// One pickable entry of a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    /// The kind or profile name a pick selects.
    pub name: String,
    /// What the menu shows: the provider's display name (`"DeepL (work)"` for a profile),
    /// followed by `⚠ <keys>` while a required option is missing.
    pub title: String,
    /// Whether this is the provider in effect on the section's axis.
    pub checked: bool,
}

/// One axis's part of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSection {
    /// The axis a pick here switches.
    pub axis: ProviderAxis,
    /// The section header: `"Translation"`, `"Dictionary (off)"`, ...
    pub heading: String,
    /// The entries, in picker order.
    pub entries: Vec<MenuEntry>,
}

/// The header of `axis`'s section; a turned-off axis says so.
fn heading(axis: ProviderAxis, enabled: bool) -> String {
    let title = match axis {
        ProviderAxis::Translation => "Translation",
        ProviderAxis::Dictionary => "Dictionary",
        ProviderAxis::Speech => "Speech",
    };
    if enabled {
        title.to_string()
    } else {
        format!("{title} (off)")
    }
}

/// How `tagent` labels provider `name` on `axis` without building it: the kind's display
/// name, with the profile name in parentheses for a profile (`"DeepL (work)"`), or the
/// bare name for a kind this axis doesn't know.
pub fn display_title(axis: ProviderAxis, profiles: &ProviderProfiles, name: &str) -> String {
    let kind = profiles.kind_of(name);
    match axis.descriptors().iter().find(|d| d.name == kind) {
        Some(descriptor) if name == kind => descriptor.display_name.to_string(),
        Some(descriptor) => format!("{} ({name})", descriptor.display_name),
        None => name.to_string(),
    }
}

/// The menu's sections, in [`ProviderAxis::ALL`] order. Per axis: the entries Settings'
/// picker would offer ([`provider_form::picker_entries`]: "Show in lists" applies, the
/// provider in effect is always kept), `effective[i]` checked, and a ⚠ on every entry
/// that lacks a required option (`env` supplies `TAGENT_<NAME>_<KEY>` overrides).
/// `enabled[i]` only changes the header: a turned-off axis can still be switched.
pub fn sections(
    profiles: &ProviderProfiles,
    hidden: &[String],
    effective: &[String; 3],
    enabled: [bool; 3],
    env: impl Fn(&str) -> Option<String>,
) -> Vec<MenuSection> {
    let no_edits = provider_form::Edits::new();
    ProviderAxis::ALL
        .into_iter()
        .zip(effective.iter().zip(enabled))
        .map(|(axis, (current, enabled))| {
            let entries = provider_form::picker_entries(axis.kinds(), profiles, hidden, current)
                .into_iter()
                .map(|name| {
                    let mut title = display_title(axis, profiles, &name);
                    let missing = provider_form::missing_required(profiles, &name, &no_edits, &env);
                    if !missing.is_empty() {
                        title.push_str(&format!("  ⚠ {}", missing.join(", ")));
                    }
                    MenuEntry {
                        checked: name.eq_ignore_ascii_case(current),
                        name,
                        title,
                    }
                })
                .collect();
            MenuSection {
                axis,
                heading: heading(axis, enabled),
                entries,
            }
        })
        .collect()
}

/// The text of the ⚠ next to the menu button: what the provider in effect lacks on each
/// turned-on axis (`"deepl: api_key required"`), one line per profile, or `""`.
pub fn warning(
    profiles: &ProviderProfiles,
    effective: &[String; 3],
    enabled: [bool; 3],
    env: impl Fn(&str) -> Option<String>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (current, enabled) in effective.iter().zip(enabled) {
        if !enabled {
            continue;
        }
        let line = provider_form::warning(profiles, current, &provider_form::Edits::new(), &env);
        if !line.is_empty() && !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn google() -> [String; 3] {
        ["google", "google", "google"].map(String::from)
    }

    fn profiles() -> ProviderProfiles {
        let mut profiles = ProviderProfiles::new();
        profiles.insert("ollama", "type", "openai");
        profiles.insert("ollama", "endpoint", "http://localhost:11434/v1");
        profiles.insert("ollama", "model", "qwen3:8b");
        profiles.insert("work", "type", "deepl");
        profiles
    }

    fn names(section: &MenuSection) -> Vec<&str> {
        section.entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn sections_come_in_axis_order_with_their_providers() {
        let sections = sections(&profiles(), &[], &google(), [true; 3], no_env);

        let axes: Vec<ProviderAxis> = sections.iter().map(|s| s.axis).collect();
        assert_eq!(axes, ProviderAxis::ALL);
        let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(headings, ["Translation", "Dictionary", "Speech"]);
        assert_eq!(
            names(&sections[0]),
            ["google", "deepl", "openai", "ollama", "work"]
        );
        assert_eq!(names(&sections[1]), ["google", "openai", "ollama"]);
        assert_eq!(names(&sections[2]), ["google"]);
        assert_eq!(sections[1].entries[2].title, "OpenAI-compatible (ollama)");
        assert_eq!(sections[0].entries[0].title, "Google Translate");
        assert_eq!(sections[1].entries[0].title, "Google Dictionary");
    }

    #[test]
    fn the_provider_in_effect_is_checked_per_axis() {
        let effective = ["work", "OLLAMA", "google"].map(String::from);

        let sections = sections(&profiles(), &[], &effective, [true; 3], no_env);

        let checked: Vec<Vec<&str>> = sections
            .iter()
            .map(|s| {
                s.entries
                    .iter()
                    .filter(|e| e.checked)
                    .map(|e| e.name.as_str())
                    .collect()
            })
            .collect();
        assert_eq!(checked, [vec!["work"], vec!["ollama"], vec!["google"]]);
    }

    /// "Show in lists" applies, except to the provider in effect.
    #[test]
    fn a_hidden_profile_is_left_out_unless_it_is_in_effect() {
        let hidden = vec!["ollama".to_string()];
        let effective = ["google", "ollama", "google"].map(String::from);

        let sections = sections(&profiles(), &hidden, &effective, [true; 3], no_env);

        assert!(!names(&sections[0]).contains(&"ollama"));
        assert!(names(&sections[1]).contains(&"ollama"));
    }

    #[test]
    fn entries_missing_a_required_option_carry_a_warning() {
        let sections = sections(&profiles(), &[], &google(), [true; 3], no_env);

        let title = |section: usize, name: &str| {
            sections[section]
                .entries
                .iter()
                .find(|e| e.name == name)
                .unwrap()
                .title
                .clone()
        };
        assert_eq!(title(0, "deepl"), "DeepL  ⚠ api_key");
        assert_eq!(title(0, "work"), "DeepL (work)  ⚠ api_key");
        assert_eq!(title(1, "openai"), "OpenAI-compatible  ⚠ endpoint, model");
        assert_eq!(title(1, "ollama"), "OpenAI-compatible (ollama)");
        // An environment override counts as set.
        let sections = super::sections(&profiles(), &[], &google(), [true; 3], |var| {
            (var == "TAGENT_WORK_API_KEY").then(|| "k:fx".to_string())
        });
        assert_eq!(sections[0].entries[4].title, "DeepL (work)");
    }

    #[test]
    fn a_disabled_axis_keeps_its_section_with_a_note() {
        let sections = sections(&profiles(), &[], &google(), [true, false, false], no_env);

        let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(
            headings,
            ["Translation", "Dictionary (off)", "Speech (off)"]
        );
        assert_eq!(names(&sections[1]), ["google", "openai", "ollama"]);
    }

    #[test]
    fn warning_covers_every_turned_on_axis_once_per_profile() {
        let mut profiles = profiles();
        profiles.insert("bare", "type", "openai");
        let effective = ["bare", "bare", "google"].map(String::from);

        assert_eq!(
            warning(&profiles, &effective, [true; 3], no_env),
            "bare: endpoint, model required"
        );
        let effective = ["work", "bare", "google"].map(String::from);
        assert_eq!(
            warning(&profiles, &effective, [true; 3], no_env),
            "work: api_key required\nbare: endpoint, model required"
        );
        // A turned-off axis isn't used, so it doesn't warn.
        assert_eq!(
            warning(&profiles, &effective, [true, false, true], no_env),
            "work: api_key required"
        );
        assert_eq!(warning(&profiles, &google(), [true; 3], no_env), "");
    }
}
