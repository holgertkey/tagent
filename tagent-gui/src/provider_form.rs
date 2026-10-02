//! Provider profiles and options in Settings: which option fields the "Options…" panel
//! shows for a profile, which required options are still missing (the ⚠ next to a
//! picker), the Providers tab's rows and add/delete rules, and how everything staged in
//! the dialog ([`Draft`]) goes back into `provider_options`.
//!
//! Pure logic (no Slint types), so it's unit-tested here; `main.rs` converts [`Field`]s
//! into the dialog's `ProviderOptionField` model.

use std::collections::{BTreeMap, BTreeSet};
use tagent::providers::{
    dictionary_providers, env_var_name, speech_providers, translation_providers,
    validate_profile_name, OptionSpec, ProviderProfiles, DICTIONARY_PROVIDERS, SPEECH_PROVIDERS,
    TRANSLATION_PROVIDERS,
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
    /// Edit in a multi-line editor (see [`is_multiline`]).
    pub multiline: bool,
    /// The built-in value the provider uses when the option is unset, or empty when it
    /// declares none. Shown in the editor while `value` is empty; never a `value` itself.
    pub default: String,
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
            multiline: false,
            default: String::new(),
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
                multiline: is_multiline(spec.multiline, spec.secret),
                default: spec.default.unwrap_or_default().to_string(),
            });
        }
    }
    rows
}

/// Whether an option gets the multi-line editor: one declared `multiline`, unless it's
/// `secret` (the multi-line editor has no password mode, so a secret stays masked).
pub fn is_multiline(multiline: bool, secret: bool) -> bool {
    multiline && !secret
}

/// The edit to record for an option with built-in value `default` when the user typed
/// `value`: `""` (= the default, so saving stores nothing) when it equals the default
/// after CRLF → LF and trimming both sides, else `value` with LF line breaks.
pub fn normalize_edit(default: &str, value: &str) -> String {
    let value = value.replace("\r\n", "\n");
    let default = default.replace("\r\n", "\n");
    if !default.trim().is_empty() && value.trim() == default.trim() {
        String::new()
    } else {
        value
    }
}

/// The placeholders whose absence gets a warning: a prompt without `{to}` doesn't tell the
/// model the target language. (A missing `{from}` is harmless, the model sees the text.)
const WARNED_PLACEHOLDERS: [&str; 1] = ["{to}"];

/// The soft warning under a multi-line editor, or `""`: for each warned placeholder the
/// built-in `default` contains, whether the effective `value` (blank = the default) lacks
/// it. Derived from the default only, so the GUI needs no knowledge of particular option
/// keys.
pub fn soft_warning(default: &str, value: &str) -> String {
    let effective = if value.trim().is_empty() {
        default
    } else {
        value
    };
    WARNED_PLACEHOLDERS
        .iter()
        .find(|placeholder| default.contains(*placeholder) && !effective.contains(*placeholder))
        .map(|placeholder| {
            format!(
                "⚠ The prompt has no {placeholder}: the model won't be told the target language."
            )
        })
        .unwrap_or_default()
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

/// The provider axes, in the order of Settings > Providers' pickers.
pub const AXES: [&str; 3] = ["translation", "dictionary", "speech"];

/// Applies `edits` to `profiles`: a non-empty value sets the key, an empty (or
/// whitespace-only) one removes it. Untouched keys, `type` and other profiles are kept.
fn apply(profiles: &mut ProviderProfiles, edits: &Edits) {
    for ((profile, key), value) in edits {
        let value = value.trim();
        if value.is_empty() {
            profiles.remove(profile, key);
        } else {
            profiles.insert(profile, key, value);
        }
    }
}

/// Everything Settings has staged for `provider_options` until its OK: profiles added
/// and deleted on the Providers tab, and option values kept by the options panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    /// Profiles added in this dialog: name → kind (both lowercase).
    pub created: BTreeMap<String, String>,
    /// Profiles to remove with all their options (lowercase). A name deleted and then
    /// added again stays here, so the new profile starts without the old options.
    pub deleted: BTreeSet<String>,
    /// Option values kept by the options panel.
    pub edits: Edits,
}

impl Draft {
    /// Stages a new profile `name` of provider kind `kind`.
    pub fn add(&mut self, name: &str, kind: &str) {
        self.created
            .insert(name.trim().to_lowercase(), kind.trim().to_lowercase());
    }

