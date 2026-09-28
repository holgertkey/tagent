//! Settings > General provider options: which option fields the "Options…" panel shows
//! for a selected provider profile, which required options are still missing (the ⚠ next
//! to a picker), and how the edits made there go back into `provider_options`.
//!
//! Pure logic (no Slint types), so it's unit-tested here; `main.rs` converts [`Field`]s
//! into the dialog's `ProviderOptionField` model.

use std::collections::BTreeMap;
use tagent::providers::{
    dictionary_providers, env_var_name, speech_providers, translation_providers, OptionSpec,
    ProviderProfiles,
};

/// One row of the "Provider options" list: a profile heading, or an option field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The profile the row belongs to (lowercase).
    pub profile: String,
    /// `true` for the heading row that introduces a profile's fields.
    pub heading: bool,
    /// Heading: `"work (google)"` / `"google"`. Field: the option key, e.g. `"api_key"`.
    pub label: String,
    /// The option's description (empty for a heading).
    pub description: String,
    /// The value to show: an edit made in this dialog, or else the saved value.
    pub value: String,
    /// Show as a password field.
    pub secret: bool,
    /// The provider can't be built without it.
    pub required: bool,
    /// The environment variable currently overriding this option, or empty.
    pub env_var: String,
}

/// Edits made in the dialog, by `(profile, key)` (both lowercase). An empty value means
/// "remove the key".
pub type Edits = BTreeMap<(String, String), String>;

/// The options provider kind `kind` declares on any axis, each key once, in declaration
/// order.
fn declared_options(kind: &str) -> Vec<&'static OptionSpec> {
    let mut specs: Vec<&'static OptionSpec> = Vec::new();
    for descriptor in [
        translation_providers(),
        dictionary_providers(),
        speech_providers(),
    ]
    .into_iter()
    .flatten()
    .filter(|descriptor| descriptor.name == kind)
    {
        for spec in descriptor.options {
            if !specs.iter().any(|known| known.key == spec.key) {
                specs.push(spec);
            }
        }
    }
    specs
}

/// The rows for the `selected` profiles (translate, dictionary, speech; duplicates shown
/// once): a heading plus one field per option each profile's kind declares. A profile
/// whose kind declares no options, or that isn't a known kind, gets no rows.
pub fn fields(
    profiles: &ProviderProfiles,
    selected: &[&str],
    edits: &Edits,
    env: impl Fn(&str) -> Option<String>,
) -> Vec<Field> {
    let mut seen: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for profile in selected.iter().map(|name| name.trim().to_lowercase()) {
        if profile.is_empty() || seen.contains(&profile) {
            continue;
        }
        seen.push(profile.clone());
        let kind = profiles.kind_of(&profile);
        let specs = declared_options(&kind);
        if specs.is_empty() {
            continue;
        }
        rows.push(Field {
            profile: profile.clone(),
            heading: true,
            label: if kind == profile {
                profile.clone()
            } else {
                format!("{profile} ({kind})")
            },
            description: String::new(),
            value: String::new(),
            secret: false,
            required: false,
            env_var: String::new(),
        });
        for spec in specs {
            let value = edits
                .get(&(profile.clone(), spec.key.to_string()))
                .cloned()
                .or_else(|| {
                    profiles
                        .get(&profile)
                        .and_then(|options| options.get(spec.key))
                        .cloned()
                })
                .unwrap_or_default();
            let var = env_var_name(&profile, spec.key);
            let env_var = if env(&var).is_some_and(|v| !v.is_empty()) {
                var
            } else {
                String::new()
            };
            rows.push(Field {
                profile: profile.clone(),
                heading: false,
                label: spec.key.to_string(),
                description: spec.description.to_string(),
                value,
                secret: spec.secret,
                required: spec.required,
                env_var,
            });
        }
    }
    rows
}