    /// Stages the deletion of profile `name`, dropping its option edits. A profile that
    /// was only added in this dialog is simply forgotten.
    pub fn delete(&mut self, name: &str) {
        let name = name.trim().to_lowercase();
        self.edits.retain(|(profile, _), _| *profile != name);
        if self.created.remove(&name).is_none() {
            self.deleted.insert(name);
        }
    }

    /// Applies the draft to `profiles`: deletions first (every option of the profile),
    /// then additions (their `type`), then the option edits, except those of a profile
    /// that ends up deleted. Other profiles and keys are kept as they are.
    pub fn apply(&self, profiles: &mut ProviderProfiles) {
        for name in &self.deleted {
            profiles.remove_profile(name);
        }
        for (name, kind) in &self.created {
            profiles.insert(name, TYPE_KEY, kind.as_str());
        }
        let edits: Edits = self
            .edits
            .iter()
            .filter(|((profile, _), _)| {
                !self.deleted.contains(profile) || self.created.contains_key(profile)
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        apply(profiles, &edits);
    }

    /// `saved` with the draft applied: what the dialog shows.
    pub fn view(&self, saved: &ProviderProfiles) -> ProviderProfiles {
        let mut view = saved.clone();
        self.apply(&mut view);
        view
    }
}

/// The option that selects a profile's provider kind.
const TYPE_KEY: &str = "type";

/// Every provider kind compiled into `tagent`, each once, in registry order (translation,
/// then dictionary, then speech kinds).
pub fn builtin_kinds() -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = Vec::new();
    for descriptor in [
        translation_providers(),
        dictionary_providers(),
        speech_providers(),
    ]
    .into_iter()
    .flatten()
    {
        if !kinds.contains(&descriptor.name) {
            kinds.push(descriptor.name);
        }
    }
    kinds
}

/// Why `name` can't be added as a profile of kind `kind` to `view`, or `""` when it can.
pub fn name_error(view: &ProviderProfiles, name: &str, kind: &str) -> String {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return "Enter a name for the profile.".to_string();
    }
    if builtin_kinds().contains(&name.as_str()) {
        return format!("`{name}` is a built-in provider: it is already in the list.");
    }
    if let Err(error) = validate_profile_name(&name, kind) {
        return error.to_string();
    }
    if view.get(&name).is_some() {
        return format!("A profile named `{name}` already exists.");
    }
    String::new()
}

/// One row of the Providers tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    /// The profile (or built-in kind) name, lowercase.
    pub name: String,
    /// What the row shows: `"google (built-in)"`, `"work (openai)"`,
    /// `"llm (nope, unknown kind)"`.
    pub label: String,
    /// A built-in kind: no "Delete".
    pub builtin: bool,
    /// Its kind is compiled into `tagent`: "Options…" and "Test" work.
    pub known_kind: bool,
    /// Offered in the provider pickers ("Show in lists" checked).
    pub shown: bool,
    /// "Show in lists" can be unchecked (see [`can_hide`]).
    pub hideable: bool,
}

/// The Providers tab's rows: every built-in kind (in registry order), then every profile
/// of `view` that isn't named after one (sorted). `hidden` are the names left out of the
/// pickers.
pub fn profile_rows(view: &ProviderProfiles, hidden: &[String]) -> Vec<ProfileRow> {
    let kinds = builtin_kinds();
    let shown = |name: &str| !hidden.iter().any(|hidden| hidden == name);
    let builtins = kinds.iter().map(|kind| ProfileRow {
        name: kind.to_string(),
        label: format!("{kind} (built-in)"),
        builtin: true,
        known_kind: true,
        shown: shown(kind),
        hideable: can_hide(kind),
    });
    let profiles = view
        .iter()
        .filter(|(name, _)| !kinds.contains(name))
        .map(|(name, _)| {
            let kind = view.kind_of(name);
            let known_kind = kinds.contains(&kind.as_str());
            ProfileRow {
                name: name.to_string(),
                label: if known_kind {
                    format!("{name} ({kind})")
                } else {
                    format!("{name} ({kind}, unknown kind)")
                },
                builtin: false,
                known_kind,
                shown: shown(name),
                hideable: known_kind,
            }
        });
    builtins.chain(profiles).collect()
}

/// The built-in kinds of each picker axis, in [`AXES`] order.
const AXIS_KINDS: [&[&str]; 3] = [
    TRANSLATION_PROVIDERS,
    DICTIONARY_PROVIDERS,
    SPEECH_PROVIDERS,
];

/// Whether "Show in lists" may hide `name`: anything but the first built-in kind of an
/// axis (`google`), which pickers fall back to, so every picker keeps an entry.
pub fn can_hide(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    !AXIS_KINDS
        .iter()
        .any(|kinds| kinds.first() == Some(&name.as_str()))
}

/// The entries of a picker for the axis with built-in `kinds`: the built-ins, then the
/// profiles of `view` whose kind is one of them, minus the `hidden` names, except
/// `keep` (what the picker selects, so the selection never vanishes) and those
/// [`can_hide`] refuses.
pub fn picker_entries(
    kinds: &[&str],
    view: &ProviderProfiles,
    hidden: &[String],
    keep: &str,
) -> Vec<String> {
    let keep = keep.trim().to_lowercase();
    kinds
        .iter()
        .map(|kind| kind.to_string())
        .chain(view.profiles_of_kinds(kinds))
        .filter(|name| *name == keep || !can_hide(name) || !hidden.contains(name))
        .collect()
}

/// What the translation, dictionary and speech pickers should select once `view` is in
/// effect: the `selected` profile while it's still offered on that axis, else the axis's
/// first built-in kind. Also returns a note naming each picker that fell back, or `""`.
pub fn picker_fallbacks(selected: &[String; 3], view: &ProviderProfiles) -> ([String; 3], String) {
    let mut notes: Vec<String> = Vec::new();
    let next: [String; 3] = std::array::from_fn(|axis| {
        let kinds = AXIS_KINDS[axis];
        let name = selected[axis].trim().to_lowercase();
        let offered =
            kinds.contains(&name.as_str()) || view.profiles_of_kinds(kinds).contains(&name);
        if offered {
            return name;
        }
        let fallback = kinds.first().copied().unwrap_or_default().to_string();
        notes.push(format!("{} now uses {fallback}", AXES[axis]));
        fallback
    });
    let note = if notes.is_empty() {
        String::new()
    } else {
        let text = notes.join(", ");
        let mut chars = text.chars();
        let first = chars.next().map(|c| c.to_uppercase().to_string());
        format!("{}{}.", first.unwrap_or_default(), chars.as_str())
    };
    (next, note)
}

/// The axes ([`AXES`]) provider kind `kind` implements, in that order: what a "Test" of
/// one of its profiles calls.
pub fn axes_of(kind: &str) -> Vec<&'static str> {
    let kind = kind.trim().to_lowercase();
    AXES.iter()
        .zip(AXIS_KINDS)
        .filter(|(_, kinds)| kinds.contains(&kind.as_str()))
        .map(|(axis, _)| *axis)
        .collect()
}

/// The longest test result shown, in characters; a longer one is cut with `…`.
const TEST_DETAIL_CHARS: usize = 120;

/// One line of a "Test" result: `"translation: OK (0.9 s): Hallo, Welt!"` or
/// `"translation: failed: <error>"`, on one line and at most [`TEST_DETAIL_CHARS`]
/// characters of detail.
pub fn format_test_line(
    axis: &str,
    elapsed: std::time::Duration,
    result: &Result<String, impl std::fmt::Display>,
) -> String {
    let one_line = |text: &str| -> String {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.chars().count() > TEST_DETAIL_CHARS {
            let cut: String = text.chars().take(TEST_DETAIL_CHARS).collect();
            format!("{}…", cut.trim_end())
        } else {
            text
        }
    };
    match result {
        Ok(detail) => format!(
            "{axis}: OK ({:.1} s): {}",
            elapsed.as_secs_f64(),
            one_line(detail)
        ),
        Err(error) => format!("{axis}: failed: {}", one_line(&error.to_string())),
    }
}