/// The required options of `profile` that have no value: neither an edit, nor a saved
/// value, nor an environment override. Empty when the provider can be built as far as its
/// options go.
pub fn missing_required(
    profiles: &ProviderProfiles,
    profile: &str,
    edits: &Edits,
    env: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    fields(profiles, &[profile], edits, env)
        .into_iter()
        .filter(|field| {
            !field.heading
                && field.required
                && field.value.trim().is_empty()
                && field.env_var.is_empty()
        })
        .map(|field| field.label)
        .collect()
}

/// The warning shown next to a provider picker: `"<profile>: <keys> required"`, or empty
/// when nothing required is missing.
pub fn warning(
    profiles: &ProviderProfiles,
    profile: &str,
    edits: &Edits,
    env: impl Fn(&str) -> Option<String>,
) -> String {
    let missing = missing_required(profiles, profile, edits, env);
    if missing.is_empty() {
        String::new()
    } else {
        format!(
            "{}: {} required",
            profile.trim().to_lowercase(),
            missing.join(", ")
        )
    }
}

/// The provider axes, in the order of Settings > General's pickers.
pub const AXES: [&str; 3] = ["translation", "dictionary", "speech"];

/// The note shown in the options panel of picker `axis` when other pickers select the same
/// profile (its options are shared, so an edit applies to all of them), or empty.
pub fn sharing_note(selected: &[&str; 3], axis: usize) -> String {
    let Some(profile) = selected.get(axis).map(|name| name.trim().to_lowercase()) else {
        return String::new();
    };
    let others: Vec<&str> = selected
        .iter()
        .enumerate()
        .filter(|&(other, name)| other != axis && name.trim().to_lowercase() == profile)
        .map(|(other, _)| AXES[other])
        .collect();
    if others.is_empty() {
        String::new()
    } else {
        format!(
            "Also selected for {}: these options apply there too.",
            others.join(" and ")
        )
    }
}