/// The note shown in the options panel when the Providers tab's pickers select
/// `profile`, or empty.
pub fn selection_note(selected: &[&str; 3], profile: &str) -> String {
    let profile = profile.trim().to_lowercase();
    let axes: Vec<&str> = selected
        .iter()
        .zip(AXES)
        .filter(|(name, _)| name.trim().to_lowercase() == profile)
        .map(|(_, axis)| axis)
        .collect();
    if axes.is_empty() {
        String::new()
    } else {
        format!("Selected for {}.", axes.join(" and "))
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

    /// The OpenAI-compatible provider comes from `tagent`'s `openai` feature, which this
    /// crate enables: offered in the translation picker, with `endpoint` and `model`
    /// required and `api_key` a password field, with no GUI code of its own.
    #[test]
    fn openai_needs_endpoint_and_model_and_hides_its_key() {
        assert!(tagent::providers::TRANSLATION_PROVIDERS.contains(&"openai"));
        let mut profiles = ProviderProfiles::new();
        profiles.insert("ollama", "type", "openai");
        let rows = fields(&profiles, &["ollama"], &Edits::new(), no_env);
        assert_eq!(
            labels(&rows),
            [
                "ollama (openai)",
                "endpoint",
                "model",
                "api_key",
                "temperature",
                "translate_prompt",
                "timeout_secs",
                "max_retries",
                "response_format",
                "dictionary_prompt"
            ]
        );
        let row = |key: &str| rows.iter().find(|row| row.label == key).unwrap();
        assert!(row("endpoint").required && !row("endpoint").secret);
        assert!(row("model").required && !row("model").secret);
        assert!(row("api_key").secret && !row("api_key").required);
        assert!(!row("translate_prompt").secret && !row("translate_prompt").required);
        assert!(!row("response_format").required && !row("response_format").multiline);
        assert_eq!(
            missing_required(&profiles, "ollama", &Edits::new(), no_env),
            ["endpoint", "model"]
        );
    }

    /// Both prompts (`translate_prompt` and, from the dictionary axis, `dictionary_prompt`)
    /// get the multi-line editor, pre-filled from the library's defaults; a default never
    /// becomes the field's value.
    #[test]
    fn openai_prompts_are_multiline_with_the_built_in_defaults() {
        let rows = fields(&ProviderProfiles::new(), &["openai"], &Edits::new(), no_env);
        let prompts = ["translate_prompt", "dictionary_prompt"];
        for (key, default) in prompts.into_iter().zip([
            tagent::providers::openai::DEFAULT_TRANSLATE_PROMPT,
            tagent::providers::openai::DEFAULT_DICTIONARY_PROMPT,
        ]) {
            let prompt = rows.iter().find(|row| row.label == key).unwrap();
            assert!(prompt.multiline, "{key}");
            assert_eq!(prompt.default, default);
            assert_eq!(prompt.value, "");
            // The shipped defaults have {to}, so they never warn about themselves.
            assert_eq!(soft_warning(&prompt.default, ""), "");
            assert_eq!(soft_warning(&prompt.default, &prompt.default), "");
        }
        assert!(rows
            .iter()
            .filter(|row| !prompts.contains(&row.label.as_str()))
            .all(|row| !row.multiline && row.default.is_empty()));
    }

    #[test]
    fn google_fields_are_single_line_without_defaults() {
        let rows = fields(&ProviderProfiles::new(), &["google"], &Edits::new(), no_env);
        assert!(rows
            .iter()
            .all(|row| !row.multiline && row.default.is_empty()));
    }

    #[test]
    fn secret_wins_over_multiline() {
        assert!(is_multiline(true, false));
        assert!(!is_multiline(true, true));
        assert!(!is_multiline(false, false));
        assert!(!is_multiline(false, true));
    }

    #[test]
    fn edits_equal_to_the_default_are_recorded_as_empty() {
        let default = "Translate {from} into {to}.\nOnly the translation.";
        assert_eq!(normalize_edit(default, default), "");
        assert_eq!(
            normalize_edit(
                default,
                "  Translate {from} into {to}.\r\nOnly the translation.\n"
            ),
            ""
        );
        assert_eq!(normalize_edit(default, ""), "");
        assert_eq!(
            normalize_edit(default, "Translate into {to}.\r\nBe brief."),
            "Translate into {to}.\nBe brief."
        );
        // Without a default nothing collapses (an empty edit stays empty anyway).
        assert_eq!(normalize_edit("", " "), " ");
        assert_eq!(normalize_edit("", "x"), "x");
    }

    #[test]
    fn soft_warning_flags_a_prompt_without_to() {
        let default = "Translate {from} into {to}.";
        assert!(soft_warning(default, "Translate into German.").contains("{to}"));
        assert_eq!(soft_warning(default, "Translate into {to}."), "");
        // Blank = the default, which has {to}.
        assert_eq!(soft_warning(default, "  \n"), "");
        // A missing {from} is harmless.
        assert_eq!(soft_warning(default, "Into {to}."), "");
        // A default without the placeholder never asks for it.
        assert_eq!(soft_warning("Be brief.", "Anything"), "");
        assert_eq!(soft_warning("", "Anything"), "");
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

    fn openai_profile(name: &str) -> ProviderProfiles {
        let mut profiles = ProviderProfiles::new();
        profiles.insert(name, "type", "openai");
        profiles.insert(name, "endpoint", "http://localhost:11434/v1");
        profiles.insert(name, "model", "old");
        profiles
    }

    #[test]
    fn draft_add_inserts_only_the_type() {
        let mut draft = Draft::default();
        draft.add(" Local ", "OpenAI");
        let view = draft.view(&ProviderProfiles::new());
        assert_eq!(view.get("local").unwrap().len(), 1);
        assert_eq!(view.kind_of("local"), "openai");
    }

    #[test]
    fn draft_delete_removes_the_profile_and_keeps_the_rest() {
        let mut saved = openai_profile("local");
        saved.insert("work", "type", "google");
        saved.insert("work", "hand_edited", "kept");
        let mut draft = Draft::default();
        draft
            .edits
            .insert(("local".into(), "model".into()), "new".into());
        draft.delete("LOCAL");
        assert!(draft.edits.is_empty());
        let view = draft.view(&saved);
        assert!(view.get("local").is_none());
        assert_eq!(view.get("work").unwrap()["hand_edited"], "kept");
        // The saved profiles themselves are untouched until the dialog's OK.
        assert!(saved.get("local").is_some());
    }

    #[test]
    fn draft_delete_then_add_gives_a_fresh_profile() {
        let saved = openai_profile("local");
        let mut draft = Draft::default();
        draft.delete("local");
        draft.add("local", "openai");
        draft
            .edits
            .insert(("local".into(), "model".into()), "fresh".into());
        let view = draft.view(&saved);
        let local = view.get("local").unwrap();
        assert_eq!(local.get("endpoint"), None);
        assert_eq!(local["model"], "fresh");
        assert_eq!(local["type"], "openai");
    }

    #[test]
    fn draft_deleting_an_added_profile_forgets_it() {
        let mut draft = Draft::default();
        draft.add("local", "openai");
        draft
            .edits
            .insert(("local".into(), "model".into()), "m".into());
        draft.delete("local");
        assert_eq!(draft, Draft::default());
    }

    #[test]
    fn draft_ignores_edits_of_a_deleted_profile() {
        let mut draft = Draft::default();
        draft.deleted.insert("local".into());
        draft
            .edits
            .insert(("local".into(), "model".into()), "m".into());
        assert!(draft.view(&openai_profile("local")).is_empty());
    }

    /// A hand-edited profile with no options at all (`"llm": {}`) can be deleted too.
    #[test]
    fn draft_deletes_an_empty_profile() {
        let saved: ProviderProfiles = serde_json::from_str(r#"{"llm": {}}"#).unwrap();
        assert!(saved.get("llm").is_some());
        let mut draft = Draft::default();
        draft.delete("llm");
        assert!(draft.view(&saved).is_empty());
    }

    #[test]
    fn name_error_rules() {
        let view = openai_profile("local");
        assert_eq!(name_error(&view, "ollama", "openai"), "");
        assert_eq!(name_error(&view, " Ollama-2 ", "openai"), "");
        assert_eq!(
            name_error(&view, "  ", "openai"),
            "Enter a name for the profile."
        );
        assert_eq!(
            name_error(&view, "Google", "openai"),
            "`google` is a built-in provider: it is already in the list."
        );
        assert!(name_error(&view, "my llm", "openai").contains("invalid profile name"));
        assert_eq!(
            name_error(&view, "LOCAL", "openai"),
            "A profile named `local` already exists."
        );
    }

    #[test]
    fn builtin_kinds_follow_the_registry() {
        assert_eq!(builtin_kinds(), ["google", "deepl", "openai"]);
    }

    #[test]
    fn profile_rows_list_builtins_then_profiles() {
        let mut view = openai_profile("local");
        view.insert("google", "max_retries", "0");
        view.insert("llm", "type", "nope");
        let rows = profile_rows(&view, &["deepl".to_string(), "llm".to_string()]);
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "google (built-in)",
                "deepl (built-in)",
                "openai (built-in)",
                "llm (nope, unknown kind)",
                "local (openai)"
            ]
        );
        assert!(rows[..3].iter().all(|row| row.builtin && row.known_kind));
        assert!(!rows[3].builtin && !rows[3].known_kind);
        assert!(!rows[4].builtin && rows[4].known_kind);
        assert_eq!(rows[4].name, "local");
        // "Show in lists": google can't be hidden; an unknown kind has no checkbox.
        let shown: Vec<(bool, bool)> = rows.iter().map(|row| (row.shown, row.hideable)).collect();
        assert_eq!(
            shown,
            [
                (true, false),
                (false, true),
                (true, true),
                (false, false),
                (true, true)
            ]
        );
    }

    #[test]
    fn can_hide_refuses_only_the_fallback_kind() {
        assert!(!can_hide("Google"));
        assert!(can_hide("deepl"));
        assert!(can_hide("openai"));
        assert!(can_hide("local"));
    }

    #[test]
    fn picker_entries_drop_hidden_names_but_keep_the_selection_and_google() {
        let mut view = openai_profile("local");
        view.insert("remote", "type", "openai");
        view.insert("work", "type", "google");
        let hidden: Vec<String> = ["google", "deepl", "local", "remote", "work"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            picker_entries(TRANSLATION_PROVIDERS, &view, &hidden, "Remote"),
            ["google", "openai", "remote"]
        );
        assert_eq!(
            picker_entries(TRANSLATION_PROVIDERS, &view, &[], ""),
            ["google", "deepl", "openai", "local", "remote", "work"]
        );
        // Profiles of kinds without that axis aren't offered there.
        assert_eq!(
            picker_entries(SPEECH_PROVIDERS, &view, &[], "google"),
            ["google", "work"]
        );
    }

    #[test]
    fn picker_fallbacks_reset_pickers_whose_profile_is_gone() {
        let mut view = ProviderProfiles::new();
        view.insert("work", "type", "google");
        let selected = [
            "local".to_string(),
            "Work".to_string(),
            "google".to_string(),
        ];
        let (next, note) = picker_fallbacks(&selected, &view);
        assert_eq!(next, ["google", "work", "google"]);
        assert_eq!(note, "Translation now uses google.");

        let selected = ["gone".to_string(), "gone".to_string(), "deepl".to_string()];
        let (next, note) = picker_fallbacks(&selected, &view);
        assert_eq!(next, ["google", "google", "google"]);
        assert_eq!(
            note,
            "Translation now uses google, dictionary now uses google, speech now uses google."
        );

        let selected = ["deepl".to_string(), "work".to_string(), "work".to_string()];
        assert_eq!(
            picker_fallbacks(&selected, &view),
            (selected.clone(), String::new())
        );
    }

    #[test]
    fn selection_note_names_the_pickers_that_select_the_profile() {
        let selected = ["local", "google", "LOCAL"];
        assert_eq!(
            selection_note(&selected, "local"),
            "Selected for translation and speech."
        );
        assert_eq!(selection_note(&selected, "deepl"), "");
    }

    #[test]
    fn axes_of_follows_the_compiled_in_kinds() {
        assert_eq!(axes_of("google"), ["translation", "dictionary", "speech"]);
        assert_eq!(axes_of("DeepL"), ["translation"]);
        assert_eq!(axes_of("openai"), ["translation", "dictionary"]);
        assert!(axes_of("nope").is_empty());
    }

    #[test]
    fn format_test_line_shows_time_and_result_on_one_line() {
        use std::time::Duration;
        let ok: Result<String, String> = Ok("Hallo,\n  Welt!".into());
        assert_eq!(
            format_test_line("translation", Duration::from_millis(940), &ok),
            "translation: OK (0.9 s): Hallo, Welt!"
        );
        let failed: Result<String, String> = Err("HTTP 401 Unauthorized".into());
        assert_eq!(
            format_test_line("speech", Duration::from_secs(3), &failed),
            "speech: failed: HTTP 401 Unauthorized"
        );
        let long: Result<String, String> = Ok("x".repeat(300));
        let line = format_test_line("dictionary", Duration::ZERO, &long);
        assert!(line.ends_with('…'));
        assert_eq!(line.chars().count(), "dictionary: OK (0.0 s): ".len() + 121);
    }
}