/// Applies `edits` to `profiles`: a non-empty value sets the key, an empty (or
/// whitespace-only) one removes it. Untouched keys, `type` and other profiles are kept.
pub fn apply(profiles: &mut ProviderProfiles, edits: &Edits) {
    for ((profile, key), value) in edits {
        let value = value.trim();
        if value.is_empty() {
            profiles.remove(profile, key);
        } else {
            profiles.insert(profile, key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn profiles() -> ProviderProfiles {
        let mut profiles = ProviderProfiles::new();
        profiles.insert("work", "type", "google");
        profiles.insert("work", "timeout_secs", "20");
        profiles.insert("work", "unrelated", "kept");
        profiles
    }

    fn labels(rows: &[Field]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    #[test]
    fn one_heading_and_the_declared_fields_per_selected_profile() {
        let rows = fields(
            &profiles(),
            &["work", "google", "WORK"],
            &Edits::new(),
            no_env,
        );
        assert_eq!(
            labels(&rows),
            [
                "work (google)",
                "timeout_secs",
                "max_retries",
                "google",
                "timeout_secs",
                "max_retries"
            ]
        );
        assert!(rows[0].heading && !rows[1].heading);
        assert_eq!(rows[1].value, "20");
        assert_eq!(rows[2].value, "");
        assert!(!rows[1].description.is_empty());
        // The Google options are neither secret nor required.
        assert!(rows.iter().all(|row| !row.secret && !row.required));
    }

    /// DeepL comes from `tagent`'s `deepl` feature, which this crate enables; its key is
    /// a required password field, with no GUI code of its own.
    #[test]
    fn deepl_key_is_a_required_password_field() {
        let rows = fields(&ProviderProfiles::new(), &["deepl"], &Edits::new(), no_env);
        assert_eq!(
            labels(&rows),
            [
                "deepl",
                "api_key",
                "endpoint",
                "timeout_secs",
                "max_retries"
            ]
        );
        assert!(rows[1].secret && rows[1].required);
        assert!(rows[2..].iter().all(|row| !row.secret && !row.required));
    }

    #[test]
    fn unknown_kinds_get_no_rows() {
        let mut profiles = profiles();
        profiles.insert("llm", "type", "no-such-kind");
        assert!(fields(&profiles, &["llm", "nope"], &Edits::new(), no_env).is_empty());
    }

    #[test]
    fn edits_win_over_saved_values_and_env_is_named() {
        let edits: Edits = [(("work".into(), "timeout_secs".into()), "30".into())].into();
        let env = |var: &str| (var == "TAGENT_WORK_MAX_RETRIES").then(|| "0".to_string());
        let rows = fields(&profiles(), &["work"], &edits, env);
        assert_eq!(rows[1].value, "30");
        assert_eq!(rows[2].env_var, "TAGENT_WORK_MAX_RETRIES");
        assert_eq!(rows[1].env_var, "");
    }

    #[test]
    fn missing_required_key_is_reported_until_set_by_edit_file_or_env() {
        let empty = ProviderProfiles::new();
        assert_eq!(
            missing_required(&empty, "deepl", &Edits::new(), no_env),
            ["api_key"]
        );
        assert_eq!(
            warning(&empty, "DeepL", &Edits::new(), no_env),
            "deepl: api_key required"
        );

        // A whitespace-only edit doesn't count.
        let blank: Edits = [(("deepl".into(), "api_key".into()), "  ".into())].into();
        assert_eq!(
            missing_required(&empty, "deepl", &blank, no_env),
            ["api_key"]
        );

        let edited: Edits = [(("deepl".into(), "api_key".into()), "k:fx".into())].into();
        assert!(missing_required(&empty, "deepl", &edited, no_env).is_empty());

        let mut saved = ProviderProfiles::new();
        saved.insert("deepl", "api_key", "k:fx");
        assert!(missing_required(&saved, "deepl", &Edits::new(), no_env).is_empty());
        // ...unless this dialog's edit clears it.
        let cleared: Edits = [(("deepl".into(), "api_key".into()), "".into())].into();
        assert_eq!(
            missing_required(&saved, "deepl", &cleared, no_env),
            ["api_key"]
        );

        let env = |var: &str| (var == "TAGENT_DEEPL_API_KEY").then(|| "k".to_string());
        assert!(missing_required(&empty, "deepl", &Edits::new(), env).is_empty());
        assert_eq!(warning(&empty, "deepl", &Edits::new(), env), "");
    }

    #[test]
    fn providers_without_required_options_never_warn() {
        assert!(warning(&profiles(), "work", &Edits::new(), no_env).is_empty());
        assert!(warning(&profiles(), "google", &Edits::new(), no_env).is_empty());
        assert!(warning(&profiles(), "no-such-kind", &Edits::new(), no_env).is_empty());
    }

    #[test]
    fn sharing_note_names_the_other_axes_with_the_same_profile() {
        let selected = ["deepl", "google", "Google"];
        assert_eq!(sharing_note(&selected, 0), "");
        assert_eq!(
            sharing_note(&selected, 1),
            "Also selected for speech: these options apply there too."
        );
        let all = ["work", "work", "work"];
        assert_eq!(
            sharing_note(&all, 0),
            "Also selected for dictionary and speech: these options apply there too."
        );
        assert_eq!(sharing_note(&all, 3), "");
    }

    #[test]
    fn apply_sets_and_removes_keys_and_keeps_the_rest() {
        let mut profiles = profiles();
        let edits: Edits = [
            (("work".into(), "timeout_secs".into()), " ".into()),
            (("work".into(), "max_retries".into()), "0".into()),
            (("google".into(), "max_retries".into()), "".into()),
        ]
        .into();
        apply(&mut profiles, &edits);
        let work = profiles.get("work").unwrap();
        assert_eq!(work.get("timeout_secs"), None);
        assert_eq!(work["max_retries"], "0");
        assert_eq!(work["type"], "google");
        assert_eq!(work["unrelated"], "kept");
        // Removing a key that was never set creates nothing.
        assert!(profiles.get("google").is_none());
    }
}
