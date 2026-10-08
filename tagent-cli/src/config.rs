use crate::platform::keycodes;
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tagent::providers::{
    self, DictionaryProvider, OptionSpec, ProviderAxis, ProviderDescriptor, ProviderOptions,
    ProviderProfiles, SpeechProvider, TranslationProvider,
};
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

mod upgrade;

/// Runtime configuration loaded from `tagent-cli.toml`.
///
/// All fields correspond directly to TOML keys documented inside the generated
/// configuration file (see [`ConfigFile`] for the on-disk layout). The [`Default`] impl
/// reflects the same defaults that are written when a new configuration file is created.
///
/// The config file is located at:
/// - **Windows**: `%APPDATA%\tagent-cli\tagent-cli.toml`
/// - **Linux**: `~/.config/tagent-cli/tagent-cli.toml`
/// - **macOS**: `~/Library/Application Support/tagent-cli/tagent-cli.toml`
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Language code of the source language (e.g. `"en"`), or `"auto"` for
    /// auto-detection. A language `tagent`'s table lists is always its code (see
    /// [`language_code`]); any other value is kept as written and passed to the provider.
    pub source_language: String,
    /// Language code of the translation target (e.g. `"ru"`), never `"auto"`. Same rules
    /// as [`Self::source_language`].
    pub target_language: String,
    /// Bring the terminal window to the foreground when a translation fires.
    pub show_terminal_on_translate: bool,
    /// Seconds before the terminal window auto-hides after a translation. `0` disables auto-hide.
    pub auto_hide_terminal_seconds: u64,
    /// Show a full dictionary entry instead of a plain translation for single words.
    pub show_dictionary: bool,
    /// Automatically correct spelling of single-word input before looking up.
    pub spell_check: bool,
    /// Name of the dictionary backend to use, e.g. `"google"`. Independent of
    /// `translate_provider`.
    pub dictionary_provider: String,
    /// Copy the translation result to the system clipboard automatically.
    pub copy_to_clipboard: bool,
    /// Append every translation to the history file.
    pub save_translation_history: bool,
    /// Path to the history log file.
    pub history_file: String,
    /// Whether terminal output is colored at all: `"auto"`, `"always"` or `"never"`, see
    /// [`UseColors`]. Kept as written; an unknown value acts as `"auto"`.
    pub use_colors: String,
    /// Terminal color for the target-language prompt (e.g. `"BrightYellow"`). `"None"` disables.
    pub target_prompt_color: String,
    /// Terminal color for the dictionary prompt. `"None"` disables.
    pub dictionary_prompt_color: String,
    /// Terminal color for the source-language prompt. `"None"` disables.
    pub source_prompt_color: String,
    /// Terminal color for part-of-speech labels in a dictionary article. `"None"` disables.
    pub part_of_speech_color: String,
    /// Terminal color for `[synonym, ...]` brackets in a dictionary article. `"None"` disables.
    pub synonym_color: String,
    /// Terminal color for the spelling-correction notice. `"None"` disables.
    pub notice_color: String,
    /// Terminal color for translation, speech, clipboard and history error messages.
    /// `"None"` disables.
    pub error_color: String,
    /// Hotkey string for triggering translation, e.g. `"Alt+Q"`, `"Ctrl+Ctrl"`, `"F9"`.
    pub translate_hotkey: String,
    /// Enable text-to-speech playback of translations.
    pub enable_text_to_speech: bool,
    /// Hotkey string for triggering speech playback, e.g. `"Alt+S"`.
    pub speech_hotkey: String,
    /// Enable the speech hotkey. When `false`, the hotkey is registered but inactive.
    pub enable_speech_hotkey: bool,
    /// Name of the translation backend to use, e.g. `"google"`.
    pub translate_provider: String,
    /// Name of the text-to-speech backend to use, e.g. `"google"`. Independent of
    /// `translate_provider`.
    pub speech_provider: String,
    /// Provider profiles from the `[provider_options.<name>]` tables. Its `Debug` masks
    /// secret values.
    pub provider_options: ProviderProfiles,
}

impl Config {
    /// The source language's display name (`"Auto"`, `"English"`), or its code if
    /// `tagent`'s table doesn't list it.
    pub fn source_language_name(&self) -> &str {
        tagent::languages::code_to_name(&self.source_language)
    }

    /// The options for provider profile `name`: its `[provider_options.<name>]` table, with
    /// `TAGENT_<NAME>_<KEY>` environment variables taking precedence.
    pub fn provider_options(&self, name: &str) -> ProviderOptions {
        self.provider_options_using(name, |var| std::env::var(var).ok())
    }

    /// [`Self::provider_options`] with the environment replaced by `lookup`.
    fn provider_options_using(
        &self,
        name: &str,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> ProviderOptions {
        self.provider_options.options_using(name, lookup)
    }

    /// Builds the translation provider for `translate_provider`, with its profile's options.
    /// The error is ready to show.
    pub fn create_translate_provider(&self) -> Result<Box<dyn TranslationProvider>, String> {
        let name = &self.translate_provider;
        providers::create_provider_with(name, &self.provider_options(name)).map_err(|e| {
            provider_error_message(
                &e,
                "translate_provider",
                name,
                providers::TRANSLATION_PROVIDERS,
            )
        })
    }

    /// Builds the dictionary provider for `dictionary_provider`, with its profile's options.
    /// The error is ready to show.
    pub fn create_dictionary_provider(&self) -> Result<Box<dyn DictionaryProvider>, String> {
        let name = &self.dictionary_provider;
        providers::create_dictionary_provider_with(name, &self.provider_options(name)).map_err(
            |e| {
                provider_error_message(
                    &e,
                    "dictionary_provider",
                    name,
                    providers::DICTIONARY_PROVIDERS,
                )
            },
        )
    }

    /// Builds the speech provider for `speech_provider`, with its profile's options. The
    /// error is ready to show.
    pub fn create_speech_provider(&self) -> Result<Box<dyn SpeechProvider>, String> {
        let name = &self.speech_provider;
        providers::create_speech_provider_with(name, &self.provider_options(name)).map_err(|e| {
            provider_error_message(&e, "speech_provider", name, providers::SPEECH_PROVIDERS)
        })
    }

    /// Builds the provider of `axis` (see [`Self::create_translate_provider`] and its
    /// twins) and returns its display name, e.g. to check a choice before switching to it.
    /// Building makes no network call. The error is ready to show.
    pub fn build_provider_name(&self, axis: ProviderAxis) -> Result<String, String> {
        match axis {
            ProviderAxis::Translation => self
                .create_translate_provider()
                .map(|provider| provider.name().to_string()),
            ProviderAxis::Dictionary => self
                .create_dictionary_provider()
                .map(|provider| provider.name().to_string()),
            ProviderAxis::Speech => self
                .create_speech_provider()
                .map(|provider| provider.name().to_string()),
        }
    }

    /// The `*_provider` value in effect on `axis`: `translate_provider`,
    /// `dictionary_provider` or `speech_provider`.
    pub fn provider_name(&self, axis: ProviderAxis) -> &str {
        match axis {
            ProviderAxis::Translation => &self.translate_provider,
            ProviderAxis::Dictionary => &self.dictionary_provider,
            ProviderAxis::Speech => &self.speech_provider,
        }
    }

    /// The setting that turns `axis` off, if it is off: `show_dictionary = false` or
    /// `enable_text_to_speech = false` (translation can't be turned off).
    pub fn axis_disabled_by(&self, axis: ProviderAxis) -> Option<&'static str> {
        match axis {
            ProviderAxis::Translation => None,
            ProviderAxis::Dictionary => (!self.show_dictionary).then_some("show_dictionary"),
            ProviderAxis::Speech => {
                (!self.enable_text_to_speech).then_some("enable_text_to_speech")
            }
        }
    }

    /// The providers `/p` offers, axis by axis (translation, dictionary, speech): for each,
    /// every kind compiled in for that axis, then the profiles of those kinds, with the one
    /// in effect marked. Nothing is built, so this makes no network calls and needs no
    /// valid options.
    pub fn provider_list(&self) -> Vec<ProviderListEntry> {
        self.provider_list_using(|var| std::env::var(var).ok())
    }

    /// [`Self::provider_list`] with the environment replaced by `lookup`.
    fn provider_list_using(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Vec<ProviderListEntry> {
        let mut entries = Vec::new();
        for axis in ProviderAxis::ALL {
            let kinds = axis.kinds();
            let names = kinds
                .iter()
                .map(|kind| kind.to_string())
                .chain(self.provider_options.profiles_of_kinds(kinds));
            for name in names {
                let kind = self.provider_options.kind_of(&name);
                let descriptor = axis
                    .descriptors()
                    .iter()
                    .find(|descriptor| descriptor.name == kind);
                let display_name = match descriptor {
                    Some(descriptor) if name == kind => descriptor.display_name.to_string(),
                    Some(descriptor) => format!("{} ({name})", descriptor.display_name),
                    None => name.clone(),
                };
                let options = self.provider_options_using(&name, &lookup);
                let missing = descriptor
                    .map(|descriptor| {
                        descriptor
                            .options
                            .iter()
                            .filter(|option| option.required)
                            .filter(|option| {
                                options.get(option.key).is_none_or(|v| v.trim().is_empty())
                            })
                            .map(|option| option.key.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                entries.push(ProviderListEntry {
                    axis,
                    active: name.eq_ignore_ascii_case(self.provider_name(axis)),
                    name,
                    display_name,
                    missing,
                });
            }
        }
        entries
    }

    /// The profiles `/config` describes: every `[provider_options.<name>]` table plus the three
    /// selected providers, sorted, without duplicates.
    fn profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .provider_options
            .iter()
            .map(|(name, _)| name.to_string())
            .chain(
                [
                    &self.translate_provider,
                    &self.dictionary_provider,
                    &self.speech_provider,
                ]
                .iter()
                .map(|name| name.to_lowercase()),
            )
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The `/config` lines describing each provider profile's effective options, as
    /// `[provider_options.<name>]` tables: secret values masked, and a value taken from an
    /// environment variable marked with a comment naming it.
    fn provider_profile_lines(&self, lookup: impl Fn(&str) -> Option<String>) -> Vec<String> {
        let mut lines = Vec::new();
        for name in self.profile_names() {
            let options = self.provider_options_using(&name, &lookup);
            let kind = options
                .get("type")
                .map(|kind| kind.trim().to_lowercase())
                .unwrap_or_else(|| name.clone());
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!(
                "[provider_options.{}]",
                toml_edit::Key::new(name.as_str())
            ));
            if options.is_empty() {
                lines.push("# (no options)".to_string());
            }
            // `type` first: it says what the rest of the options configure.
            let (kind_option, other_options): (Vec<_>, Vec<_>) =
                options.iter().partition(|(key, _)| *key == "type");
            for (key, value) in kind_option.into_iter().chain(other_options) {
                let shown = if providers::is_secret_option(&kind, key) {
                    mask_secret(value)
                } else {
                    value.to_string()
                };
                let var = providers::env_var_name(&name, key);
                let origin = if lookup(&var).is_some_and(|v| !v.is_empty()) {
                    format!("  # from env {var}")
                } else {
                    String::new()
                };
                lines.push(format!("{key} = {}{origin}", Value::from(shown)));
            }
        }
        lines
    }

    /// The `/config` lines for every setting, as `[section]` headers and `key = value`
    /// lines named and formatted as in `tagent-cli.toml` (the language settings followed by
    /// their name as a comment, when the table has one), then the provider profiles
    /// ([`Self::provider_profile_lines`]).
    fn display_lines(&self, lookup: impl Fn(&str) -> Option<String>) -> Vec<String> {
        let values = toml_edit::ser::to_document(&ConfigFile::from(self))
            .expect("the configuration serializes to TOML");
        let mut lines = Vec::new();
        for (section, item) in values.iter() {
            let Some(table) = item.as_table_like() else {
                continue;
            };
            if section == "provider_options" {
                continue;
            }
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            for (key, value) in table.iter() {
                let Some(value) = value.as_value() else {
                    continue;
                };
                let mut value = value.clone();
                value.decor_mut().clear();
                let code = match (section, key) {
                    ("translation", "source_language") => Some(&self.source_language),
                    ("translation", "target_language") => Some(&self.target_language),
                    _ => None,
                };
                let note = match code {
                    Some(code) if tagent::languages::code_to_name(code) != code => {
                        format!("  # {}", tagent::languages::code_to_name(code))
                    }
                    _ => String::new(),
                };
                lines.push(format!("{key} = {value}{note}"));
            }
        }
        lines.push(String::new());
        lines.push("# Provider profiles (effective options, secrets masked)".to_string());
        lines.extend(self.provider_profile_lines(lookup));
        lines
    }
}

/// Masks a secret for display: `••••` plus its last 4 characters, or just `••••` when it's
/// too short for that to hide most of it.
fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() < 12 {
        "••••".to_string()
    } else {
        format!(
            "••••{}",
            chars[chars.len() - 4..].iter().collect::<String>()
        )
    }
}

/// Writes the config file. On Unix it is created, or tightened, to mode `0600` before
/// anything is written, since it can hold API keys; on Windows `%APPDATA%` is already
/// per-user.
fn write_config_file(path: &str, content: &str) -> std::io::Result<()> {
    let mut open = OpenOptions::new();
    open.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let mut file = open.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(content.as_bytes())
}

/// The on-disk layout of `tagent-cli.toml`: one table per section, plus the provider
/// profiles under `provider_options`. A missing section or key takes
/// [`Config::default`]'s value; unknown keys are ignored when reading (and kept on disk,
/// since `/save` edits the file in place).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ConfigFile {
    provider: ProviderSection,
    translation: TranslationSection,
    dictionary: DictionarySection,
    interface: InterfaceSection,
    colors: ColorsSection,
    history: HistorySection,
    hotkeys: HotkeysSection,
    speech: SpeechSection,
    /// Profile name → option key → value. Values must be strings; names and keys are
    /// lowercased by [`ProviderProfiles`].
    provider_options: ProviderProfiles,
}

/// `[provider]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct ProviderSection {
    translate_provider: String,
}

/// `[translation]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct TranslationSection {
    source_language: String,
    target_language: String,
}

/// `[dictionary]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct DictionarySection {
    show_dictionary: bool,
    spell_check: bool,
    dictionary_provider: String,
}

/// `[interface]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct InterfaceSection {
    show_terminal_on_translate: bool,
    auto_hide_terminal_seconds: u64,
    copy_to_clipboard: bool,
}

/// `[colors]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct ColorsSection {
    use_colors: String,
    source_prompt_color: String,
    target_prompt_color: String,
    dictionary_prompt_color: String,
    part_of_speech_color: String,
    synonym_color: String,
    notice_color: String,
    error_color: String,
}

/// `[history]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct HistorySection {
    save_translation_history: bool,
    history_file: String,
}

/// `[hotkeys]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct HotkeysSection {
    translate_hotkey: String,
}

/// `[speech]`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct SpeechSection {
    enable_text_to_speech: bool,
    speech_hotkey: String,
    enable_speech_hotkey: bool,
    speech_provider: String,
}

/// Each section's defaults are [`Config::default`]'s, so they are spelled out only once.
macro_rules! section_defaults_from_config {
    ($($section:ident => $field:ident),* $(,)?) => {
        $(
            impl Default for $section {
                fn default() -> Self {
                    ConfigFile::from(&Config::default()).$field
                }
            }
        )*
    };
}

section_defaults_from_config!(
    ProviderSection => provider,
    TranslationSection => translation,
    DictionarySection => dictionary,
    InterfaceSection => interface,
    ColorsSection => colors,
    HistorySection => history,
    HotkeysSection => hotkeys,
    SpeechSection => speech,
);

impl From<&Config> for ConfigFile {
    fn from(config: &Config) -> Self {
        let config = config.clone();
        Self {
            provider: ProviderSection {
                translate_provider: config.translate_provider,
            },
            translation: TranslationSection {
                source_language: config.source_language,
                target_language: config.target_language,
            },
            dictionary: DictionarySection {
                show_dictionary: config.show_dictionary,
                spell_check: config.spell_check,
                dictionary_provider: config.dictionary_provider,
            },
            interface: InterfaceSection {
                show_terminal_on_translate: config.show_terminal_on_translate,
                auto_hide_terminal_seconds: config.auto_hide_terminal_seconds,
                copy_to_clipboard: config.copy_to_clipboard,
            },
            colors: ColorsSection {
                use_colors: config.use_colors,
                source_prompt_color: config.source_prompt_color,
                target_prompt_color: config.target_prompt_color,
                dictionary_prompt_color: config.dictionary_prompt_color,
                part_of_speech_color: config.part_of_speech_color,
                synonym_color: config.synonym_color,
                notice_color: config.notice_color,
                error_color: config.error_color,
            },
            history: HistorySection {
                save_translation_history: config.save_translation_history,
                history_file: config.history_file,
            },
            hotkeys: HotkeysSection {
                translate_hotkey: config.translate_hotkey,
            },
            speech: SpeechSection {
                enable_text_to_speech: config.enable_text_to_speech,
                speech_hotkey: config.speech_hotkey,
                enable_speech_hotkey: config.enable_speech_hotkey,
                speech_provider: config.speech_provider,
            },
            provider_options: config.provider_options,
        }
    }
}

impl From<ConfigFile> for Config {
    fn from(file: ConfigFile) -> Self {
        Self {
            source_language: file.translation.source_language,
            target_language: file.translation.target_language,
            show_terminal_on_translate: file.interface.show_terminal_on_translate,
            auto_hide_terminal_seconds: file.interface.auto_hide_terminal_seconds,
            show_dictionary: file.dictionary.show_dictionary,
            spell_check: file.dictionary.spell_check,
            dictionary_provider: file.dictionary.dictionary_provider,
            copy_to_clipboard: file.interface.copy_to_clipboard,
            save_translation_history: file.history.save_translation_history,
            history_file: file.history.history_file,
            use_colors: file.colors.use_colors,
            target_prompt_color: file.colors.target_prompt_color,
            dictionary_prompt_color: file.colors.dictionary_prompt_color,
            source_prompt_color: file.colors.source_prompt_color,
            part_of_speech_color: file.colors.part_of_speech_color,
            synonym_color: file.colors.synonym_color,
            notice_color: file.colors.notice_color,
            error_color: file.colors.error_color,
            translate_hotkey: file.hotkeys.translate_hotkey,
            enable_text_to_speech: file.speech.enable_text_to_speech,
            speech_hotkey: file.speech.speech_hotkey,
            enable_speech_hotkey: file.speech.enable_speech_hotkey,
            translate_provider: file.provider.translate_provider,
            speech_provider: file.speech.speech_provider,
            provider_options: file.provider_options,
        }
    }
}

/// Parses the contents of `tagent-cli.toml`, see [`parse_config_with_warnings`]; the
/// warnings are dropped.
fn parse_config(content: &str) -> Result<Config, toml_edit::de::Error> {
    parse_config_with_warnings(content).map(|(config, _)| config)
}

/// Parses the contents of `tagent-cli.toml`, with the warnings about its language
/// settings ([`normalize_languages`]). Those are changed in memory only; the file itself
/// is left untouched.
///
/// The error (a syntax error, or a value of the wrong type) names the line and column
/// and shows the offending line.
fn parse_config_with_warnings(
    content: &str,
) -> Result<(Config, Vec<String>), toml_edit::de::Error> {
    let file: ConfigFile = toml_edit::de::from_str(content)?;
    let mut config = Config::from(file);
    let mut warnings = normalize_languages(&mut config);
    if UseColors::parse(&config.use_colors).is_none() {
        warnings.push(format!(
            "use_colors = {:?} is not one of auto, always, never; auto is used",
            config.use_colors
        ));
    }
    Ok((config, warnings))
}

/// Turns `source_language`/`target_language` as read from the file (a code or a name, in
/// any case) into codes ([`language_code`]), and replaces an `Auto` target with
/// [`AUTO_TARGET_FALLBACK`]. A value `tagent`'s table doesn't list is kept as written and
/// used as a language code. Returns the warnings to show, without the file name.
fn normalize_languages(config: &mut Config) -> Vec<String> {
    let mut warnings = Vec::new();
    for (key, value) in [
        ("source_language", &mut config.source_language),
        ("target_language", &mut config.target_language),
    ] {
        match tagent::languages::language_code(value) {
            Some(code) => *value = code.to_string(),
            None => warnings.push(format!(
                "{key} = {value:?} is not a known language, used as a language code"
            )),
        }
    }
    let resolved = LanguagePair::new(&config.source_language, &config.target_language);
    warnings.extend(
        resolved
            .notices
            .iter()
            .map(|notice| format!("{notice} (target_language)")),
    );
    config.source_language = resolved.source;
    config.target_language = resolved.target;
    warnings
}

/// The code `tagent`'s table lists for `input` (a language code or name, in any case;
/// `"auto"` for auto-detection), or `input` itself when the table doesn't list it.
pub fn language_code(input: &str) -> String {
    tagent::languages::language_code(input).map_or_else(|| input.to_string(), str::to_string)
}

/// The message for a config file that can't be loaded: the file, then the error with its
/// line and column.
fn invalid_file_message(path: &str, error: &toml_edit::de::Error) -> String {
    format!(
        "invalid configuration file {}:\n{}",
        path,
        error.to_string().trim_end()
    )
}

/// What [`update_config_file`] did, with the lines to report it.
#[derive(Debug)]
pub struct ConfigUpdate {
    path: String,
    outcome: UpdateOutcome,
    /// [`upgrade::UnknownKey::summary`] of every unknown key, left as it is.
    unknown: Vec<String>,
}

#[derive(Debug, PartialEq)]
enum UpdateOutcome {
    /// There was no file; a new one was written.
    Created,
    /// Nothing was missing; the file wasn't touched.
    UpToDate,
    /// Settings were added; the previous file is in `backup`.
    Updated { added: Vec<String>, backup: String },
}

impl ConfigUpdate {
    /// The report, one line per entry.
    pub fn lines(&self) -> Vec<String> {
        let name = Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone());
        let mut lines = Vec::new();
        match &self.outcome {
            UpdateOutcome::Created => {
                lines.push(format!("Created default configuration file: {}", self.path))
            }
            UpdateOutcome::UpToDate => lines.push(format!("{name} is up to date.")),
            UpdateOutcome::Updated { added, backup } => {
                lines.push(format!("Updated {}, added:", self.path));
                lines.extend(added.iter().map(|item| format!("  {item}")));
                lines.push(format!("The previous version is saved as {backup}"));
            }
        }
        if !self.unknown.is_empty() {
            lines.push(format!(
                "Not changed (unknown to this version; fix or remove them in {name}):"
            ));
            lines.extend(self.unknown.iter().map(|item| format!("  {item}")));
        }
        lines
    }
}

/// Brings the config file at `path` up to date with this version's template (`tagent-cli
/// --update-config`, `/config update`): adds missing keys and sections with their
/// comments and defaults, and the example provider profiles, see [`upgrade::upgrade`].
/// Before writing, the file is copied to `<path>.bak`. Nothing is removed or renamed;
/// unknown keys are only listed. An up-to-date file isn't touched (no backup, no write),
/// a missing one is created, and a file the application wouldn't load is left alone with
/// the same error the application shows at startup.
pub fn update_config_file(path: &Path) -> Result<ConfigUpdate, String> {
    let shown = path.display().to_string();
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            write_config_file(&shown, &render_config(&Config::default()))
                .map_err(|e| format!("can't write {shown}: {e}"))?;
            return Ok(ConfigUpdate {
                path: shown,
                outcome: UpdateOutcome::Created,
                unknown: Vec::new(),
            });
        }
        Err(e) => return Err(format!("can't read {shown}: {e}")),
    };
    // The same check as at startup, which a file can fail while being valid TOML.
    toml_edit::de::from_str::<ConfigFile>(&content).map_err(|e| {
        format!(
            "{}\n(the file was not changed)",
            invalid_file_message(&shown, &e)
        )
    })?;
    let unknown = unknown_keys_in(&content)
        .iter()
        .map(upgrade::UnknownKey::summary)
        .collect();
    let upgrade = upgrade::upgrade(&content).map_err(|e| format!("{shown}: {e}"))?;
    let outcome = match upgrade {
        None => UpdateOutcome::UpToDate,
        Some(upgrade) => {
            let backup = format!("{shown}.bak");
            write_config_file(&backup, &content)
                .map_err(|e| format!("can't write the backup {backup}: {e}"))?;
            write_config_file(&shown, &upgrade.content)
                .map_err(|e| format!("can't write {shown}: {e}"))?;
            UpdateOutcome::Updated {
                added: upgrade.added,
                backup,
            }
        }
    };
    Ok(ConfigUpdate {
        path: shown,
        outcome,
        unknown,
    })
}

/// The startup notice for `count` settings missing from the config file, if any.
fn new_settings_notice(count: usize) -> Option<String> {
    let settings = match count {
        0 => return None,
        1 => "1 new setting is".to_string(),
        n => format!("{n} new settings are"),
    };
    Some(format!(
        "Config: {settings} available (run /config update or tagent-cli --update-config)"
    ))
}

/// The keys and sections of `content` (a config file that [`parse_config`] accepted)
/// that this version doesn't know, see [`upgrade::unknown_keys`].
fn unknown_keys_in(content: &str) -> Vec<upgrade::UnknownKey> {
    toml_edit::Document::parse(content.to_string())
        .map(|doc| upgrade::unknown_keys(&doc))
        .unwrap_or_default()
}

/// The commented template a new `tagent-cli.toml` starts from. [`render_config`] fills in
/// the actual values, so the literal ones here only have to be valid.
fn config_template() -> String {
    format!(
        r#"# Text Translator Configuration File (TOML)
# This program translates selected text using keyboard shortcuts
#
# Usage:
# 1. Select text in any application
# 2. Press the translation hotkey (default: Alt+A)
# 3. Translation will be shown (enable copy_to_clipboard below to also copy it)
# 4. Type /q or /e in the interactive prompt to exit the program
#
# Configuration changes take effect immediately (no restart required),
# except where noted. Strings are quoted; true/false and numbers are not.

[provider]
# Translation service provider: a provider profile name (see "Provider profiles"
# at the end of this file); a built-in provider name works without a profile
# Supported values: {translate_providers}
# Default: google
translate_provider = "google"

[translation]
# Languages are language codes:
{language_codes}
# Other codes are passed to the provider as they are (e.g. "uk", "zh-TW").
# Names work too (e.g. "German"); /save writes them as codes.

# Source language for translation; "auto" detects it
# Default: auto
source_language = "auto"

# Target language for translation (not "auto")
# Default: the system language (from the locale), else en
target_language = "{default_target}"

[dictionary]
# Show dictionary entry for single words instead of simple translation
# Set to true to show detailed word information (definitions, part of speech, examples)
# Set to false to always use simple translation
# This feature works best with English words
show_dictionary = true

# Check spelling of single words and suggest the correct word if a typo is detected
# When enabled, misspelled words are automatically corrected and the correction is shown
# Set to false to disable spell checking (typos will fall back to simple translation)
spell_check = true

# Dictionary lookup backend (a provider profile name), independent of translate_provider
# Supported values: {dictionary_providers}
# Default: google
dictionary_provider = "google"

[interface]
# Show terminal window on top when translating
# Set to true to show terminal window during translation
# Set to false to keep terminal in background
show_terminal_on_translate = true

# Auto-hide terminal after translation (in seconds)
# Set to 0 to keep terminal visible (no auto-hide)
# Set to any number > 0 to auto-hide after that many seconds
# Example: 3 = hide terminal after 3 seconds
auto_hide_terminal_seconds = 3

# Automatically copy translation result to clipboard
# Set to true to automatically copy result to clipboard after translation
# Set to false to display result only (without copying to clipboard)
# When enabled, you can paste the result anywhere with Ctrl+V
copy_to_clipboard = false

[colors]
# Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
# BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta,
# BrightCyan, BrightWhite. Use "None" to disable a color.

# Whether the output is colored at all
# auto: only on a terminal that shows colors (not a pipe or a file, not TERM=dumb,
#       not an old Windows console; NO_COLOR or CLICOLOR=0 also turn colors off)
# always: always, even into a pipe or a file
# never: never, as if every color below were "None"
# Default: auto
use_colors = "auto"

# Language pair prompt (e.g., "[auto → ru]: "). Default: None (no color)
source_prompt_color = "None"

# Translation label, the provider that translated (e.g., "[google]: "). Default: BrightYellow
target_prompt_color = "BrightYellow"

# Dictionary prompt (e.g., "[Word]: "). Default: BrightYellow
dictionary_prompt_color = "BrightYellow"

# Colors used inside a dictionary article and for status messages.
# Part-of-speech labels (e.g., "Noun", "Существительное"). Default: Cyan
part_of_speech_color = "Cyan"
# Synonym brackets (e.g., "[fierce, brutal]"). Default: Green
synonym_color = "Green"
# Spelling-correction notice (e.g., "Showing translation for word violent"). Default: Magenta
notice_color = "Magenta"
# Error messages (translation, speech, clipboard, history). Default: Red
error_color = "Red"

[history]
# Save translation history to file
# Set to true to save all translations with timestamps to a text file
# Set to false to disable history logging
# History includes original text, translation, language direction, and timestamp
save_translation_history = false

# History file path
# File where translation history will be saved
# Use an absolute path: a relative one is taken from the folder tagent-cli is started in
# File will be created automatically if it doesn't exist
# On Windows, write backslashes doubled ("C:\\Users\\...") or use single quotes
history_file = ""

[hotkeys]
# Hotkey for translation
# Supported formats:
#   - Single keys: F1-F12 ONLY (other keys must use modifiers)
#   - Modifier combinations: Ctrl, Alt and/or Shift, then a letter, a digit or F1-F12:
#     Alt+Q, Ctrl+Shift+T, Alt+Shift+7, Ctrl+F9, etc.
#     NOT allowed: Shift+Key alone (interferes with text input), Win (Super),
#     Tab, Space, Enter, Esc, Backspace, Delete, Insert, arrows, Home/End/PageUp/PageDown
#     as the last key, and Ctrl+A/C/V/X/Y/Z
#   - Double-press: Ctrl+Ctrl, Shift+Shift, F8+F8 (not Alt+Alt)
# Examples:
#   translate_hotkey = "Alt+A" (default)
#   translate_hotkey = "Ctrl+Ctrl"
#   translate_hotkey = "F9"
#   translate_hotkey = "Ctrl+Shift+C"
#   translate_hotkey = "F8+F8"
# Note: Hotkey changes require application restart to take effect
translate_hotkey = "Alt+A"

[speech]
# Enable text-to-speech functionality
# Set to true to enable TTS for selected text (default)
# Set to false to disable TTS completely
enable_text_to_speech = true

# Hotkey for text-to-speech
# Supported formats (same as translate_hotkey):
#   - Single keys: F1-F12 ONLY
#   - Modifier combinations: Alt+S, Ctrl+Shift+S, etc.
#   - Double-press: Ctrl+Ctrl, Shift+Shift, F8+F8
# Examples:
#   speech_hotkey = "Alt+S"
#   speech_hotkey = "F10"
#   speech_hotkey = "Ctrl+Shift+S"
# Note: Hotkey changes require application restart to take effect
speech_hotkey = "Alt+S"

# Enable or disable the speech hotkey
# Set to true to enable the speech hotkey
# Set to false to disable speech hotkey
enable_speech_hotkey = true

# Speech synthesis backend (a provider profile name), independent of translate_provider
# Supported values: {speech_providers}
# Default: google
speech_provider = "google"

# Provider profiles
# One [provider_options.<name>] table per profile. translate_provider,
# dictionary_provider and speech_provider take a profile name; a built-in name
# (e.g. google) works without a table. The optional `type` key picks the provider
# kind (default: the profile name), so several configured instances of one kind can
# coexist. Keys are passed to the provider as they are (api_key, endpoint, model,
# timeout_secs, max_retries, ...), and every value is a quoted string, numbers too.
# An environment variable TAGENT_<NAME>_<KEY> (e.g. TAGENT_DEEPL_API_KEY) overrides a key.
#
# Ready-made profiles for every available provider follow, each under a ## title. To
# use one, remove the leading hash and space from each line below its title and fill in
# the empty values; the numbers shown are the defaults. Lines starting with ## are
# explanations, and a #key = "" line is an optional key: uncomment it too if you need it.
#
{profile_examples}"#,
        language_codes = language_code_lines(),
        default_target = Config::default().target_language,
        translate_providers = tagent::providers::TRANSLATION_PROVIDERS.join(", "),
        dictionary_providers = tagent::providers::DICTIONARY_PROVIDERS.join(", "),
        speech_providers = tagent::providers::SPEECH_PROVIDERS.join(", "),
        profile_examples = profile_examples(),
    )
}

/// The template's list of the language codes `tagent` knows, with their names
/// (`# en (English), ru (Russian), ...`), wrapped like the rest of the template.
fn language_code_lines() -> String {
    let mut lines = Vec::new();
    let mut line = String::from("#");
    let entries: Vec<String> = tagent::languages::LANGUAGES
        .iter()
        .map(|language| format!("{} ({})", language.code, language.name))
        .collect();
    for (i, entry) in entries.iter().enumerate() {
        let entry = if i + 1 < entries.len() {
            format!("{entry},")
        } else {
            entry.clone()
        };
        if line.len() + 1 + entry.len() > 80 {
            lines.push(line);
            line = String::from("#");
        }
        line.push(' ');
        line.push_str(&entry);
    }
    lines.push(line);
    lines.join("\n")
}

/// The built-in provider kinds, each with its descriptors on every axis it serves, in
/// registry order (translation, then dictionary, then speech).
fn provider_kinds() -> Vec<(&'static str, Vec<&'static ProviderDescriptor>)> {
    let mut kinds: Vec<(&'static str, Vec<&'static ProviderDescriptor>)> = Vec::new();
    for descriptor in providers::translation_providers()
        .iter()
        .chain(providers::dictionary_providers())
        .chain(providers::speech_providers())
    {
        match kinds.iter_mut().find(|(name, _)| *name == descriptor.name) {
            Some((_, descriptors)) => descriptors.push(descriptor),
            None => kinds.push((descriptor.name, vec![descriptor])),
        }
    }
    kinds
}

/// The commented-out `[provider_options.<kind>]` blocks at the end of a new config file:
/// one per built-in provider kind, generated from `tagent`'s registry, so a new provider
/// (or a compiled-out one) is reflected without touching the template. Removing the
/// leading `"# "` from a block's lines gives a working profile: required keys are empty
/// strings to fill in, the transport options carry the provider's defaults, and any other
/// optional key stays commented out (`#key`). A kind's block may be followed by an example
/// of a profile with a name of its own: the kind's custom one ([`custom_example_lines`]),
/// else, for the first kind with required options, a generic second profile
/// ([`second_instance_lines`]). Each block starts with its `##` title lines; every line
/// below them starts with `"# "`, and blocks are separated by a lone `"#"`.
fn profile_examples() -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut second_instance_shown = false;
    for (kind, descriptors) in provider_kinds() {
        lines.extend(example_block_lines(kind, &descriptors));
        lines.push(String::new());
        let extra = match custom_example_lines(kind) {
            Some(custom) => custom,
            None if second_instance_shown => Vec::new(),
            None => {
                let second = second_instance_lines(kind, &descriptors);
                second_instance_shown = !second.is_empty();
                second
            }
        };
        if !extra.is_empty() {
            lines.extend(extra);
            lines.push(String::new());
        }
    }
    lines.pop();
    commented_out(&lines)
}

/// A generic second profile of kind `kind` (`<kind>-work`, with the required keys empty),
/// before it is commented out; empty for a kind without required options.
fn second_instance_lines(kind: &str, descriptors: &[&ProviderDescriptor]) -> Vec<String> {
    let mut required: Vec<&str> = Vec::new();
    for option in descriptors.iter().flat_map(|d| d.options) {
        if option.required && !required.contains(&option.key) {
            required.push(option.key);
        }
    }
    if required.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![
        format!("## A second {kind} profile (e.g. another account), selected by its own name"),
        format!("[provider_options.{kind}-work]"),
        format!("type = \"{kind}\""),
    ];
    lines.extend(required.iter().map(|key| format!("{key} = \"\"")));
    lines
}

/// A ready-to-use profile of kind `kind` under a name of its own, for a kind whose
/// typical use is a particular server rather than one service (`openai`: a local
/// Ollama); `None` for the other kinds.
fn custom_example_lines(kind: &str) -> Option<Vec<String>> {
    match kind {
        "openai" => Some(
            [
                "## A profile of your own for a particular server, here a local Ollama, selected",
                "## by its own name; one profile can serve translations and dictionary lookups",
                "[provider_options.ollama]",
                "type = \"openai\"",
                "endpoint = \"http://localhost:11434/v1\"",
                "model = \"qwen3:8b\"",
            ]
            .map(String::from)
            .to_vec(),
        ),
        _ => None,
    }
}

/// The example block of provider kind `kind` alone, exactly as [`profile_examples`] writes
/// it (without the separating `"#"` lines); `None` for a kind that isn't built in.
fn profile_example(kind: &str) -> Option<String> {
    provider_kinds()
        .into_iter()
        .find(|(name, _)| *name == kind)
        .map(|(kind, descriptors)| commented_out(&example_block_lines(kind, &descriptors)))
}

/// The lines of one kind's example block, before they are commented out: the `## <kind>:`
/// header line, the table header and one entry per option (with its explanation).
fn example_block_lines(kind: &str, descriptors: &[&ProviderDescriptor]) -> Vec<String> {
    // One name per provider, even when it serves several axes under the same name.
    let mut names: Vec<&str> = Vec::new();
    for descriptor in descriptors {
        if !names.contains(&descriptor.display_name) {
            names.push(descriptor.display_name);
        }
    }
    let transport = descriptors[0].transport;
    let mut options: Vec<&OptionSpec> = Vec::new();
    for option in descriptors.iter().flat_map(|d| d.options) {
        if !options.iter().any(|o| o.key == option.key) {
            options.push(option);
        }
    }
    let mut lines = vec![
        format!("## {kind}: {}", names.join(", ")),
        format!("[provider_options.{kind}]"),
    ];
    for option in options {
        let mut note = option.description.to_string();
        if option.required {
            note.push_str(". Required");
        }
        lines.push(format!("## {note}"));
        if option.secret {
            lines.push(format!(
                "## (or set {} instead of storing it in this file)",
                providers::env_var_name(kind, option.key)
            ));
        }
        let default = match option.key {
            "timeout_secs" => Some(transport.timeout.as_secs().to_string()),
            "max_retries" => Some(transport.max_retries.to_string()),
            _ => None,
        };
        match (default, option.default) {
            (Some(value), _) => lines.push(format!("{} = {}", option.key, Value::from(value))),
            // A long default (a prompt) as a commented-out multi-line string, one `#` per
            // line, so it can be copied and edited; a blank line in it can't end the block.
            (None, Some(value)) if option.multiline || value.contains('\n') => {
                lines.push(format!("#{} = \"\"\"", option.key));
                lines.extend(value.lines().map(|line| format!("#{line}")));
                lines.push("#\"\"\"".to_string());
            }
            (None, Some(value)) => {
                lines.push(format!("#{} = {}", option.key, Value::from(value)));
            }
            (None, None) if option.required => lines.push(format!("{} = \"\"", option.key)),
            (None, None) => lines.push(format!("#{} = \"\"", option.key)),
        }
    }
    lines
}

/// `lines` as TOML comments: `"# <line>"`, an empty line as a lone `"#"`. A block's
/// title (its `##` lines above the table header) stays as it is, already a comment, so it
/// stands out from the commented-out lines and isn't part of what a user uncomments.
fn commented_out(lines: &[String]) -> String {
    let mut in_title = true;
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                in_title = true;
                return "#\n".to_string();
            }
            if line.starts_with('[') {
                in_title = false;
            }
            if in_title && line.starts_with("## ") {
                format!("{line}\n")
            } else {
                format!("# {line}\n")
            }
        })
        .collect()
}

/// The banner's "Active Hotkeys" block. On Wayland the desktop owns the hotkeys, so it
/// shows what the desktop bound (and where to change it), or why the hotkeys are off;
/// elsewhere, the configured hotkeys.
pub(crate) fn hotkey_banner_lines(
    config: &Config,
    hotkeys: &crate::platform::HotkeyBanner,
) -> Vec<String> {
    use crate::platform::HotkeyBanner;
    let speech_on = config.enable_speech_hotkey && config.enable_text_to_speech;
    let configured = |header: &str| {
        let mut lines = vec![header.to_string()];
        lines.push(format!("  Translation: {}", config.translate_hotkey));
        if speech_on {
            lines.push(format!("  Speech: {}", config.speech_hotkey));
        }
        lines
    };
    match hotkeys {
        HotkeyBanner::Configured => configured("Active Hotkeys:"),
        HotkeyBanner::Waiting => configured("Active Hotkeys (waiting for the desktop):"),
        HotkeyBanner::Desktop(desktop) => {
            if let Some(problem) = &desktop.problem {
                let reason = problem
                    .strip_prefix("Global hotkeys are off: ")
                    .unwrap_or(problem);
                let mut chars = reason.chars();
                let reason: String = chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default();
                return vec!["Active Hotkeys: off".to_string(), format!("  {reason}")];
            }
            let mut lines = vec!["Active Hotkeys:".to_string()];
            lines.push(format!(
                "  Translation: {}",
                desktop.translate.as_deref().unwrap_or("not bound")
            ));
            if speech_on {
                lines.push(format!(
                    "  Speech: {}",
                    desktop.speech.as_deref().unwrap_or("not bound")
                ));
            }
            lines.push("  Set by the desktop.".to_string());
            lines.push("  Change: GNOME Settings > Apps > Tagent CLI".to_string());
            lines
        }
    }
}

/// The command-line options `--help` lists, as (flags, description). The user book's
/// "Command-line options" page must name every flag (a test checks it).
pub(crate) const HELP_OPTIONS: &[(&str, &str)] = &[
    ("-h, --help", "Show this help message"),
    ("-c, --config", "Show current configuration"),
    ("-v, --version", "Show version information"),
    (
        "-s, --speech",
        "Speak the following text using text-to-speech",
    ),
    (
        "-l, --lang",
        "Set languages: -l <target> or -l <source> <target>",
    ),
    (
        "--print-default-config",
        "Print a new config file with every setting and its default",
    ),
    (
        "--update-config",
        "Add the settings your config file lacks (backup: .bak)",
    ),
    (
        "--install-desktop",
        "Linux: add the menu entry and icon the Wayland hotkeys need",
    ),
    (
        "--uninstall-desktop",
        "Linux: remove what --install-desktop added",
    ),
];

/// A complete, commented `tagent-cli.toml` holding `config`'s values: the template, with
/// every value replaced and the provider profiles appended.
pub(crate) fn render_config(config: &Config) -> String {
    let mut doc: DocumentMut = config_template()
        .parse()
        .expect("the config template is valid TOML");
    let values = toml_edit::ser::to_document(&ConfigFile::from(config))
        .expect("the configuration serializes to TOML");
    for (section, item) in values.iter() {
        let Some(source) = item.as_table_like() else {
            continue;
        };
        if section == "provider_options" {
            insert_profiles(&mut doc, source);
            continue;
        }
        for (key, value) in source.iter() {
            if let Some(value) = value.as_value() {
                set_value(&mut doc, section, key, value.clone())
                    .expect("the config template's sections are tables");
            }
        }
    }
    doc.to_string()
}

/// Appends one `[provider_options.<name>]` table per profile to `doc`, under an implicit
/// `provider_options` table (so no empty `[provider_options]` header is written). The
/// template's closing comment, which explains profiles, is moved above the first one.
fn insert_profiles(doc: &mut DocumentMut, source: &dyn TableLike) {
    let mut profiles = Table::new();
    profiles.set_implicit(true);
    for (name, options) in source.iter() {
        let mut table = Table::new();
        for (key, value) in options.as_table_like().into_iter().flat_map(|t| t.iter()) {
            if let Some(value) = value.as_value() {
                let mut value = value.clone();
                value.decor_mut().clear();
                table.insert(key, Item::Value(value));
            }
        }
        profiles.insert(name, Item::Table(table));
    }
    if let Some((_, first)) = profiles.iter_mut().next() {
        let comment = doc.trailing().as_str().unwrap_or_default().to_string();
        if let Some(first) = first.as_table_mut() {
            first.decor_mut().set_prefix(format!("{comment}\n"));
        }
        doc.set_trailing("");
    }
    doc.insert("provider_options", Item::Table(profiles));
}

/// Sets `[section] key` in `doc` to `value`, creating the section if needed. An existing
/// key keeps its place and its comments: the ones above it belong to the key, and the
/// inline one after it is copied from the old value.
fn set_value(
    doc: &mut DocumentMut,
    section: &str,
    key: &str,
    mut value: Value,
) -> Result<(), String> {
    let table = doc
        .as_table_mut()
        .entry(section)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_like_mut()
        .ok_or_else(|| format!("`{section}` is not a table"))?;
    match table.get_mut(key) {
        Some(item) => {
            if let Some(old) = item.as_value() {
                *value.decor_mut() = old.decor().clone();
            }
            *item = Item::Value(value);
        }
        None => {
            value.decor_mut().clear();
            table.insert(key, Item::Value(value));
        }
    }
    Ok(())
}

/// A complete `tagent-cli.toml` with the default settings, every explanation and the
/// example provider profiles: what a new file starts as (`--print-default-config`).
pub fn default_config_text() -> String {
    render_config(&Config::default())
}

/// `content` (an existing `tagent-cli.toml`) with the session settings set to `config`'s:
/// the languages (`/l`) and the three providers (`/p`), the only values the app itself
/// changes. Everything else — comments, order, unknown keys, provider profiles — stays as
/// it is. Each provider key (`translate_provider`, `dictionary_provider`,
/// `speech_provider`) is written only when the file has the key or when the value differs
/// from the default a missing key stands for, so saving an untouched session adds nothing
/// to a file without those keys.
fn with_session_settings(content: &str, config: &Config) -> Result<String, String> {
    let mut doc: DocumentMut = content.parse().map_err(|e| format!("{e}"))?;
    set_value(
        &mut doc,
        "translation",
        "source_language",
        Value::from(config.source_language.as_str()),
    )?;
    set_value(
        &mut doc,
        "translation",
        "target_language",
        Value::from(config.target_language.as_str()),
    )?;
    let default = Config::default();
    for axis in ProviderAxis::ALL {
        let (section, key) = match axis {
            ProviderAxis::Translation => ("provider", "translate_provider"),
            ProviderAxis::Dictionary => ("dictionary", "dictionary_provider"),
            ProviderAxis::Speech => ("speech", "speech_provider"),
        };
        let has_key = doc
            .get(section)
            .and_then(Item::as_table_like)
            .is_some_and(|table| table.contains_key(key));
        let value = config.provider_name(axis);
        if has_key || value != default.provider_name(axis) {
            set_value(&mut doc, section, key, Value::from(value))?;
        }
    }
    Ok(doc.to_string())
}

impl Default for Config {
    fn default() -> Self {
        // Try to get data directory path for history file, fallback to current directory
        // On Linux: ~/.local/share/tagent-cli/translation_history.txt
        // On Windows: %APPDATA%/tagent-cli/translation_history.txt
        let default_history = if let Some(data_dir) = dirs::data_dir() {
            let history_path = data_dir.join("tagent-cli").join("translation_history.txt");
            history_path.to_string_lossy().to_string()
        } else {
            "translation_history.txt".to_string()
        };

        Self {
            source_language: "auto".to_string(),
            target_language: default_target_language(),
            show_terminal_on_translate: true,
            auto_hide_terminal_seconds: 3,
            show_dictionary: true,
            spell_check: true,
            dictionary_provider: "google".to_string(), // Default dictionary provider
            copy_to_clipboard: false,
            save_translation_history: false,
            history_file: default_history,
            use_colors: "auto".to_string(),
            target_prompt_color: "BrightYellow".to_string(), // Default bright yellow for target
            dictionary_prompt_color: "BrightYellow".to_string(), // Default bright yellow for dictionary
            source_prompt_color: "None".to_string(),             // Default no color for source
            part_of_speech_color: DEFAULT_PART_OF_SPEECH_COLOR.to_string(),
            synonym_color: DEFAULT_SYNONYM_COLOR.to_string(),
            notice_color: DEFAULT_NOTICE_COLOR.to_string(),
            error_color: DEFAULT_ERROR_COLOR.to_string(),
            translate_hotkey: "Alt+A".to_string(), // Default translation hotkey
            enable_text_to_speech: true,           // TTS enabled by default
            speech_hotkey: "Alt+S".to_string(),    // Default speech hotkey
            enable_speech_hotkey: true,            // Enable speech hotkey by default
            translate_provider: "google".to_string(), // Default translation provider
            speech_provider: "google".to_string(), // Default speech provider
            provider_options: ProviderProfiles::new(),
        }
    }
}

/// Default `[translation]` `target_language`: the first of the system's preferred
/// languages that `tagent` lists, else English.
fn default_target_language() -> String {
    tagent::languages::language_for_locales(sys_locale::get_locales())
        .unwrap_or("en")
        .to_string()
}

/// Default `[colors]` `part_of_speech_color`.
const DEFAULT_PART_OF_SPEECH_COLOR: &str = "Cyan";
/// Default `[colors]` `synonym_color`.
const DEFAULT_SYNONYM_COLOR: &str = "Green";
/// Default `[colors]` `notice_color`.
const DEFAULT_NOTICE_COLOR: &str = "Magenta";
/// Default `[colors]` `error_color`.
const DEFAULT_ERROR_COLOR: &str = "Red";

/// Target language code substituted wherever `"auto"` would otherwise become the
/// target: auto-detection only makes sense for the source language.
pub const AUTO_TARGET_FALLBACK: &str = "en";

/// A source/target language pair of codes resolved from user input (`/l`, `-l`, or the
/// config file), with `"auto"` never left as the target. Callers turn names into codes
/// first ([`language_code`]).
///
/// `notices` holds the messages to show the user about what was adjusted or is
/// worth knowing (an `"auto"` target replaced with [`AUTO_TARGET_FALLBACK`], or a
/// same-language pair). A same-language pair is allowed on purpose: it's the
/// natural way to express a future monolingual (explanatory) dictionary lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePair {
    /// Source language code (may be `"auto"`).
    pub source: String,
    /// Target language code (never `"auto"`).
    pub target: String,
    /// Messages to show the user, in order.
    pub notices: Vec<String>,
}

impl LanguagePair {
    /// Resolves a requested `source -> target` pair.
    pub fn new(source: &str, target: &str) -> Self {
        let mut notices = Vec::new();
        let target = if is_auto(target) {
            notices.push(format!(
                "Target can't be Auto; using {} instead",
                tagent::languages::code_to_name(AUTO_TARGET_FALLBACK)
            ));
            AUTO_TARGET_FALLBACK.to_string()
        } else {
            target.to_string()
        };
        Self::finish(source.to_string(), target, notices)
    }

    /// Resolves the pair produced by swapping `source` and `target`. An `"auto"`
    /// source has no concrete language to become the new target, so
    /// [`AUTO_TARGET_FALLBACK`] takes its place.
    pub fn swapped(source: &str, target: &str) -> Self {
        let mut notices = Vec::new();
        let new_target = if is_auto(source) {
            notices.push(format!(
                "Source was Auto; using {} as the new target",
                tagent::languages::code_to_name(AUTO_TARGET_FALLBACK)
            ));
            AUTO_TARGET_FALLBACK.to_string()
        } else {
            source.to_string()
        };
        Self::finish(target.to_string(), new_target, notices)
    }

    fn finish(source: String, target: String, mut notices: Vec<String>) -> Self {
        if !is_auto(&source) && source.eq_ignore_ascii_case(&target) {
            notices.push("Note: source and target are the same language".to_string());
        }
        Self {
            source,
            target,
            notices,
        }
    }
}

/// `Russian (ru) -> English (en)` for a pair of language codes (`/l`, `-l`, the banner).
/// `auto` and a code `tagent`'s table doesn't list are shown once: `Auto -> uk`.
pub fn language_pair_description(source: &str, target: &str) -> String {
    let describe = |code: &str| {
        let name = tagent::languages::code_to_name(code);
        if name == code || is_auto(code) {
            name.to_string()
        } else {
            format!("{name} ({code})")
        }
    };
    format!("{} -> {}", describe(source), describe(target))
}

/// Whether a language code means auto-detection.
fn is_auto(language: &str) -> bool {
    language.trim().eq_ignore_ascii_case("auto")
}

/// Formats a provider factory error for display. When the configured name is unknown, it
/// appends the supported values and the setting to change (`setting` is the config key,
/// e.g. `"speech_provider"`), because `tagent`'s own message only echoes the bad name. For
/// invalid options it points at the profile's `[provider_options.<profile>]` table and its
/// environment variables. Any other error is shown as is.
pub fn provider_error_message(
    error: &tagent::error::Error,
    setting: &str,
    profile: &str,
    supported: &[&str],
) -> String {
    match error {
        tagent::error::Error::UnknownProvider(_) => format!(
            "{error} (supported values for {setting}: {})",
            supported.join(", ")
        ),
        tagent::error::Error::InvalidOptions(_) => format!(
            "{error} (check [provider_options.{}] in tagent-cli.toml, or the {}<KEY> environment variables)",
            profile.to_lowercase(),
            providers::env_var_name(profile, "")
        ),
        _ => error.to_string(),
    }
}

/// The providers the banner reports: each one's display name (its `name()`, e.g.
/// `"Google Translate"`, or `"DeepL (work)"` for a profile), or why it is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveProviders {
    /// The translation provider in use.
    pub translation: Result<String, String>,
    /// The dictionary provider in use.
    pub dictionary: Result<String, String>,
    /// The speech provider the next playback will use.
    pub speech: Result<String, String>,
}

/// One line of `/p`'s list, see [`Config::provider_list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderListEntry {
    /// The axis this entry is listed under.
    pub axis: ProviderAxis,
    /// The name `/p` and the axis's `*_provider` setting take: a kind or a profile name.
    pub name: String,
    /// What the provider calls itself, e.g. `"DeepL (deepl-work)"` for a profile.
    pub display_name: String,
    /// Whether this is the provider in effect on its axis.
    pub active: bool,
    /// Required options that are unset (or empty), e.g. `api_key`.
    pub missing: Vec<String>,
}

/// The heading `/p` shows for `axis`: `"Translation"`, `"Dictionary"` or `"Speech"`.
pub fn axis_title(axis: ProviderAxis) -> &'static str {
    match axis {
        ProviderAxis::Translation => "Translation",
        ProviderAxis::Dictionary => "Dictionary",
        ProviderAxis::Speech => "Speech",
    }
}

/// `/p`'s output lines for `entries` (as [`Config::provider_list`] returns them): one
/// section per axis, with a note on an axis `config` turns off, and the entries numbered
/// continuously from 1 in list order, which is what `/p <number>` refers to.
pub fn provider_list_lines(entries: &[ProviderListEntry], config: &Config) -> Vec<String> {
    let name_width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let number_width = entries.len().to_string().len();
    let mut lines = vec!["Providers:".to_string()];
    for axis in ProviderAxis::ALL {
        let mut heading = format!(" {}", axis_title(axis));
        if let Some(setting) = config.axis_disabled_by(axis) {
            heading.push_str(&format!(" (off: {setting} = false)"));
        }
        lines.push(heading);
        for (index, entry) in entries.iter().enumerate() {
            if entry.axis != axis {
                continue;
            }
            let marker = if entry.active { '*' } else { ' ' };
            let mut line = format!(
                " {marker} {:>number_width$}  {:name_width$}  {}",
                index + 1,
                entry.name,
                entry.display_name
            );
            if !entry.missing.is_empty() {
                line.push_str(&format!(" (missing: {})", entry.missing.join(", ")));
            }
            lines.push(line);
        }
    }
    lines.push(
        "Switch with /p <number>, /p <name> (translation) or /p t|d|s <name>; \
         /save keeps the choice."
            .to_string(),
    );
    lines
}

/// The banner's `Providers:` block. The dictionary line is shown only with
/// `show_dictionary`, the speech line only with `enable_text_to_speech`, like the
/// speech hotkey line.
fn provider_banner_lines(config: &Config, providers: &ActiveProviders) -> Vec<String> {
    let line = |label: &str, provider: &Result<String, String>| match provider {
        Ok(name) => format!("  {label}: {name}"),
        Err(reason) => format!("  {label}: unavailable ({reason})"),
    };
    let mut lines = vec![
        "Providers:".to_string(),
        line("Translation", &providers.translation),
    ];
    if config.show_dictionary {
        lines.push(line("Dictionary", &providers.dictionary));
    }
    if config.enable_text_to_speech {
        lines.push(line("Speech", &providers.speech));
    }
    lines
}

/// Thread-safe configuration manager with live-reload support.
///
/// `ConfigManager` loads `tagent-cli.toml` on construction and can reload it at
/// runtime without restarting the application. Use [`ConfigManager::new`] with
/// the path returned by [`ConfigManager::get_default_config_path`].
///
/// # Example
///
/// ```ignore
/// let path = ConfigManager::get_default_config_path().unwrap();
/// let manager = Arc::new(ConfigManager::new(path.to_str().unwrap()).unwrap());
/// let config = manager.get_config();
/// println!("Target language: {}", config.target_language);
/// ```
pub struct ConfigManager {
    config_path: String,
    config: Arc<Mutex<Config>>,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
}

impl ConfigManager {
    /// Returns the platform-default path for `tagent-cli.toml`, creating parent directories as needed.
    ///
    /// - **Windows**: `%APPDATA%\tagent-cli\tagent-cli.toml`
    /// - **Linux**: `~/.config/tagent-cli/tagent-cli.toml`
    /// - **macOS**: `~/Library/Application Support/tagent-cli/tagent-cli.toml`
    pub fn get_default_config_path() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("tagent-cli");

        // Create directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("tagent-cli.toml"))
    }

    /// Returns the platform-default path for the interactive-mode line-editing history file,
    /// creating parent directories as needed.
    ///
    /// This is separate from [`Config::history_file`], which logs translation *results* for
    /// the user to read; this file stores rustyline's input-line history instead.
    ///
    /// - **Windows**: `%APPDATA%\tagent-cli\interactive_history.txt`
    /// - **Linux**: `~/.config/tagent-cli/interactive_history.txt`
    /// - **macOS**: `~/Library/Application Support/tagent-cli/interactive_history.txt`
    pub fn get_default_interactive_history_path() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("tagent-cli");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("interactive_history.txt"))
    }

    /// Create a new `ConfigManager` for the given config file path.
    ///
    /// If the file does not exist, a default configuration file is created at `config_path`.
    pub fn new(config_path: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let manager = Self {
            config_path: config_path.to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };

        // Load or create config file
        manager.load_or_create_config()?;

        Ok(manager)
    }

    /// Load configuration from file or create default if not exists
    fn load_or_create_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if Path::new(&self.config_path).exists() {
            self.load_config()?;
        } else {
            self.create_default_config()?;
        }
        Ok(())
    }

    /// Create default configuration file
    fn create_default_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        write_config_file(&self.config_path, &render_config(&Config::default()))?;
        apply_use_colors(&Config::default());
        println!("Created default configuration file: {}", self.config_path);

        // Update last modified time
        self.update_last_modified_time()?;

        Ok(())
    }

    /// Load configuration from the TOML file. On an error the current configuration stays
    /// in effect; the message names the file, and the line and column of the problem.
    fn load_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let content = fs::read_to_string(&self.config_path)?;
        let (new_config, language_warnings) = parse_config_with_warnings(&content)
            .map_err(|e| invalid_file_message(&self.config_path, &e))?;

        apply_use_colors(&new_config);
        if let Ok(mut config) = self.config.lock() {
            *config = new_config;
        }
        for unknown in unknown_keys_in(&content) {
            eprintln!("{}", unknown.warning(&self.file_name()));
        }
        for warning in language_warnings {
            eprintln!("Warning: {}: {warning}", self.file_name());
        }

        self.update_last_modified_time()?;

        Ok(())
    }

    /// Adds the settings the config file lacks (`/config update`), see
    /// [`update_config_file`]. The configuration in effect is then read from the updated
    /// file, so an edit not yet picked up by the hot reload isn't skipped, without
    /// repeating the unknown-key warnings the report already lists.
    pub fn update_config_file(&self) -> Result<ConfigUpdate, String> {
        let update = update_config_file(Path::new(&self.config_path))?;
        let content = fs::read_to_string(&self.config_path).map_err(|e| e.to_string())?;
        let config =
            parse_config(&content).map_err(|e| invalid_file_message(&self.config_path, &e))?;
        if let Ok(mut current) = self.config.lock() {
            *current = config;
        }
        self.update_last_modified_time()
            .map_err(|e| e.to_string())?;
        Ok(update)
    }

    /// The startup notice about settings the config file lacks, if it lacks any (see
    /// [`update_config_file`]).
    pub fn new_settings_notice(&self) -> Option<String> {
        let content = fs::read_to_string(&self.config_path).ok()?;
        new_settings_notice(upgrade::new_settings_count(&content))
    }

    /// The config file's name, for messages (`tagent-cli.toml`).
    fn file_name(&self) -> String {
        Path::new(&self.config_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.config_path.clone())
    }

    /// Save the current languages and providers to the config file (see
    /// [`with_session_settings`]), editing it in place so comments,
    /// key order, unknown keys and provider profiles are kept. A missing file is written
    /// in full from the current configuration.
    pub fn save_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let config = self.get_config();
        let content = match fs::read_to_string(&self.config_path) {
            Ok(existing) => with_session_settings(&existing, &config).map_err(|e| {
                format!(
                    "can't update configuration file {}:\n{}",
                    self.config_path,
                    e.trim_end()
                )
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => render_config(&config),
            Err(e) => return Err(e.into()),
        };
        write_config_file(&self.config_path, &content)?;
        self.update_last_modified_time()?;
        Ok(())
    }

    /// Return a snapshot of the current in-memory configuration.
    pub fn get_config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    /// Replace the entire in-memory config (without saving to file)
    #[allow(dead_code)]
    pub fn set_config(&self, new_config: Config) {
        if let Ok(mut config) = self.config.lock() {
            *config = new_config;
        }
    }

    /// Set source and target languages in memory (without saving to file).
    ///
    /// Both are codes: callers resolve user input through [`language_code`] and
    /// [`LanguagePair`] first, so `target` is never `"auto"`.
    pub fn set_languages(&self, source: &str, target: &str) {
        if let Ok(mut config) = self.config.lock() {
            config.source_language = source.to_string();
            config.target_language = target.to_string();
        }
    }

    /// Set the provider of `axis` (`translate_provider`, `dictionary_provider` or
    /// `speech_provider`) in memory, without saving to file. Doesn't validate `name`:
    /// callers validate it first (`/p` builds the provider before calling this).
    pub fn set_provider(&self, axis: ProviderAxis, name: &str) {
        if let Ok(mut config) = self.config.lock() {
            let field = match axis {
                ProviderAxis::Translation => &mut config.translate_provider,
                ProviderAxis::Dictionary => &mut config.dictionary_provider,
                ProviderAxis::Speech => &mut config.speech_provider,
            };
            *field = name.to_string();
        }
    }

    /// Print the interactive-mode banner: version, the current language pair, the
    /// providers in use, active hotkeys and a command summary.
    ///
    /// Shown at startup and again after `/clear`.
    pub fn display_banner(
        config: &Config,
        providers: &ActiveProviders,
        hotkeys: &crate::platform::HotkeyBanner,
    ) {
        println!("Text Translator v{}", env!("CARGO_PKG_VERSION"));
        println!();

        println!(
            "Languages: {}",
            language_pair_description(&config.source_language, &config.target_language)
        );
        println!();

        for line in provider_banner_lines(config, providers) {
            println!("{line}");
        }
        println!();

        for line in hotkey_banner_lines(config, hotkeys) {
            println!("{line}");
        }
        println!();

        println!(
            r#"Commands:
  /h (help), /c (config), /s (speech), /ss (speak translation)
  /l (lang), /p (provider), /save, /clear, /e (exit)"#
        );
        println!();
    }

    /// Display help information (unified for CLI and Interactive modes)
    pub fn display_help() {
        println!();
        println!("=== Text Translator v{} ===", env!("CARGO_PKG_VERSION"));
        println!();
        println!("MODES:");
        println!();
        println!("1. Unified Mode (default): Run without arguments");
        println!("   - Interactive prompt in terminal + GUI hotkeys");
        println!("   - Both methods work simultaneously");
        println!();
        println!("2. CLI Mode: Run 'tagent-cli <text>' for one-time translation");
        println!();

        println!("USAGE:");
        println!("  tagent-cli [OPTIONS] [text]");
        println!();

        println!("ARGUMENTS:");
        println!("  <text>    Text to translate (use quotes for phrases with spaces)");
        println!();

        println!("OPTIONS:");
        for (flags, description) in HELP_OPTIONS {
            if flags.len() < 15 {
                println!("  {flags:<15}{description}");
            } else {
                println!("  {flags}");
                println!("  {:<15}{description}", "");
            }
        }
        println!();

        println!("SUPPORTED LANGUAGES (name or code, e.g. -l German or -l de):");
        println!("  Auto               English (en)       Russian (ru)");
        println!("  Spanish (es)       French (fr)        German (de)");
        println!("  Chinese (zh)       Japanese (ja)      Korean (ko)");
        println!("  Italian (it)       Portuguese (pt)    Dutch (nl)");
        println!("  Polish (pl)        Turkish (tr)       Arabic (ar)");
        println!("  Hindi (hi)");
        println!();

        println!("EXAMPLES:");
        println!(
            "  tagent-cli                           Start unified mode (interactive + hotkeys)"
        );
        println!("  tagent-cli hello                     Translate 'hello' (CLI mode)");
        println!("  tagent-cli \"Hello world\"             Translate phrase (CLI mode)");
        println!("  tagent-cli -s \"Hello world\"          Speak text using TTS");
        println!("  tagent-cli -l German hello               Translate 'hello' to German");
        println!(
            "  tagent-cli -l English German hello        Translate 'hello' from English to German"
        );
        println!("  tagent-cli --config                  Show configuration");
        println!();

        println!("UNIFIED MODE - TRANSLATION METHODS:");
        println!();
        println!("1. Interactive Terminal:");
        println!("   - Type any text and press Enter to translate");
        println!("   - Single words show dictionary entries (if enabled)");
        println!("   - Phrases show translations");
        println!("   - Empty line = skip/continue");
        println!("   - Arrow keys/Ctrl+A/Ctrl+E to edit, Ctrl+R to search history");
        println!("   - Input history persists across sessions; Tab-completes slash-commands");
        println!();

        println!("2. GUI Hotkeys (Any Application):");
        println!("   - Select text in any application");
        println!("   - Press configured hotkey (default: Alt+A)");
        println!("   - Result copied to clipboard if copy_to_clipboard = true");
        println!("   - Configure hotkeys in tagent-cli.toml [hotkeys] section");
        println!();

        println!("INTERACTIVE COMMANDS (must start with slash):");
        println!("  /h, /help, /?           - Show this help");
        println!("  /c, /config             - Show current configuration");
        println!("  /v, /version            - Show version information");
        println!(
            "  /s, /speech <text>      - Speak text using text-to-speech (press Esc to cancel)"
        );
        println!(
            "  /s, /speech             - Speak the last translated phrase (typed or via hotkey)"
        );
        println!("  /ss                     - Speak the translation of the last phrase");
        println!("  /l, /lang               - Swap source and target languages");
        println!("  /l, /lang <target>      - Set target language (source=Auto)");
        println!("  /l, /lang <src> <tgt>   - Set source and target languages");
        println!("  /p, /provider           - List the providers of every axis, numbered");
        println!("  /p <number>             - Switch to that entry of the list for this session");
        println!("  /p <name>               - Switch the translation provider for this session");
        println!(
            "  /p t|d|s <name>         - Switch the translation, dictionary or speech provider"
        );
        println!("  /save                   - Save the languages and providers to file");
        println!("  /config update          - Add the settings the config file lacks");
        println!("  /clear, /cls            - Clear screen");
        println!("  /q, /quit, /e, /exit    - Exit program");
        println!();

        println!("CONFIGURATION:");
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("  Config file: {}", config_path.display());
        } else {
            println!("  Config file: tagent-cli.toml (typically in %APPDATA%\\tagent-cli\\)");
        }
        println!();
        println!("  Edit 'tagent-cli.toml' to change translation settings:");
        println!("  - source_language: Source language code (auto, en, ru, etc.)");
        println!("  - target_language: Target language code (ru, en, etc.)");
        println!(
            "  - translate_provider: Translation backend ({})",
            tagent::providers::TRANSLATION_PROVIDERS.join(", ")
        );
        println!("  - show_dictionary: Enable dictionary lookup for single words");
        println!(
            "  - dictionary_provider: Dictionary backend ({})",
            tagent::providers::DICTIONARY_PROVIDERS.join(", ")
        );
        println!("  - copy_to_clipboard: Copy results to clipboard");
        println!("  - translate_hotkey: Custom hotkey (Ctrl+Ctrl, Alt+Q, F9, etc.)");
        println!("  - speech_hotkey: Hotkey for text-to-speech (Alt+S, F10, etc.)");
        println!(
            "  - speech_provider: Text-to-speech backend ({})",
            tagent::providers::SPEECH_PROVIDERS.join(", ")
        );
        println!("  - save_translation_history: Save all translations to file");
        println!();

        println!("FEATURES:");
        println!("- Same translation engine for all modes");
        println!("- Google Translate API with dictionary lookups");
        println!("- Configuration hot-reload (changes take effect immediately)");
        println!("- Configurable hotkeys with various combinations");
        println!("- Text-to-speech support (Google TTS), with replay of the last translation");
        println!("- Translation history logging");
        println!("- Clipboard integration");
        println!();
        println!("Run 'tagent-cli --config' to see current settings.");
        println!("===============================================");
        println!();
    }

    /// Display current configuration (unified for CLI and Interactive modes)
    pub fn display_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Reload config to get latest values
        self.reload_or_warn();
        let config = self.get_config();

        println!();
        println!("=== Current Configuration ===");
        for line in config.display_lines(|var| std::env::var(var).ok()) {
            println!("{line}");
        }
        println!();
        println!("# Config file: {}", self.config_path);
        println!("# Edit this file to change settings (changes take effect immediately)");
        println!("=============================");
        println!();

        Ok(())
    }

    /// Reload the configuration file if it has been modified on disk since the last load.
    ///
    /// Returns `true` when the configuration was actually reloaded, `false` when
    /// the file was unchanged or did not exist.
    pub fn check_and_reload(&self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        if !Path::new(&self.config_path).exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(&self.config_path)?;
        let current_modified = metadata.modified()?;

        let should_reload = {
            let last_modified = self.last_modified.lock().unwrap();
            match *last_modified {
                Some(last) => current_modified > last,
                None => true,
            }
        };

        if should_reload {
            // Recorded before loading, so a broken edit is reported once rather than on
            // every call until the file changes again.
            if let Ok(mut last_modified) = self.last_modified.lock() {
                *last_modified = Some(current_modified);
            }
            self.load_config()
                .map_err(|e| format!("{e}\n(keeping the previous settings)"))?;
            return Ok(true);
        }

        Ok(false)
    }

    /// [`Self::check_and_reload`], printing a warning instead of returning an error: for
    /// callers that just go on with the configuration currently in effect.
    pub fn reload_or_warn(&self) {
        if let Err(e) = self.check_and_reload() {
            eprintln!("Warning: {e}");
        }
    }

    /// Update last modified time
    fn update_last_modified_time(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if Path::new(&self.config_path).exists() {
            let metadata = fs::metadata(&self.config_path)?;
            let modified = metadata.modified()?;

            if let Ok(mut last_modified) = self.last_modified.lock() {
                *last_modified = Some(modified);
            }
        }
        Ok(())
    }

    /// The source and target language codes in effect.
    pub fn get_language_codes(&self) -> (String, String) {
        let config = self.get_config();
        (config.source_language, config.target_language)
    }

    /// Parse color name to colored::Color enum
    /// Returns None for "None" or empty string (no color)
    pub fn parse_color(color_name: &str) -> Option<colored::Color> {
        let color_lower = color_name.trim().to_lowercase();

        // Handle "None" or empty string as no color
        if color_lower.is_empty() || color_lower == "none" {
            return None;
        }

        match color_lower.as_str() {
            "black" => Some(colored::Color::Black),
            "red" => Some(colored::Color::Red),
            "green" => Some(colored::Color::Green),
            "yellow" => Some(colored::Color::Yellow),
            "blue" => Some(colored::Color::Blue),
            "magenta" => Some(colored::Color::Magenta),
            "cyan" => Some(colored::Color::Cyan),
            "white" => Some(colored::Color::White),
            "brightblack" | "bright_black" => Some(colored::Color::BrightBlack),
            "brightred" | "bright_red" => Some(colored::Color::BrightRed),
            "brightgreen" | "bright_green" => Some(colored::Color::BrightGreen),
            "brightyellow" | "bright_yellow" => Some(colored::Color::BrightYellow),
            "brightblue" | "bright_blue" => Some(colored::Color::BrightBlue),
            "brightmagenta" | "bright_magenta" => Some(colored::Color::BrightMagenta),
            "brightcyan" | "bright_cyan" => Some(colored::Color::BrightCyan),
            "brightwhite" | "bright_white" => Some(colored::Color::BrightWhite),
            _ => None, // Return None for unknown colors
        }
    }
}

// === Shared utility functions ===

/// Save translation history entry to file.
/// Shared across translator, interactive, and CLI modes.
pub fn save_translation_history(
    original: &str,
    translated: &str,
    source_lang: &str,
    target_lang: &str,
    config: &Config,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !config.save_translation_history {
        return Ok(());
    }

    let timestamp: DateTime<Utc> = Utc::now();
    let formatted_time = timestamp.format("%Y-%m-%d %H:%M:%S UTC");

    let entry = format!(
        "[{}] {} -> {}\nIN:  {}\nOUT: {}\n---\n\n",
        formatted_time, source_lang, target_lang, original, translated
    );

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&config.history_file).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.history_file)?;

    file.write_all(entry.as_bytes())?;
    file.flush()?;

    Ok(())
}

/// Check if text is a single word (no spaces, punctuation at edges allowed).
/// Shared across translator, interactive, and CLI modes.
pub fn is_single_word(text: &str) -> bool {
    let cleaned = text.trim_matches(|c: char| !c.is_alphabetic());
    !cleaned.is_empty()
        && !cleaned.contains(' ')
        && cleaned
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

/// Wrap `label` in the ANSI escape sequence for `color_name`, or return it unchanged
/// if the color name is unrecognized or `"None"`.
pub fn colorize(label: &str, color_name: &str) -> String {
    if let Some(color) = ConfigManager::parse_color(color_name) {
        label.color(color).to_string()
    } else {
        label.to_string()
    }
}

/// Render a dictionary article for the terminal, coloring each span per the
/// `[colors]` settings. The clipboard and the history file get
/// [`tagent::article::to_plain`] of the same lines instead, so no escape codes
/// ever reach them.
pub fn render_article(lines: &[tagent::article::Line], config: &Config) -> String {
    tagent::article::render_with(lines, "  ", |role, text| {
        colorize(text, article_role_color(role, config))
    })
}

/// The configured color name for a dictionary article span of `role`.
fn article_role_color(role: tagent::article::Role, config: &Config) -> &str {
    use tagent::article::Role;
    match role {
        Role::PartOfSpeech => &config.part_of_speech_color,
        Role::Synonym => &config.synonym_color,
        // Header and Plain text use the terminal's own foreground color.
        _ => "None",
    }
}

/// `[colors]` `use_colors`: whether terminal output is colored at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseColors {
    /// Colored only on a terminal that shows colors, see [`color_override`].
    Auto,
    /// Always colored, even into a pipe or a file.
    Always,
    /// Never colored.
    Never,
}

impl UseColors {
    /// Parses a setting value, in any case; `None` for an unknown one.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

/// Whether to color the output, or `None` to leave it to `colored`, which colors only a
/// terminal stdout and honors `NO_COLOR`, `CLICOLOR` and `CLICOLOR_FORCE`.
///
/// `term` is the `TERM` variable; `ansi_supported` is
/// [`crate::platform::ansi_supported`]. `Auto` turns colors off for a
/// `TERM=dumb` terminal and a console that can't show escape sequences, which `colored`
/// doesn't check.
fn color_override(mode: UseColors, term: Option<&str>, ansi_supported: bool) -> Option<bool> {
    match mode {
        UseColors::Always => Some(true),
        UseColors::Never => Some(false),
        UseColors::Auto if term == Some("dumb") || !ansi_supported => Some(false),
        UseColors::Auto => None,
    }
}

/// Applies `config`'s `use_colors` to every colored output of the process, see
/// [`color_override`]. Called on each (re)load of the config file, so a change takes
/// effect without a restart.
fn apply_use_colors(config: &Config) {
    let mode = UseColors::parse(&config.use_colors).unwrap_or(UseColors::Auto);
    let term = std::env::var("TERM").ok();
    match color_override(mode, term.as_deref(), crate::platform::ansi_supported()) {
        Some(enabled) => colored::control::set_override(enabled),
        None => colored::control::unset_override(),
    }
}

/// Print a label with optional color, without a trailing newline.
/// Eliminates the repeated pattern of `if let Some(color) = parse_color(...) { ... } else { ... }`.
pub fn print_colored(label: &str, color_name: &str) {
    print!("{}", colorize(label, color_name));
}

// Hotkey configuration types and parser
/// A parsed hotkey configuration, describing how a key or key combination
/// should be detected by the platform keyboard hook.
#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyType {
    /// A single key press (only `F1`-`F12` are allowed here for safety).
    SingleKey {
        /// Virtual-key code of the key.
        vk_code: u32,
    },
    /// A modifier(s) + key combination, e.g. `Alt+Q` or `Ctrl+Shift+T`.
    ModifierCombo {
        /// Virtual-key codes of the required modifier keys, all of which must be held.
        modifiers: Vec<u32>,
        /// Virtual-key code of the non-modifier key that completes the combo.
        key: u32,
    },
    /// Two presses of the same key within a configurable time window, e.g. `Ctrl+Ctrl`.
    DoublePress {
        /// Virtual-key code of the key.
        vk_code: u32,
        /// Minimum time between presses, in milliseconds, for the second press to count.
        min_interval_ms: u64,
        /// Maximum time between presses, in milliseconds, for the second press to count.
        max_interval_ms: u64,
    },
}

impl HotkeyType {
    /// Whether this hotkey involves Alt: a `ModifierCombo` with Alt among its modifiers,
    /// or a `DoublePress` of Alt. Only Alt puts a Windows app into menu mode, which is
    /// what the slow, careful selection copy ([`CopyMode::Alt`]) exists for.
    pub fn uses_alt(&self) -> bool {
        let is_alt = |vk: u32| keycodes::normalize_vk_code(vk) == keycodes::KEY_ALT;
        match self {
            HotkeyType::SingleKey { .. } => false,
            HotkeyType::ModifierCombo { modifiers, .. } => modifiers.iter().any(|&m| is_alt(m)),
            HotkeyType::DoublePress { vk_code, .. } => is_alt(*vk_code),
        }
    }

    /// Whether this hotkey involves Win (Super; Cmd on macOS), in any position. Such
    /// hotkeys are refused by [`HotkeyParser::validate_hotkey`].
    pub fn uses_win(&self) -> bool {
        let is_win = |vk: u32| matches!(vk, keycodes::KEY_LWIN | keycodes::KEY_RWIN);
        match self {
            HotkeyType::SingleKey { vk_code } => is_win(*vk_code),
            HotkeyType::ModifierCombo { modifiers, key } => {
                modifiers.iter().any(|&m| is_win(m)) || is_win(*key)
            }
            HotkeyType::DoublePress { vk_code, .. } => is_win(*vk_code),
        }
    }
}

/// How the global hotkey that fired copies the selection (Windows; Linux and macOS
/// accept it and ignore it). Each hotkey has its own: the translate and speech hotkeys
/// may differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyMode {
    /// The hotkey involves Alt: wait for Alt to be released and cancel any menu mode
    /// before the simulated Ctrl+C (see the Windows `ClipboardManager`).
    Alt,
    /// Any other hotkey: release the held Shift/Win keys and copy at once.
    Plain,
}

impl CopyMode {
    /// The copy mode for `hotkey`; [`CopyMode::Alt`], the careful one, when there is
    /// none.
    pub fn for_hotkey(hotkey: Option<&HotkeyType>) -> Self {
        match hotkey {
            Some(hotkey) if !hotkey.uses_alt() => CopyMode::Plain,
            _ => CopyMode::Alt,
        }
    }
}

/// Stateless parser that converts hotkey configuration strings (e.g. `"Alt+Q"`)
/// into [`HotkeyType`] values, and validates them against dangerous system shortcuts.
pub struct HotkeyParser;

impl HotkeyParser {
    /// Parse hotkey string into HotkeyType
    pub fn parse(hotkey_str: &str) -> Result<HotkeyType, String> {
        let trimmed = hotkey_str.trim();

        if trimmed.is_empty() {
            return Err("Empty hotkey string".to_string());
        }

        // Check for double-press pattern (e.g., "Ctrl+Ctrl")
        if trimmed.contains('+') {
            let parts: Vec<&str> = trimmed.split('+').map(|s| s.trim()).collect();

            // Check if it's a double-press (same key twice)
            if parts.len() == 2 && parts[0].eq_ignore_ascii_case(parts[1]) {
                // Normalized because the observed key event is always normalized to the
                // generic code before comparison in the keyboard hooks (see keycodes::normalize_vk_code).
                let vk_code = keycodes::normalize_vk_code(Self::key_name_to_vk(parts[0])?);
                return Ok(HotkeyType::DoublePress {
                    vk_code,
                    min_interval_ms: 50,
                    max_interval_ms: 500,
                });
            }

            // Otherwise it's a modifier combination
            // Last part is the key, everything else is modifiers
            if parts.len() < 2 {
                return Err("Invalid modifier combination".to_string());
            }

            // `key` (the trigger) is compared against the raw observed vk_code and stays
            // left/right-specific. `modifiers` are compared against the normalized observed
            // code, so they must be normalized here too, or a side-specific modifier
            // (e.g. "LAlt") would never match.
            let key = Self::key_name_to_vk(parts.last().unwrap())?;
            let modifiers: Result<Vec<u32>, String> = parts[..parts.len() - 1]
                .iter()
                .map(|m| Self::key_name_to_vk(m).map(keycodes::normalize_vk_code))
                .collect();

            return Ok(HotkeyType::ModifierCombo {
                modifiers: modifiers?,
                key,
            });
        }

        // Single key
        let vk_code = Self::key_name_to_vk(trimmed)?;
        Ok(HotkeyType::SingleKey { vk_code })
    }

    /// Convert key name to platform-specific virtual key code
    fn key_name_to_vk(key_name: &str) -> Result<u32, String> {
        keycodes::key_name_to_vk(key_name)
    }

    /// Validate that the hotkey doesn't conflict with critical system shortcuts, with
    /// typing, or with what every app uses the same keys for. The hook takes a hotkey
    /// away from every application, so only keys nobody else needs are accepted:
    /// - a single key: `F1`-`F12`;
    /// - a combination: Ctrl, Alt and/or Shift (not Shift alone), then a letter, a digit
    ///   or `F1`-`F12` (not Tab, Space, Enter, Esc, Backspace, Delete, Insert, arrows,
    ///   Home/End/PageUp/PageDown: system and editing shortcuts live there), and not
    ///   `Ctrl+A/C/V/X/Y/Z` (select all, copy, paste, cut, redo, undo);
    /// - a double press: `F1`-`F12`, Ctrl or Shift (not Alt: its first press reaches the
    ///   app, which opens the menu bar);
    /// - nothing with Win (Super).
    pub fn validate_hotkey(hotkey: &HotkeyType) -> Result<(), String> {
        if hotkey.uses_win() {
            return Err("Win (Super) is not allowed in hotkeys: the system reserves most Win combinations, and releasing Win can open the Start menu. Use Ctrl or Alt instead (e.g., Ctrl+Shift+T, Alt+Q).".to_string());
        }

        let is_function_key = |vk: u32| (keycodes::KEY_F1..=keycodes::KEY_F12).contains(&vk);

        match hotkey {
            HotkeyType::SingleKey { vk_code } if !is_function_key(*vk_code) => {
                return Err("Single keys are only allowed for F1-F12. For other keys, use a modifier combination (e.g., Alt+Q, Ctrl+Shift+T)".to_string());
            }
            HotkeyType::SingleKey { .. } => {}
            HotkeyType::ModifierCombo { modifiers, key } => {
                // Modifiers are normalized by `parse`, so the generic codes are enough.
                if let Some(&not_modifier) = modifiers.iter().find(|&&m| {
                    !matches!(
                        keycodes::normalize_vk_code(m),
                        keycodes::KEY_CONTROL | keycodes::KEY_ALT | keycodes::KEY_SHIFT
                    )
                }) {
                    return Err(format!(
                        "Only Ctrl, Alt and Shift can be held for a hotkey, followed by one key; {} is not a modifier (e.g., Alt+Q, Ctrl+Shift+T)",
                        key_display_name(not_modifier)
                    ));
                }

                let is_letter = (('A' as u32)..=('Z' as u32)).contains(key);
                let is_digit = (('0' as u32)..=('9' as u32)).contains(key);
                if !(is_letter || is_digit || is_function_key(*key)) {
                    return Err("A hotkey must end with a letter, a digit or F1-F12: Tab, Space, Enter, Esc, Backspace, Delete, Insert, the arrows, Home, End, PageUp and PageDown belong to system and editing shortcuts, and a modifier can't be the last key (e.g., Alt+Q, Ctrl+Shift+T)".to_string());
                }

                let has = |modifier: u32| {
                    modifiers
                        .iter()
                        .any(|&m| keycodes::normalize_vk_code(m) == modifier)
                };
                let has_ctrl = has(keycodes::KEY_CONTROL);
                let has_alt = has(keycodes::KEY_ALT);
                let has_shift = has(keycodes::KEY_SHIFT);

                // Shift+Key is how capitals and symbols are typed.
                if has_shift && !has_ctrl && !has_alt {
                    return Err("Shift+Key combinations are not allowed (interferes with text input). Use multi-modifier combinations like Ctrl+Shift+T or Alt+Shift+Q instead.".to_string());
                }

                if has_ctrl && !has_alt && !has_shift && "ACVXYZ".chars().any(|c| *key == c as u32)
                {
                    return Err("Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+Y and Ctrl+Z are select all, copy, paste, cut, redo and undo in every app. Use another key (e.g., Ctrl+Q, Ctrl+Shift+C).".to_string());
                }

                // Warnings for common shortcuts (don't block, just warn in logs)
                if has_alt && *key == keycodes::KEY_F4 {
                    eprintln!("Warning: Alt+F4 may close windows");
                }
                if has_ctrl && has_alt && (is_letter || is_digit) {
                    eprintln!(
                        "Warning: Ctrl+Alt+{} is AltGr+{} on many keyboard layouts, which types a character there",
                        key_display_name(*key),
                        key_display_name(*key)
                    );
                }
            }
            // Doubling an ordinary letter/digit/etc. key (e.g. "Q+Q") is indistinguishable
            // from just typing it twice. Alt is not swallowed for a double press, so its
            // first press reaches the app and opens the menu bar, which eats the copy.
            HotkeyType::DoublePress { vk_code, .. }
                if !is_function_key(*vk_code)
                    && !matches!(*vk_code, keycodes::KEY_CONTROL | keycodes::KEY_SHIFT) =>
            {
                return Err("Double-press is only allowed for F1-F12, Ctrl or Shift (e.g., Ctrl+Ctrl, F8+F8). For other keys, use a modifier combination instead (e.g., Alt+Q).".to_string());
            }
            HotkeyType::DoublePress { .. } => {}
        }

        Ok(())
    }
}

/// A key code as a user would write it, for error messages: a letter or digit as
/// itself, `F1`-`F12` by name, anything else by its code.
fn key_display_name(vk: u32) -> String {
    match vk {
        _ if (('A' as u32)..=('Z' as u32)).contains(&vk)
            || (('0' as u32)..=('9' as u32)).contains(&vk) =>
        {
            char::from_u32(vk).map(String::from).unwrap_or_default()
        }
        _ if (keycodes::KEY_F1..=keycodes::KEY_F12).contains(&vk) => {
            format!("F{}", vk - keycodes::KEY_F1 + 1)
        }
        _ => format!("key code {vk}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_alt_is_true_only_for_hotkeys_with_alt() {
        for hotkey in ["Alt+A", "LAlt+Q", "RAlt+Q", "Ctrl+Alt+T", "Alt+Alt"] {
            assert!(HotkeyParser::parse(hotkey).unwrap().uses_alt(), "{hotkey}");
        }
        for hotkey in ["Ctrl+Q", "Ctrl+Shift+T", "F9", "Ctrl+Ctrl", "Shift+Shift"] {
            assert!(!HotkeyParser::parse(hotkey).unwrap().uses_alt(), "{hotkey}");
        }
    }

    #[test]
    fn copy_mode_follows_the_hotkey_and_defaults_to_alt() {
        let alt = HotkeyParser::parse("Alt+S").unwrap();
        let plain = HotkeyParser::parse("Ctrl+Shift+T").unwrap();
        assert_eq!(CopyMode::for_hotkey(Some(&alt)), CopyMode::Alt);
        assert_eq!(CopyMode::for_hotkey(Some(&plain)), CopyMode::Plain);
        assert_eq!(CopyMode::for_hotkey(None), CopyMode::Alt);
    }

    fn banner_config() -> Config {
        Config {
            translate_hotkey: "Alt+A".to_string(),
            speech_hotkey: "Alt+S".to_string(),
            enable_speech_hotkey: true,
            enable_text_to_speech: true,
            ..Config::default()
        }
    }

    #[test]
    fn hotkey_banner_shows_the_config_off_wayland_and_while_waiting() {
        use crate::platform::HotkeyBanner;
        let config = banner_config();
        assert_eq!(
            hotkey_banner_lines(&config, &HotkeyBanner::Configured),
            ["Active Hotkeys:", "  Translation: Alt+A", "  Speech: Alt+S"]
        );
        assert_eq!(
            hotkey_banner_lines(&config, &HotkeyBanner::Waiting)[0],
            "Active Hotkeys (waiting for the desktop):"
        );
        let no_speech = Config {
            enable_speech_hotkey: false,
            ..banner_config()
        };
        assert_eq!(
            hotkey_banner_lines(&no_speech, &HotkeyBanner::Configured),
            ["Active Hotkeys:", "  Translation: Alt+A"]
        );
    }

    #[test]
    fn hotkey_banner_shows_what_the_desktop_bound() {
        use crate::platform::{DesktopHotkeys, HotkeyBanner};
        let bound = HotkeyBanner::Desktop(DesktopHotkeys {
            translate: Some("Ctrl+Alt+T".into()),
            speech: None,
            problem: None,
        });
        assert_eq!(
            hotkey_banner_lines(&banner_config(), &bound),
            [
                "Active Hotkeys:",
                "  Translation: Ctrl+Alt+T",
                "  Speech: not bound",
                "  Set by the desktop.",
                "  Change: GNOME Settings > Apps > Tagent CLI",
            ]
        );
        let off = HotkeyBanner::Desktop(DesktopHotkeys {
            problem: Some("Global hotkeys are off: the desktop bound no hotkeys.".into()),
            ..DesktopHotkeys::default()
        });
        assert_eq!(
            hotkey_banner_lines(&banner_config(), &off),
            ["Active Hotkeys: off", "  The desktop bound no hotkeys."]
        );
    }

    /// `/l auto` and `-l auto`: an "auto" target is replaced, never kept.
    #[test]
    fn language_pair_replaces_auto_target_with_fallback() {
        let pair = LanguagePair::new("auto", "auto");
        assert_eq!(pair.source, "auto");
        assert_eq!(pair.target, AUTO_TARGET_FALLBACK);
        assert_eq!(AUTO_TARGET_FALLBACK, "en");
        assert_eq!(
            pair.notices,
            vec!["Target can't be Auto; using English instead".to_string()]
        );

        // Any spelling of "auto" counts.
        assert_eq!(
            LanguagePair::new("ru", " AUTO ").target,
            AUTO_TARGET_FALLBACK
        );
    }

    #[test]
    fn language_pair_keeps_a_normal_pair_without_notices() {
        let pair = LanguagePair::new("auto", "ru");
        assert_eq!((pair.source.as_str(), pair.target.as_str()), ("auto", "ru"));
        assert!(pair.notices.is_empty());
    }

    /// Same-language pairs are allowed (future monolingual dictionaries), just
    /// noted -- including one produced by the Auto-target fallback.
    #[test]
    fn language_pair_allows_same_language_with_a_note() {
        let pair = LanguagePair::new("en", "auto");
        assert_eq!((pair.source.as_str(), pair.target.as_str()), ("en", "en"));
        assert_eq!(
            pair.notices,
            vec![
                "Target can't be Auto; using English instead".to_string(),
                "Note: source and target are the same language".to_string(),
            ]
        );

        // Codes the table doesn't list compare case-insensitively too.
        let pair = LanguagePair::new("zh-TW", "zh-tw");
        assert_eq!(
            pair.notices,
            vec!["Note: source and target are the same language".to_string()]
        );
    }

    #[test]
    fn swapped_pair_uses_fallback_for_auto_source() {
        let pair = LanguagePair::swapped("auto", "ru");
        assert_eq!((pair.source.as_str(), pair.target.as_str()), ("ru", "en"));
        assert_eq!(
            pair.notices,
            vec!["Source was Auto; using English as the new target".to_string()]
        );
    }

    #[test]
    fn swapped_pair_swaps_two_concrete_languages() {
        let pair = LanguagePair::swapped("en", "ru");
        assert_eq!((pair.source.as_str(), pair.target.as_str()), ("ru", "en"));
        assert!(pair.notices.is_empty());
    }

    #[test]
    fn language_code_turns_known_names_into_codes_and_keeps_the_rest() {
        assert_eq!(language_code("German"), "de");
        assert_eq!(language_code("DE"), "de");
        assert_eq!(language_code("Auto"), "auto");
        assert_eq!(language_code("zh-TW"), "zh-TW");
        assert_eq!(language_code("uk"), "uk");
    }

    #[test]
    fn language_pair_description_shows_names_and_codes() {
        assert_eq!(
            language_pair_description("auto", "ru"),
            "Auto -> Russian (ru)"
        );
        assert_eq!(language_pair_description("AUTO", "auto"), "Auto -> Auto");
        assert_eq!(language_pair_description("en", "uk"), "English (en) -> uk");
    }

    /// Names and codes in any case load as codes; the file keeps what it has.
    #[test]
    fn languages_load_as_codes_from_names_or_codes() {
        for (source, target) in [
            ("Auto", "Russian"),
            ("auto", "ru"),
            ("AUTO", "RU"),
            ("auto", "rUsSiAn"),
        ] {
            let (config, warnings) = parse_config_with_warnings(&format!(
                "[translation]\nsource_language = \"{source}\"\ntarget_language = \"{target}\"\n"
            ))
            .unwrap();
            assert_eq!(
                (
                    config.source_language.as_str(),
                    config.target_language.as_str()
                ),
                ("auto", "ru"),
                "{source} -> {target}"
            );
            assert!(warnings.is_empty(), "{warnings:?}");
        }

        let (config, _) = parse_config_with_warnings(
            "[translation]\nsource_language = \"English\"\ntarget_language = \"de\"\n",
        )
        .unwrap();
        assert_eq!(
            (
                config.source_language.as_str(),
                config.target_language.as_str()
            ),
            ("en", "de")
        );
    }

    /// A value the table doesn't list is kept as written (not lowercased) and warned about.
    #[test]
    fn an_unknown_language_is_kept_with_a_warning() {
        let (config, warnings) = parse_config_with_warnings(
            "[translation]\nsource_language = \"Klingon\"\ntarget_language = \"zh-TW\"\n",
        )
        .unwrap();
        assert_eq!(config.source_language, "Klingon");
        assert_eq!(config.target_language, "zh-TW");
        assert_eq!(
            warnings,
            vec![
                "source_language = \"Klingon\" is not a known language, used as a language code"
                    .to_string(),
                "target_language = \"zh-TW\" is not a known language, used as a language code"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn an_auto_target_in_the_file_is_replaced_with_a_warning() {
        let (config, warnings) =
            parse_config_with_warnings("[translation]\ntarget_language = \"Auto\"\n").unwrap();
        assert_eq!(config.target_language, "en");
        assert_eq!(
            warnings,
            vec!["Target can't be Auto; using English instead (target_language)".to_string()]
        );
    }

    #[test]
    fn language_names_come_from_the_codes() {
        let config = Config {
            source_language: "en".to_string(),
            ..Config::default()
        };
        assert_eq!(config.source_language_name(), "English");
        let config = Config {
            source_language: "uk".to_string(),
            ..config
        };
        assert_eq!(config.source_language_name(), "uk");
    }

    /// A hand-edited `target_language = "Auto"` is replaced in memory on load.
    #[test]
    fn test_load_config_replaces_auto_target_language() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_auto_target_{}.toml",
            std::process::id()
        ));
        fs::write(
            &path,
            "[translation]\nsource_language = \"Auto\"\ntarget_language = \"Auto\"\n",
        )
        .unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert_eq!(manager.get_config().source_language, "auto");
        assert_eq!(manager.get_config().target_language, AUTO_TARGET_FALLBACK);
        // The file itself is not rewritten.
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("target_language = \"Auto\""));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_parse_single_key() {
        // F9 should parse correctly
        let result = HotkeyParser::parse("F9").unwrap();
        assert!(matches!(result, HotkeyType::SingleKey { vk_code: _ }));

        let result = HotkeyParser::parse("f9").unwrap();
        assert!(matches!(result, HotkeyType::SingleKey { vk_code: _ }));

        // Space should parse but fail validation (tested separately)
        let result = HotkeyParser::parse("Space").unwrap();
        assert!(matches!(result, HotkeyType::SingleKey { vk_code: _ }));
    }

    #[test]
    fn test_single_key_validation() {
        // F1-F12 should pass validation
        let hotkey = HotkeyParser::parse("F9").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("F1").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("F12").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        // Other single keys should fail validation
        let hotkey = HotkeyParser::parse("Space").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Tab").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Enter").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());
    }

    #[test]
    fn test_parse_modifier_combo() {
        let result = HotkeyParser::parse("Alt+Space").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));

        let result = HotkeyParser::parse("Ctrl+Shift+C").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));

        let result = HotkeyParser::parse("Win+T").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));
    }

    #[test]
    fn test_shift_only_validation() {
        // Shift+Key should fail validation
        let hotkey = HotkeyParser::parse("Shift+T").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Shift+Space").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        // Multi-modifier with Shift should pass validation
        let hotkey = HotkeyParser::parse("Ctrl+Shift+T").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Alt+Shift+Q").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());
    }

    #[test]
    fn test_parse_double_press() {
        let result = HotkeyParser::parse("Ctrl+Ctrl").unwrap();
        assert!(matches!(result, HotkeyType::DoublePress { .. }));

        let result = HotkeyParser::parse("F8+F8").unwrap();
        assert!(matches!(result, HotkeyType::DoublePress { .. }));
    }

    #[test]
    fn test_modifier_combo_normalizes_lr_modifiers() {
        let result = HotkeyParser::parse("LAlt+Q").unwrap();
        match result {
            HotkeyType::ModifierCombo { modifiers, .. } => {
                assert_eq!(modifiers, vec![super::keycodes::KEY_ALT]);
            }
            _ => panic!("expected ModifierCombo"),
        }

        let result = HotkeyParser::parse("RCtrl+Shift+T").unwrap();
        match result {
            HotkeyType::ModifierCombo { modifiers, .. } => {
                assert!(modifiers.contains(&super::keycodes::KEY_CONTROL));
                assert!(modifiers.contains(&super::keycodes::KEY_SHIFT));
                assert!(!modifiers.contains(&super::keycodes::KEY_RCONTROL));
            }
            _ => panic!("expected ModifierCombo"),
        }
    }

    #[test]
    fn test_modifier_combo_key_field_not_normalized() {
        // The trigger key (last part) is compared against the raw observed vk_code by the
        // keyboard hooks, so it must stay left/right-specific instead of being normalized.
        let result = HotkeyParser::parse("Ctrl+LAlt").unwrap();
        match result {
            HotkeyType::ModifierCombo { key, .. } => {
                assert_eq!(key, super::keycodes::KEY_LALT);
            }
            _ => panic!("expected ModifierCombo"),
        }
    }

    #[test]
    fn test_double_press_normalizes_lr_target() {
        let result = HotkeyParser::parse("LCtrl+LCtrl").unwrap();
        match result {
            HotkeyType::DoublePress { vk_code, .. } => {
                assert_eq!(vk_code, super::keycodes::KEY_CONTROL);
            }
            _ => panic!("expected DoublePress"),
        }
    }

    #[test]
    fn test_double_press_plain_ctrl_unaffected() {
        let result = HotkeyParser::parse("Ctrl+Ctrl").unwrap();
        match result {
            HotkeyType::DoublePress { vk_code, .. } => {
                assert_eq!(vk_code, super::keycodes::KEY_CONTROL);
            }
            _ => panic!("expected DoublePress"),
        }
    }

    #[test]
    fn test_invalid_inputs() {
        assert!(HotkeyParser::parse("InvalidKey").is_err());
        assert!(HotkeyParser::parse("").is_err());
    }

    #[test]
    fn test_system_shortcut_validation() {
        let hotkey = HotkeyParser::parse("Ctrl+Alt+Delete").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Win+L").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());
    }

    #[test]
    fn validate_refuses_non_modifiers_held_and_modifiers_last() {
        for hotkey in [
            "A+Q",
            "Ctrl+A+Q",
            "F5+Q",
            "Ctrl+Shift",
            "Alt+Ctrl",
            "Ctrl+LShift",
        ] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(HotkeyParser::validate_hotkey(&parsed).is_err(), "{hotkey}");
        }
    }

    #[test]
    fn validate_accepts_only_letters_digits_and_function_keys_last() {
        for hotkey in [
            "Alt+Tab",
            "Ctrl+Tab",
            "Ctrl+Esc",
            "Alt+Space",
            "Ctrl+Space",
            "Ctrl+Enter",
            "Alt+Enter",
            "Ctrl+Backspace",
            "Ctrl+Delete",
            "Ctrl+Alt+Delete",
            "Ctrl+Insert",
            "Ctrl+Left",
            "Alt+Right",
            "Ctrl+Shift+Up",
            "Ctrl+Home",
            "Ctrl+End",
            "Ctrl+PageUp",
            "Ctrl+PageDown",
        ] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(HotkeyParser::validate_hotkey(&parsed).is_err(), "{hotkey}");
        }
        for hotkey in [
            "Alt+A",
            "Alt+S",
            "Ctrl+Q",
            "Ctrl+Shift+T",
            "Alt+Shift+7",
            "Ctrl+F9",
        ] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(HotkeyParser::validate_hotkey(&parsed).is_ok(), "{hotkey}");
        }
    }

    #[test]
    fn validate_refuses_ctrl_clipboard_and_undo_keys_alone() {
        for hotkey in [
            "Ctrl+A", "Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+Y", "Ctrl+Z", "RCtrl+C",
        ] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(HotkeyParser::validate_hotkey(&parsed).is_err(), "{hotkey}");
        }
        // With another modifier they are other shortcuts and stay allowed.
        for hotkey in ["Ctrl+Shift+C", "Ctrl+Alt+V"] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(HotkeyParser::validate_hotkey(&parsed).is_ok(), "{hotkey}");
        }
    }

    #[test]
    fn validate_refuses_double_alt() {
        let hotkey = HotkeyParser::parse("Alt+Alt").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());
    }

    #[test]
    fn validate_refuses_every_hotkey_with_win() {
        for hotkey in [
            "Win+T",
            "LWin+T",
            "RWin+T",
            "Ctrl+Win+T",
            "Win+Alt+Q",
            "Win+Win",
        ] {
            let parsed = HotkeyParser::parse(hotkey).unwrap();
            assert!(parsed.uses_win(), "{hotkey}");
            let err = HotkeyParser::validate_hotkey(&parsed).unwrap_err();
            assert!(
                err.contains("Win (Super) is not allowed"),
                "{hotkey}: {err}"
            );
        }
        for hotkey in ["Alt+A", "Ctrl+Shift+T", "F9", "Ctrl+Ctrl", "Shift+Shift"] {
            assert!(!HotkeyParser::parse(hotkey).unwrap().uses_win(), "{hotkey}");
        }
    }

    #[test]
    fn test_double_press_validation_only_allows_f1_to_f12_or_modifiers() {
        let hotkey = HotkeyParser::parse("F8+F8").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Ctrl+Ctrl").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Shift+Shift").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Alt+Alt").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        // Doubling an ordinary letter/digit/space/etc. is indistinguishable from
        // just typing that key twice while using the app normally -- must be rejected.
        let hotkey = HotkeyParser::parse("Q+Q").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("A+A").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("5+5").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Space+Space").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());
    }

    #[test]
    fn test_load_config_missing_speech_section() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_missing_speech_{}.toml",
            std::process::id()
        ));
        fs::write(
            &path,
            "[translation]\nsource_language = \"Auto\"\ntarget_language = \"Russian\"\n",
        )
        .unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert!(manager.get_config().enable_text_to_speech);
        assert_eq!(manager.get_config().speech_provider, "google");

        let _ = fs::remove_file(&path);
    }

    /// The speech hotkey default is `Alt+S` (the same as `tagent-gui`'s) and must agree in every
    /// place that spells it out: `Config::default()`, the fallback for a config file without the
    /// key, and the comments of a freshly generated config file.
    #[test]
    fn test_speech_hotkey_default_is_alt_s_everywhere() {
        assert_eq!(Config::default().speech_hotkey, "Alt+S");

        let path = std::env::temp_dir().join(format!(
            "tagent_test_speech_hotkey_default_{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[speech]\nenable_text_to_speech = true\n").unwrap();
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();
        assert_eq!(manager.get_config().speech_hotkey, "Alt+S");
        let _ = fs::remove_file(&path);

        let generated = render_config(&Config::default());
        // Anchored to the start of a line so the commented example (`#   speech_hotkey = ...`)
        // cannot satisfy it: this must be the live setting.
        assert!(generated.contains("\nspeech_hotkey = \"Alt+S\"\n"));
        assert!(
            !generated.contains("Alt+E"),
            "generated config still mentions the old Alt+E default"
        );
    }

    #[test]
    fn test_load_config_explicit_speech_provider_is_respected() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_explicit_speech_provider_{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[speech]\nspeech_provider = \"other\"\n").unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert_eq!(manager.get_config().speech_provider, "other");
        // Independent of the translate provider.
        assert_eq!(manager.get_config().translate_provider, "google");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_config_dictionary_provider_defaults_when_absent() {
        // Both when the whole `[dictionary]` section is missing and when the section is
        // present without the key (an older config file).
        for (label, content) in [
            ("no_section", "[translation]\nsource_language = \"Auto\"\n"),
            ("no_key", "[dictionary]\nshow_dictionary = false\n"),
        ] {
            let path = std::env::temp_dir().join(format!(
                "tagent_test_dictionary_provider_default_{}_{}.toml",
                label,
                std::process::id()
            ));
            fs::write(&path, content).unwrap();

            let manager = ConfigManager {
                config_path: path.to_str().unwrap().to_string(),
                config: Arc::new(Mutex::new(Config::default())),
                last_modified: Arc::new(Mutex::new(None)),
            };
            manager.load_config().unwrap();

            assert_eq!(
                manager.get_config().dictionary_provider,
                "google",
                "{label}"
            );

            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn test_load_config_explicit_dictionary_provider_is_respected() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_explicit_dictionary_provider_{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[dictionary]\ndictionary_provider = \"other\"\n").unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert_eq!(manager.get_config().dictionary_provider, "other");
        // Independent of the other two provider axes.
        assert_eq!(manager.get_config().translate_provider, "google");
        assert_eq!(manager.get_config().speech_provider, "google");

        let _ = fs::remove_file(&path);
    }

    /// `render_config` and the two `ConfigFile` conversions list every field by hand, so a
    /// field mapped to the wrong key would go unnoticed if its value happened to equal the
    /// default. Every field here is deliberately non-default (bools flipped, strings unique
    /// sentinels).
    #[test]
    fn test_generated_config_roundtrips_every_field_with_distinct_values() {
        let defaults = Config::default();
        let config = Config {
            source_language: "src-sentinel".to_string(),
            target_language: "tgt-sentinel".to_string(),
            show_terminal_on_translate: !defaults.show_terminal_on_translate,
            auto_hide_terminal_seconds: 17,
            show_dictionary: !defaults.show_dictionary,
            spell_check: !defaults.spell_check,
            dictionary_provider: "dict-sentinel".to_string(),
            copy_to_clipboard: !defaults.copy_to_clipboard,
            save_translation_history: !defaults.save_translation_history,
            history_file: "history-sentinel.txt".to_string(),
            use_colors: "use-colors-sentinel".to_string(),
            target_prompt_color: "target-color-sentinel".to_string(),
            dictionary_prompt_color: "dict-color-sentinel".to_string(),
            source_prompt_color: "source-color-sentinel".to_string(),
            part_of_speech_color: "pos-color-sentinel".to_string(),
            synonym_color: "synonym-color-sentinel".to_string(),
            notice_color: "notice-color-sentinel".to_string(),
            error_color: "error-color-sentinel".to_string(),
            translate_hotkey: "translate-hotkey-sentinel".to_string(),
            enable_text_to_speech: !defaults.enable_text_to_speech,
            speech_hotkey: "speech-hotkey-sentinel".to_string(),
            enable_speech_hotkey: !defaults.enable_speech_hotkey,
            translate_provider: "translate-sentinel".to_string(),
            speech_provider: "speech-sentinel".to_string(),
            provider_options: {
                let mut profiles = ProviderProfiles::new();
                profiles.insert("profile-sentinel", "key-sentinel", "value-sentinel");
                profiles
            },
        };

        let path = std::env::temp_dir().join(format!(
            "tagent_test_roundtrip_all_fields_{}.toml",
            std::process::id()
        ));
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        fs::write(&path, render_config(&config)).unwrap();
        manager.load_config().unwrap();
        let _ = fs::remove_file(&path);

        let loaded = manager.get_config();
        assert_eq!(loaded.source_language, config.source_language);
        assert_eq!(loaded.target_language, config.target_language);
        assert_eq!(
            loaded.show_terminal_on_translate,
            config.show_terminal_on_translate
        );
        assert_eq!(
            loaded.auto_hide_terminal_seconds,
            config.auto_hide_terminal_seconds
        );
        assert_eq!(loaded.show_dictionary, config.show_dictionary);
        assert_eq!(loaded.spell_check, config.spell_check);
        assert_eq!(loaded.dictionary_provider, config.dictionary_provider);
        assert_eq!(loaded.copy_to_clipboard, config.copy_to_clipboard);
        assert_eq!(loaded.use_colors, config.use_colors);
        assert_eq!(
            loaded.save_translation_history,
            config.save_translation_history
        );
        assert_eq!(loaded.history_file, config.history_file);
        assert_eq!(loaded.target_prompt_color, config.target_prompt_color);
        assert_eq!(
            loaded.dictionary_prompt_color,
            config.dictionary_prompt_color
        );
        assert_eq!(loaded.source_prompt_color, config.source_prompt_color);
        assert_eq!(loaded.part_of_speech_color, config.part_of_speech_color);
        assert_eq!(loaded.synonym_color, config.synonym_color);
        assert_eq!(loaded.notice_color, config.notice_color);
        assert_eq!(loaded.error_color, config.error_color);
        assert_eq!(loaded.translate_hotkey, config.translate_hotkey);
        assert_eq!(loaded.enable_text_to_speech, config.enable_text_to_speech);
        assert_eq!(loaded.speech_hotkey, config.speech_hotkey);
        assert_eq!(loaded.enable_speech_hotkey, config.enable_speech_hotkey);
        assert_eq!(loaded.translate_provider, config.translate_provider);
        assert_eq!(loaded.speech_provider, config.speech_provider);
        assert_eq!(loaded.provider_options, config.provider_options);
    }

    #[test]
    fn provider_error_message_lists_supported_values_for_unknown_provider() {
        let error = tagent::error::Error::UnknownProvider("bogus".to_string());
        assert_eq!(
            provider_error_message(&error, "speech_provider", "bogus", &["google", "other"]),
            "unknown provider: bogus (supported values for speech_provider: google, other)"
        );
    }

    #[test]
    fn provider_error_message_leaves_other_errors_unchanged() {
        let error = tagent::error::Error::Network("down".to_string());
        assert_eq!(
            provider_error_message(&error, "speech_provider", "google", &["google"]),
            error.to_string()
        );
    }

    /// The `Supported values:` comments in a generated config come from `tagent`'s own
    /// lists, so they can't go stale when a backend is added.
    #[test]
    fn generated_config_comments_list_the_providers_tagent_offers() {
        let toml = render_config(&Config::default());
        for list in [
            tagent::providers::TRANSLATION_PROVIDERS,
            tagent::providers::DICTIONARY_PROVIDERS,
            tagent::providers::SPEECH_PROVIDERS,
        ] {
            let line = format!("# Supported values: {}\n", list.join(", "));
            assert!(toml.contains(&line), "missing {line:?} in generated config");
        }
        assert!(!toml.contains("{translate_providers}"));
    }

    /// A config file written before the article/status colors existed gets their defaults.
    #[test]
    fn test_load_config_article_colors_default_when_absent() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_missing_article_colors_{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[colors]\ntarget_prompt_color = \"Blue\"\n").unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();
        let config = manager.get_config();
        let _ = fs::remove_file(&path);

        let defaults = Config::default();
        assert_eq!(config.target_prompt_color, "Blue");
        assert_eq!(config.part_of_speech_color, defaults.part_of_speech_color);
        assert_eq!(config.synonym_color, defaults.synonym_color);
        assert_eq!(config.notice_color, defaults.notice_color);
        assert_eq!(config.error_color, defaults.error_color);
    }

    fn sample_article() -> Vec<tagent::article::Line> {
        use tagent::providers::{Definition, DictionaryEntry, PartOfSpeechEntry};
        let entry = DictionaryEntry::new(
            "violent",
            vec![PartOfSpeechEntry::new(
                "adjective",
                vec![
                    Definition::new("жестокий", vec!["brutal".to_string()]),
                    Definition::new("сильный", vec![]),
                ],
            )],
        );
        tagent::article::article_lines(&entry, "ru", Some("насильственный"))
    }

    #[test]
    fn use_colors_parses_in_any_case() {
        assert_eq!(UseColors::parse("auto"), Some(UseColors::Auto));
        assert_eq!(UseColors::parse(" Always "), Some(UseColors::Always));
        assert_eq!(UseColors::parse("NEVER"), Some(UseColors::Never));
        assert_eq!(UseColors::parse("yes"), None);
        assert_eq!(UseColors::parse(""), None);
    }

    /// `always`/`never` decide alone; `auto` turns colors off for `TERM=dumb` and a
    /// console without escape sequences, and otherwise leaves the decision to `colored`.
    #[test]
    fn color_override_follows_use_colors_term_and_console() {
        use UseColors::*;
        for (term, ansi) in [(None, true), (Some("dumb"), true), (Some("xterm"), false)] {
            assert_eq!(color_override(Always, term, ansi), Some(true));
            assert_eq!(color_override(Never, term, ansi), Some(false));
        }
        assert_eq!(color_override(Auto, None, true), None);
        assert_eq!(color_override(Auto, Some("xterm-256color"), true), None);
        assert_eq!(color_override(Auto, Some("dumb"), true), Some(false));
        assert_eq!(color_override(Auto, Some("xterm"), false), Some(false));
        assert_eq!(color_override(Auto, None, false), Some(false));
    }

    /// `use_colors` defaults to `auto`; an unknown value is kept as written (it acts as
    /// `auto`) and warned about.
    #[test]
    fn use_colors_default_and_unknown_value() {
        let (config, warnings) = parse_config_with_warnings("").unwrap();
        assert_eq!(config.use_colors, "auto");
        assert!(warnings.is_empty(), "{warnings:?}");

        let (config, warnings) =
            parse_config_with_warnings("[colors]\nuse_colors = \"Never\"\n").unwrap();
        assert_eq!(UseColors::parse(&config.use_colors), Some(UseColors::Never));
        assert!(warnings.is_empty(), "{warnings:?}");

        let (config, warnings) =
            parse_config_with_warnings("[colors]\nuse_colors = \"yes\"\n").unwrap();
        assert_eq!(config.use_colors, "yes");
        assert_eq!(
            warnings,
            vec!["use_colors = \"yes\" is not one of auto, always, never; auto is used"]
        );
    }

    /// Only part-of-speech labels and synonyms are colored; header and definition text
    /// keep the terminal's own foreground.
    #[test]
    fn article_role_color_maps_roles_to_config_keys() {
        use tagent::article::Role;
        let config = Config {
            part_of_speech_color: "pos".to_string(),
            synonym_color: "syn".to_string(),
            ..Config::default()
        };
        assert_eq!(article_role_color(Role::PartOfSpeech, &config), "pos");
        assert_eq!(article_role_color(Role::Synonym, &config), "syn");
        assert_eq!(article_role_color(Role::Header, &config), "None");
        assert_eq!(article_role_color(Role::Plain, &config), "None");
    }

    /// With every article color set to `"None"`, the rendered article must be exactly
    /// the plain text that goes to the clipboard and the history file.
    #[test]
    fn render_article_without_colors_equals_plain_text() {
        let lines = sample_article();
        let config = Config {
            part_of_speech_color: "None".to_string(),
            synonym_color: "None".to_string(),
            ..Config::default()
        };
        assert_eq!(
            render_article(&lines, &config),
            tagent::article::to_plain(&lines)
        );
        assert_eq!(
            tagent::article::to_plain(&lines),
            "насильственный\nПрилагательное\n  жестокий [brutal]\n  сильный"
        );
    }

    #[test]
    fn test_load_config_explicit_false_is_respected() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_explicit_false_speech_{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[speech]\nenable_text_to_speech = false\n").unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert!(!manager.get_config().enable_text_to_speech);

        let _ = fs::remove_file(&path);
    }

    // --- Provider profiles ([provider_options.<name>] tables) ---------------------------

    fn manager_at(label: &str) -> (ConfigManager, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_profiles_{}_{}.toml",
            label,
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        (manager, path)
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    const PROFILES_TOML: &str = r#"[provider]
translate_provider = "work"

[provider_options.Work]
type = "google"
API_Key = "file-key-0123456789"
timeout_secs = "15"

[provider_options.deepl]
api_key = "deepl-key"

[provider_options.empty]
"#;

    /// Profile names and keys are lowercased on load, and `/save` (which edits the file in
    /// place) leaves the profile tables exactly as they were.
    #[test]
    fn provider_profiles_survive_save() {
        let (manager, path) = manager_at("roundtrip");
        fs::write(&path, PROFILES_TOML).unwrap();
        manager.load_config().unwrap();
        let loaded = manager.get_config();
        assert_eq!(loaded.translate_provider, "work");
        let work = loaded.provider_options.get("work").unwrap();
        assert_eq!(work["type"], "google");
        assert_eq!(work["api_key"], "file-key-0123456789");
        assert_eq!(work["timeout_secs"], "15");
        assert_eq!(
            loaded.provider_options.get("deepl").unwrap()["api_key"],
            "deepl-key"
        );
        assert!(loaded.provider_options.get("empty").unwrap().is_empty());

        manager.save_config().unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains(
                "[provider_options.Work]\ntype = \"google\"\nAPI_Key = \"file-key-0123456789\""
            ),
            "{written}"
        );
        assert!(written.contains("[provider_options.empty]\n"), "{written}");
        manager.load_config().unwrap();
        assert_eq!(
            manager.get_config().provider_options,
            loaded.provider_options
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn provider_options_env_overrides_file() {
        let (manager, path) = manager_at("env");
        fs::write(&path, PROFILES_TOML).unwrap();
        manager.load_config().unwrap();
        let config = manager.get_config();
        let _ = fs::remove_file(&path);

        let from_file = config.provider_options_using("Work", no_env);
        assert_eq!(from_file.get("api_key"), Some("file-key-0123456789"));
        assert_eq!(from_file.get("type"), Some("google"));

        let env = |var: &str| (var == "TAGENT_WORK_API_KEY").then(|| "env-key".to_string());
        let from_env = config.provider_options_using("work", env);
        assert_eq!(from_env.get("api_key"), Some("env-key"));
        assert_eq!(from_env.get("timeout_secs"), Some("15"));

        // A profile without a section still gets its key from the environment.
        let env = |var: &str| (var == "TAGENT_OTHER_API_KEY").then(|| "k".to_string());
        assert_eq!(
            config.provider_options_using("other", env).get("api_key"),
            Some("k")
        );
    }

    #[test]
    fn profile_lines_mask_secrets_and_name_their_origin() {
        let (manager, path) = manager_at("display");
        fs::write(&path, PROFILES_TOML).unwrap();
        manager.load_config().unwrap();
        let config = manager.get_config();
        let _ = fs::remove_file(&path);

        let sentinel = "ENV-SENTINEL-KEY-4711";
        let env = |var: &str| (var == "TAGENT_DEEPL_API_KEY").then(|| sentinel.to_string());
        let text = config.provider_profile_lines(env).join("\n");
        assert!(!text.contains(sentinel), "{text}");
        assert!(!text.contains("file-key-0123456789"), "{text}");
        assert!(
            text.contains("[provider_options.work]\ntype = \"google\"\napi_key = \"••••6789\"\ntimeout_secs = \"15\""),
            "{text}"
        );
        assert!(
            text.contains("api_key = \"••••4711\"  # from env TAGENT_DEEPL_API_KEY"),
            "{text}"
        );
        // The selected built-in providers are listed too.
        assert!(
            text.contains("[provider_options.google]\n# (no options)"),
            "{text}"
        );
        // Debug output (a log line, a panic message) never shows a configured key.
        let debug = format!("{config:?}");
        assert!(!debug.contains("file-key-0123456789"), "{debug}");
        assert!(!debug.contains("deepl-key"), "{debug}");
    }

    /// `/config` shows every setting under its file key, formatted as in the file, so it
    /// can be copied into `tagent-cli.toml` as is.
    #[test]
    fn display_lines_use_the_file_keys() {
        let config = Config {
            target_language: "ru".to_string(),
            history_file: "history.txt".to_string(),
            ..Config::default()
        };
        let lines = config.display_lines(no_env);
        let text = lines.join("\n");
        assert!(
            text.starts_with("[provider]\ntranslate_provider = \"google\"\n"),
            "{text}"
        );
        assert!(
            text.contains("[translation]\nsource_language = \"auto\"  # Auto\ntarget_language = \"ru\"  # Russian\n"),
            "{text}"
        );
        assert!(
            text.contains("\nauto_hide_terminal_seconds = 3\n"),
            "{text}"
        );
        assert!(
            text.contains("\n[colors]\nuse_colors = \"auto\"\nsource_prompt_color = \"None\"\n"),
            "{text}"
        );

        // Every `key = value` line matches the file: parsed back as TOML (the profile
        // comment lines aside), it is the same configuration.
        let settings: Vec<&str> = lines
            .iter()
            .map(String::as_str)
            .take_while(|line| !line.starts_with("# Provider profiles"))
            .collect();
        assert_eq!(parse_config(&settings.join("\n")).unwrap(), config);
    }

    #[test]
    fn mask_secret_hides_short_values_entirely() {
        assert_eq!(mask_secret("short"), "••••");
        assert_eq!(mask_secret("abcdefghijkl"), "••••ijkl");
    }

    #[test]
    fn invalid_profile_options_name_the_section() {
        let config = Config {
            translate_provider: "google".to_string(),
            provider_options: {
                let mut profiles = ProviderProfiles::new();
                profiles.insert("google", "timeout_secs", "0");
                profiles
            },
            ..Config::default()
        };
        let message = config
            .create_translate_provider()
            .err()
            .expect("invalid timeout");
        assert!(message.contains("timeout_secs"), "{message}");
        assert!(message.contains("[provider_options.google]"), "{message}");
        assert!(message.contains("TAGENT_GOOGLE_<KEY>"), "{message}");
        // The other axes of the same profile fail the same way.
        assert!(config.create_dictionary_provider().is_err());
        assert!(config.create_speech_provider().is_err());
    }

    /// DeepL comes from `tagent`'s `deepl` feature, which this crate enables. The profile
    /// names are unusual so a `TAGENT_DEEPL_API_KEY` in the developer's shell can't
    /// interfere.
    #[test]
    fn deepl_profile_from_the_config_file_builds() {
        let config = parse_config(
            r#"
            [provider]
            translate_provider = "cli-test-deepl"

            [provider_options.cli-test-deepl]
            type = "deepl"
            api_key = "dummy-key:fx"

            [provider_options.cli-test-nokey]
            type = "deepl"
            "#,
        )
        .unwrap();
        let provider = config.create_translate_provider().unwrap();
        assert_eq!(provider.name(), "DeepL (cli-test-deepl)");
        assert!(tagent::providers::TRANSLATION_PROVIDERS.contains(&"deepl"));

        let no_key = Config {
            translate_provider: "cli-test-nokey".to_string(),
            ..config
        };
        let message = no_key
            .create_translate_provider()
            .err()
            .expect("no api_key");
        assert!(message.contains("api_key"), "{message}");
        assert!(
            message.contains("[provider_options.cli-test-nokey]"),
            "{message}"
        );
    }

    #[test]
    fn default_config_builds_every_provider() {
        let config = Config::default();
        assert!(config.create_translate_provider().is_ok());
        assert!(config.create_dictionary_provider().is_ok());
        assert!(config.create_speech_provider().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn config_file_is_private_after_every_write() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;

        // Created by the manager (default config).
        let (manager, path) = manager_at("mode");
        manager.create_default_config().unwrap();
        assert_eq!(mode(&path), 0o600);

        // An existing, world-readable file is tightened by /save.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        manager.save_config().unwrap();
        assert_eq!(mode(&path), 0o600);
        let _ = fs::remove_file(&path);
    }

    // --- TOML format ----------------------------------------------------------------

    /// A freshly generated file means exactly the defaults.
    #[test]
    fn generated_template_parses_back_into_defaults() {
        let toml = render_config(&Config::default());
        assert_eq!(parse_config(&toml).unwrap(), Config::default());
        // No profiles: no `[provider_options]` header at all, and the explanation stays.
        assert!(!toml.contains("\n[provider_options"), "{toml}");
        assert!(toml.contains("# Provider profiles\n"), "{toml}");
        // The literal template is itself valid and matches the defaults, apart from
        // `history_file`, which depends on the platform's data directory.
        let template = parse_config(&config_template()).unwrap();
        assert_eq!(
            template,
            Config {
                history_file: String::new(),
                ..Config::default()
            }
        );
    }

    /// `toml` with the commented-out example block `[provider_options.<name>]` enabled the
    /// way the file tells the user to: `"# "` removed from each line of the block.
    pub(super) fn uncomment_example(toml: &str, name: &str) -> String {
        let header = format!("# [provider_options.{name}]");
        let mut in_block = false;
        toml.lines()
            .map(|line| {
                if line == header {
                    in_block = true;
                } else if in_block && !line.starts_with("# ") {
                    in_block = false;
                }
                let line = match line.strip_prefix("# ") {
                    Some(rest) if in_block => rest,
                    _ => line,
                };
                format!("{line}\n")
            })
            .collect()
    }

    /// `toml` with the commented-out multi-line strings (`#<key> = """` ... `#"""`, the
    /// prompts) of an enabled example block enabled too (`#` removed).
    pub(super) fn uncomment_prompt(toml: &str) -> String {
        let mut in_prompt = false;
        toml.lines()
            .map(|line| {
                if line.starts_with('#') && line.ends_with(" = \"\"\"") {
                    in_prompt = true;
                }
                let line = match line.strip_prefix('#') {
                    Some(rest) if in_prompt => {
                        if rest == "\"\"\"" {
                            in_prompt = false;
                        }
                        rest
                    }
                    _ => line,
                };
                format!("{line}\n")
            })
            .collect()
    }

    /// The `openai` example with its prompts enabled loads the built-in prompts, which the
    /// provider then uses as they are (the value's final newline is trimmed away).
    #[test]
    fn the_openai_example_prompts_load_as_the_default_prompts() {
        let toml = uncomment_prompt(&uncomment_example(
            &render_config(&Config::default()),
            "openai",
        ));
        let mut config = parse_config(&toml).unwrap_or_else(|e| panic!("{e}\n{toml}"));
        let options = config.provider_options.get("openai").unwrap();
        assert_eq!(
            options["translate_prompt"].trim(),
            providers::openai::DEFAULT_TRANSLATE_PROMPT
        );
        assert_eq!(
            options["dictionary_prompt"].trim(),
            providers::openai::DEFAULT_DICTIONARY_PROMPT
        );
        config
            .provider_options
            .insert("openai", "endpoint", "http://localhost:11434/v1");
        config
            .provider_options
            .insert("openai", "model", "qwen3:8b");
        build_example_profile(&config, "openai", "openai").unwrap();
    }

    /// Builds profile `name` on every axis its kind serves, with the file's options only.
    fn build_example_profile(config: &Config, kind: &str, name: &str) -> Result<(), String> {
        let options = config.provider_options_using(name, no_env);
        let built = |result: Result<(), tagent::error::Error>| result.map_err(|e| e.to_string());
        if providers::TRANSLATION_PROVIDERS.contains(&kind) {
            built(providers::create_provider_with(name, &options).map(drop))?;
        }
        if providers::DICTIONARY_PROVIDERS.contains(&kind) {
            built(providers::create_dictionary_provider_with(name, &options).map(drop))?;
        }
        if providers::SPEECH_PROVIDERS.contains(&kind) {
            built(providers::create_speech_provider_with(name, &options).map(drop))?;
        }
        Ok(())
    }

    /// A kind serving several axes under one display name names it once in its header.
    #[test]
    fn example_headers_name_each_display_name_once() {
        let toml = render_config(&Config::default());
        assert!(toml.contains("\n## openai: OpenAI-compatible\n"), "{toml}");
        assert!(
            toml.contains("\n## google: Google Translate, Google Dictionary, Google TTS\n"),
            "{toml}"
        );
    }

    /// Every built-in provider kind gets a commented-out example with every option it
    /// declares, and the examples change nothing until uncommented.
    #[test]
    fn generated_config_has_an_example_profile_per_provider() {
        let toml = render_config(&Config::default());
        let kinds = provider_kinds();
        assert!(kinds.iter().any(|(kind, _)| *kind == "deepl"), "{kinds:?}");
        for (kind, descriptors) in kinds {
            let header = format!("\n# [provider_options.{kind}]\n");
            let start = toml
                .find(&header)
                .unwrap_or_else(|| panic!("{kind}: {toml}"));
            let block: String = toml[start + 1..]
                .lines()
                .take_while(|line| line.starts_with("# "))
                .collect::<Vec<_>>()
                .join("\n");
            for option in descriptors.iter().flat_map(|d| d.options) {
                assert!(
                    block.contains(&format!("{} = ", option.key)),
                    "{kind}/{}: {block}",
                    option.key
                );
            }
        }
        assert!(!toml.contains("{profile_examples}"));
        assert_eq!(parse_config(&toml).unwrap(), Config::default());
    }

    /// An uncommented example is a working profile: a kind without required options
    /// builds as is, with its transport defaults; one with required options fails until
    /// they are filled in, naming the missing key. The environment is not consulted.
    #[test]
    fn uncommented_example_profiles_build() {
        let toml = render_config(&Config::default());
        let mut names: Vec<(&str, String)> = provider_kinds()
            .into_iter()
            .map(|(kind, _)| (kind, kind.to_string()))
            .collect();
        names.push(("deepl", "deepl-work".to_string()));
        for (kind, name) in names {
            let enabled = uncomment_example(&toml, &name);
            let config =
                parse_config(&enabled).unwrap_or_else(|e| panic!("{name}: {e}\n{enabled}"));
            let options = config.provider_options.get(&name).expect(&name);
            if kind != name {
                assert_eq!(options["type"], kind);
            }
            let required: Vec<&str> = provider_kinds()
                .into_iter()
                .filter(|(k, _)| *k == kind)
                .flat_map(|(_, descriptors)| descriptors)
                .flat_map(|d| d.options)
                .filter(|o| o.required)
                .map(|o| o.key)
                .collect();
            if required.is_empty() {
                build_example_profile(&config, kind, &name)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(options.contains_key("timeout_secs"), "{name}: {options:?}");
                continue;
            }
            let error = build_example_profile(&config, kind, &name).unwrap_err();
            assert!(error.contains(required[0]), "{name}: {error}");

            let mut filled = config.clone();
            for key in &required {
                let value = match *key {
                    "endpoint" => "http://localhost:11434/v1",
                    _ => "dummy-value:fx",
                };
                filled.provider_options.insert(&name, key, value);
            }
            build_example_profile(&filled, kind, &name)
                .unwrap_or_else(|e| panic!("{name} filled: {e}"));
        }
    }

    /// Each example block starts with its `##` title lines, uncommented; every other line
    /// down to the separating `"#"` is commented out with `"# "`.
    #[test]
    fn example_titles_are_plain_comments_above_commented_out_lines() {
        let toml = render_config(&Config::default());
        let examples = &toml[toml.find("\n#\n## google:").expect(&toml) + 3..];
        let mut titles = 0;
        for block in examples.trim_end().split("\n#\n") {
            let lines: Vec<&str> = block.lines().filter(|line| !line.is_empty()).collect();
            let title_end = lines
                .iter()
                .position(|line| !line.starts_with("## "))
                .expect(block);
            assert!(title_end > 0, "{block}");
            titles += 1;
            assert!(
                lines[title_end].starts_with("# [provider_options."),
                "{block}"
            );
            for line in &lines[title_end..] {
                assert!(line.starts_with("# "), "{line}\n{block}");
            }
        }
        assert_eq!(titles, provider_kinds().len() + 2, "{examples}");
    }

    /// A profile of one's own name follows its kind's block: the second deepl profile
    /// after deepl, the Ollama profile after openai.
    #[test]
    fn named_examples_follow_their_kinds_block() {
        let toml = render_config(&Config::default());
        let position = |name: &str| {
            toml.find(&format!("\n# [provider_options.{name}]\n"))
                .unwrap_or_else(|| panic!("{name}: {toml}"))
        };
        assert!(position("deepl") < position("deepl-work"));
        assert!(position("deepl-work") < position("openai"));
        assert!(position("openai") < position("ollama"));
        assert_eq!(toml.matches("-work]\n").count(), 1, "{toml}");
    }

    /// The uncommented Ollama example is a complete profile of kind openai: it builds on
    /// both of the kind's axes as it is, with no unknown keys.
    #[test]
    fn uncommented_ollama_example_builds_as_is() {
        let enabled = uncomment_example(&render_config(&Config::default()), "ollama");
        assert!(unknown_keys_in(&enabled).is_empty(), "{enabled}");
        let config = parse_config(&enabled).unwrap_or_else(|e| panic!("{e}\n{enabled}"));
        let options = config.provider_options.get("ollama").expect("ollama");
        assert_eq!(options["type"], "openai");
        assert_eq!(options["endpoint"], "http://localhost:11434/v1");
        build_example_profile(&config, "openai", "ollama").unwrap();
    }

    /// Real profiles are written below the examples, which stay comments.
    #[test]
    fn rendered_profiles_keep_the_examples_commented() {
        let mut provider_options = ProviderProfiles::new();
        provider_options.insert("google", "timeout_secs", "15");
        let config = Config {
            provider_options,
            ..Config::default()
        };
        let toml = render_config(&config);
        let example = toml.find("# [provider_options.google]\n").unwrap();
        let real = toml.find("\n[provider_options.google]\n").unwrap();
        assert!(example < real, "{toml}");
        assert_eq!(parse_config(&toml).unwrap(), config);
    }

    #[test]
    fn a_file_with_one_section_takes_defaults_for_the_rest() {
        let config = parse_config("[interface]\ncopy_to_clipboard = true\n").unwrap();
        assert_eq!(
            config,
            Config {
                copy_to_clipboard: true,
                ..Config::default()
            }
        );
        assert_eq!(parse_config("").unwrap(), Config::default());
    }

    #[test]
    fn unknown_sections_and_keys_are_ignored() {
        let config =
            parse_config("[interface]\nfuture_key = 1\n[future_section]\nx = \"y\"\n").unwrap();
        assert_eq!(config, Config::default());
    }

    /// Errors name the line and column and show the offending line (which holds the key).
    #[test]
    fn type_and_syntax_errors_name_the_line() {
        let error = parse_config("[interface]\n\ncopy_to_clipboard = \"yes\"\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 3"), "{error}");
        assert!(error.contains("copy_to_clipboard = \"yes\""), "{error}");
        assert!(error.contains("expected a boolean"), "{error}");

        let error = parse_config("[translation]\nsource_language = Auto\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("source_language = Auto"), "{error}");
    }

    /// The old INI file is not valid TOML: a leftover would fail loudly, not load silently.
    #[test]
    fn an_ini_style_file_is_rejected() {
        assert!(parse_config("[Translation]\nSourceLanguage = Auto\n").is_err());
    }

    /// Profile values are strings; a number names its line instead of being dropped.
    #[test]
    fn provider_option_values_must_be_strings() {
        let error = parse_config("[provider_options.google]\ntimeout_secs = 15\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("expected a string"), "{error}");
    }

    /// Backslashes (a Windows path) and quotes are escaped by the TOML writer.
    #[test]
    fn rendered_values_are_escaped() {
        let config = Config {
            history_file: r#"C:\Users\me\AppData\Roaming\tagent-cli\hist "1".txt"#.to_string(),
            ..Config::default()
        };
        assert_eq!(parse_config(&render_config(&config)).unwrap(), config);
    }

    /// Profiles are appended below the explanation, one table each, with no
    /// `[provider_options]` header of their own.
    #[test]
    fn rendered_profiles_follow_their_explanation() {
        let mut provider_options = ProviderProfiles::new();
        provider_options.insert("deepl-work", "type", "deepl");
        provider_options.insert("deepl-work", "api_key", "key-0123456789");
        provider_options.insert("google", "timeout_secs", "15");
        let config = Config {
            provider_options,
            ..Config::default()
        };
        let toml = render_config(&config);
        assert!(!toml.contains("\n[provider_options]"), "{toml}");
        let explanation = toml.find("# Provider profiles").unwrap();
        let first = toml.find("\n[provider_options.deepl-work]\n").unwrap();
        let second = toml.find("\n[provider_options.google]\n").unwrap();
        assert!(explanation < first && first < second, "{toml}");
        assert!(
            toml.ends_with("[provider_options.google]\ntimeout_secs = \"15\"\n"),
            "{toml}"
        );
        assert_eq!(parse_config(&toml).unwrap(), config);
    }

    const HAND_EDITED_TOML: &str = r#"# My own notes about this file

[translation]
# which languages I usually want
source_language = "Auto"  # auto-detect
target_language = "Russian"
my_future_key = "kept"

[interface]
copy_to_clipboard = true

[provider_options.deepl-work]
type = "deepl"
api_key = "secret-0123456789"  # from the account page
"#;

    /// `/save` changes only the languages: comments (above and after a key), key order,
    /// unknown keys, other sections and the provider profiles all survive byte for byte.
    #[test]
    fn save_edits_only_the_languages_in_place() {
        let (manager, path) = manager_at("save_in_place");
        fs::write(&path, HAND_EDITED_TOML).unwrap();
        manager.load_config().unwrap();
        manager.set_languages("en", "de");
        manager.save_config().unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let expected = HAND_EDITED_TOML
            .replace(
                "source_language = \"Auto\"  # auto-detect",
                "source_language = \"en\"  # auto-detect",
            )
            .replace("target_language = \"Russian\"", "target_language = \"de\"");
        assert_eq!(written, expected);

        manager.load_config().unwrap();
        let reloaded = manager.get_config();
        assert_eq!(
            (
                reloaded.source_language.as_str(),
                reloaded.target_language.as_str()
            ),
            ("en", "de")
        );
        assert!(reloaded.copy_to_clipboard);
        let _ = fs::remove_file(&path);
    }

    /// A file with language names gets codes on the first `/save`, even without `/l`;
    /// the comments stay.
    #[test]
    fn save_writes_language_names_as_codes() {
        let (manager, path) = manager_at("save_names_as_codes");
        fs::write(&path, HAND_EDITED_TOML).unwrap();
        manager.load_config().unwrap();
        manager.save_config().unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let expected = HAND_EDITED_TOML
            .replace(
                "source_language = \"Auto\"  # auto-detect",
                "source_language = \"auto\"  # auto-detect",
            )
            .replace("target_language = \"Russian\"", "target_language = \"ru\"");
        assert_eq!(written, expected);
        let _ = fs::remove_file(&path);
    }

    /// `/save` after `/p` writes the provider in place: the comment above the key and the
    /// one after its value survive, and a second save changes nothing.
    #[test]
    fn save_writes_the_translation_provider_in_place() {
        let (manager, path) = manager_at("save_provider");
        let original = "[provider]\n# my backend\ntranslate_provider = \"google\"  # for now\n\n\
                        [translation]\nsource_language = \"auto\"\ntarget_language = \"ru\"\n";
        fs::write(&path, original).unwrap();
        manager.load_config().unwrap();
        manager.set_provider(ProviderAxis::Translation, "deepl-work");
        manager.save_config().unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(
            written,
            original.replace(
                "translate_provider = \"google\"",
                "translate_provider = \"deepl-work\""
            )
        );

        manager.save_config().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), written);
        let _ = fs::remove_file(&path);
    }

    /// A file without a `[provider]` table gets one when the provider was switched away
    /// from the default, and none when it wasn't (a missing key already means the default).
    #[test]
    fn save_adds_a_missing_provider_table_only_when_needed() {
        let (manager, path) = manager_at("save_provider_missing");
        let original = "[translation]\nsource_language = \"auto\"\ntarget_language = \"ru\"\n";
        fs::write(&path, original).unwrap();
        manager.load_config().unwrap();

        manager.save_config().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        manager.set_provider(ProviderAxis::Translation, "deepl-work");
        manager.save_config().unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.starts_with(original), "{written}");
        assert!(
            written.contains("[provider]\ntranslate_provider = \"deepl-work\"\n"),
            "{written}"
        );
        manager.load_config().unwrap();
        assert_eq!(manager.get_config().translate_provider, "deepl-work");
        let _ = fs::remove_file(&path);
    }

    /// `/save` after `/p d` and `/p s` writes those keys in place, comments kept, and a
    /// second save changes nothing.
    #[test]
    fn save_writes_the_dictionary_and_speech_providers_in_place() {
        let (manager, path) = manager_at("save_axes");
        let original = "[dictionary]\n# lookups\ndictionary_provider = \"google\"  # default\n\n\
                        [speech]\n# voice\nspeech_provider = \"google\"  # default\n";
        fs::write(&path, original).unwrap();
        manager.load_config().unwrap();
        manager.set_provider(ProviderAxis::Dictionary, "ollama");
        manager.set_provider(ProviderAxis::Speech, "work");
        manager.save_config().unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let expected = original
            .replace(
                "dictionary_provider = \"google\"",
                "dictionary_provider = \"ollama\"",
            )
            .replace("speech_provider = \"google\"", "speech_provider = \"work\"");
        // The languages are written too; the provider lines are what this test is about.
        assert!(written.starts_with(&expected), "{written}");

        manager.save_config().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), written);
        let _ = fs::remove_file(&path);
    }

    /// A file without `dictionary_provider`/`speech_provider` gets each only when it was
    /// switched away from the default.
    #[test]
    fn save_adds_missing_dictionary_and_speech_keys_only_when_needed() {
        let (manager, path) = manager_at("save_axes_missing");
        let original = "[translation]\nsource_language = \"auto\"\ntarget_language = \"ru\"\n\n\
                        [dictionary]\nshow_dictionary = true\n";
        fs::write(&path, original).unwrap();
        manager.load_config().unwrap();

        manager.save_config().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        manager.set_provider(ProviderAxis::Dictionary, "ollama");
        manager.save_config().unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains(
                "[dictionary]\nshow_dictionary = true\ndictionary_provider = \"ollama\"\n"
            ),
            "{written}"
        );
        assert!(!written.contains("speech_provider"), "{written}");
        assert!(!written.contains("[speech]"), "{written}");

        manager.set_provider(ProviderAxis::Speech, "work");
        manager.save_config().unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("[speech]\nspeech_provider = \"work\"\n"),
            "{written}"
        );
        manager.load_config().unwrap();
        let config = manager.get_config();
        assert_eq!(config.dictionary_provider, "ollama");
        assert_eq!(config.speech_provider, "work");
        let _ = fs::remove_file(&path);
    }

    /// A file without a `[translation]` table gets a proper one (not an inline table).
    #[test]
    fn save_adds_a_missing_translation_table() {
        let (manager, path) = manager_at("save_missing_section");
        fs::write(&path, "[interface]\ncopy_to_clipboard = true\n").unwrap();
        manager.load_config().unwrap();
        manager.set_languages("en", "de");
        manager.save_config().unwrap();

        let written = fs::read_to_string(&path).unwrap();
        assert!(
            written.starts_with("[interface]\ncopy_to_clipboard = true\n"),
            "{written}"
        );
        assert!(
            written.contains("[translation]\nsource_language = \"en\"\ntarget_language = \"de\"\n"),
            "{written}"
        );
        let _ = fs::remove_file(&path);
    }

    /// With the file gone, `/save` writes a complete, commented one.
    #[test]
    fn save_without_a_file_writes_the_template() {
        let (manager, path) = manager_at("save_no_file");
        manager.set_languages("en", "de");
        manager.save_config().unwrap();
        let written = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(
            written.contains("# Text Translator Configuration File"),
            "{written}"
        );
        let config = parse_config(&written).unwrap();
        assert_eq!(config, manager.get_config());
    }

    /// A broken hand edit is reported once, and the previous settings stay in effect until
    /// the file is fixed.
    #[test]
    fn a_broken_edit_keeps_the_previous_config() {
        let (manager, path) = manager_at("broken_reload");
        let set_mtime = |secs: u64| {
            let file = fs::File::options().write(true).open(&path).unwrap();
            file.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs))
                .unwrap();
        };
        fs::write(&path, "[translation]\ntarget_language = \"German\"\n").unwrap();
        set_mtime(1_000_000);
        assert!(manager.check_and_reload().unwrap());
        assert_eq!(manager.get_config().target_language, "de");

        fs::write(&path, "[translation]\ntarget_language = German\n").unwrap();
        set_mtime(2_000_000);
        let error = manager.check_and_reload().unwrap_err().to_string();
        assert!(error.contains(&manager.config_path), "{error}");
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("keeping the previous settings"), "{error}");
        assert_eq!(manager.get_config().target_language, "de");
        // Reported once per edit, not on every call.
        assert!(!manager.check_and_reload().unwrap());

        fs::write(&path, "[translation]\ntarget_language = \"French\"\n").unwrap();
        set_mtime(3_000_000);
        assert!(manager.check_and_reload().unwrap());
        assert_eq!(manager.get_config().target_language, "fr");
        let _ = fs::remove_file(&path);
    }

    /// At startup, a broken file is an error naming the file (the caller exits with it).
    #[test]
    fn a_broken_file_fails_at_startup() {
        let (_, path) = manager_at("broken_startup");
        fs::write(&path, "[interface]\ncopy_to_clipboard = \"yes\"\n").unwrap();
        let error = ConfigManager::new(path.to_str().unwrap())
            .err()
            .unwrap()
            .to_string();
        let _ = fs::remove_file(&path);
        assert!(error.contains(path.to_str().unwrap()), "{error}");
        assert!(error.contains("line 2"), "{error}");
    }

    fn google_providers() -> ActiveProviders {
        ActiveProviders {
            translation: Ok("Google Translate".to_string()),
            dictionary: Ok("Google Dictionary".to_string()),
            speech: Ok("Google TTS".to_string()),
        }
    }

    #[test]
    fn banner_lists_every_provider_by_default() {
        assert_eq!(
            provider_banner_lines(&Config::default(), &google_providers()),
            [
                "Providers:",
                "  Translation: Google Translate",
                "  Dictionary: Google Dictionary",
                "  Speech: Google TTS",
            ]
        );
    }

    /// Like the speech hotkey line: a feature that is off doesn't list its provider.
    #[test]
    fn banner_leaves_out_disabled_dictionary_and_speech() {
        let config = Config {
            show_dictionary: false,
            enable_text_to_speech: false,
            ..Config::default()
        };
        assert_eq!(
            provider_banner_lines(&config, &google_providers()),
            ["Providers:", "  Translation: Google Translate"]
        );
    }

    #[test]
    fn banner_says_why_a_provider_is_unavailable() {
        let providers = ActiveProviders {
            speech: Err("unknown provider: nope".to_string()),
            ..google_providers()
        };
        let lines = provider_banner_lines(&Config::default(), &providers);
        assert_eq!(lines[3], "  Speech: unavailable (unknown provider: nope)");
    }

    /// The names the banner shows are the providers' own `name()`, so a profile carries
    /// its name, as `tagent` reports it. The profile name is unusual so no
    /// `TAGENT_<NAME>_<KEY>` in the developer's shell can interfere.
    #[test]
    fn banner_names_come_from_the_built_providers() {
        let config = parse_config(
            r#"
            [speech]
            speech_provider = "cli-test-banner"

            [provider_options.cli-test-banner]
            type = "google"
            "#,
        )
        .unwrap();
        let speech = config.create_speech_provider().unwrap();
        assert_eq!(speech.name(), "Google TTS (cli-test-banner)");
        let translate = config.create_translate_provider().unwrap();
        assert_eq!(translate.name(), "Google Translate");
    }

    #[test]
    fn new_settings_notice_counts_settings() {
        assert_eq!(new_settings_notice(0), None);
        assert_eq!(
            new_settings_notice(1).unwrap(),
            "Config: 1 new setting is available (run /config update or tagent-cli --update-config)"
        );
        assert!(new_settings_notice(3)
            .unwrap()
            .starts_with("Config: 3 new settings are available"));
    }

    #[test]
    fn update_config_file_backs_up_and_adds_what_is_missing() {
        let (manager, path) = manager_at("update");
        let original = "[interface]\ncopy_to_clipboard = true\nfuture_key = 1\n";
        write_config_file(path.to_str().unwrap(), original).unwrap();
        manager.load_config().unwrap();
        assert!(manager.new_settings_notice().is_some());

        let update = manager.update_config_file().unwrap();
        let backup = PathBuf::from(format!("{}.bak", path.display()));
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.starts_with("[provider]\n"), "{updated}");
        assert!(manager.get_config().copy_to_clipboard);
        assert_eq!(manager.new_settings_notice(), None);
        // The write was recorded, so the hot reload has nothing to re-read.
        assert!(!manager.check_and_reload().unwrap());
        let lines = update.lines();
        assert!(lines[0].starts_with("Updated "), "{lines:?}");
        assert!(
            lines.contains(&"  [speech] (section, 4 settings)".to_string()),
            "{lines:?}"
        );
        assert!(lines
            .iter()
            .any(|l| l.contains(&backup.display().to_string())));
        assert!(
            lines.contains(&"  line 3: unknown key `future_key` in [interface]".to_string()),
            "{lines:?}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&backup).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // A second run finds nothing to do and leaves the file (and the backup) alone.
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        fs::remove_file(&backup).unwrap();
        let update = update_config_file(&path).unwrap();
        assert_eq!(update.outcome, UpdateOutcome::UpToDate);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert!(!backup.exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn update_config_file_leaves_a_broken_file_alone() {
        let (_, path) = manager_at("update_broken");
        // Valid TOML, but a value of the wrong type: the application wouldn't start.
        let broken = "[interface]\nauto_hide_terminal_seconds = \"3\"\n";
        write_config_file(path.to_str().unwrap(), broken).unwrap();
        let error = update_config_file(&path).unwrap_err();
        assert!(error.contains("invalid configuration file"), "{error}");
        assert!(error.contains("line 2"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), broken);
        assert!(!PathBuf::from(format!("{}.bak", path.display())).exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn update_config_file_creates_a_missing_file() {
        let (_, path) = manager_at("update_missing");
        let update = update_config_file(&path).unwrap();
        assert_eq!(update.outcome, UpdateOutcome::Created);
        assert_eq!(
            parse_config(&fs::read_to_string(&path).unwrap()).unwrap(),
            Config::default()
        );
        let _ = fs::remove_file(&path);
    }

    fn provider_list_config() -> Config {
        let mut config = Config::default();
        config.provider_options.insert("work", "type", "deepl");
        config.provider_options.insert("work", "api_key", "key:fx");
        config
            .provider_options
            .insert("llm", "type", "no-such-kind");
        config.provider_options.insert("google", "max_retries", "0");
        config
    }

    /// The entries of `axis`, as `(name, display name)` pairs.
    fn axis_entries(entries: &[ProviderListEntry], axis: ProviderAxis) -> Vec<&ProviderListEntry> {
        entries.iter().filter(|e| e.axis == axis).collect()
    }

    #[test]
    fn provider_list_has_kinds_then_profiles_per_axis() {
        let config = provider_list_config();

        let entries = config.provider_list_using(|_| None);

        let names = |axis| -> Vec<&str> {
            axis_entries(&entries, axis)
                .iter()
                .map(|e| e.name.as_str())
                .collect()
        };
        assert_eq!(
            names(ProviderAxis::Translation),
            ["google", "deepl", "openai", "work"]
        );
        assert_eq!(names(ProviderAxis::Dictionary), ["google", "openai"]);
        assert_eq!(names(ProviderAxis::Speech), ["google"]);
        let translation = axis_entries(&entries, ProviderAxis::Translation);
        assert_eq!(translation[0].display_name, "Google Translate");
        assert_eq!(translation[2].display_name, "OpenAI-compatible");
        assert_eq!(translation[3].display_name, "DeepL (work)");
        let active: Vec<bool> = translation.iter().map(|e| e.active).collect();
        assert_eq!(active, [true, false, false, false]);
        // Axes come in display order, each in one block.
        let axes: Vec<ProviderAxis> = entries.iter().map(|e| e.axis).collect();
        let mut sorted = axes.clone();
        sorted.sort_by_key(|axis| ProviderAxis::ALL.iter().position(|a| a == axis));
        assert_eq!(axes, sorted);
    }

    /// A profile of kind `openai` serves translation and the dictionary, not speech.
    #[test]
    fn provider_list_puts_a_profile_under_every_axis_its_kind_serves() {
        let mut config = provider_list_config();
        config.provider_options.insert("ollama", "type", "openai");

        let entries = config.provider_list_using(|_| None);

        let axes: Vec<ProviderAxis> = entries
            .iter()
            .filter(|e| e.name == "ollama")
            .map(|e| e.axis)
            .collect();
        assert_eq!(axes, [ProviderAxis::Translation, ProviderAxis::Dictionary]);
        let dictionary = axis_entries(&entries, ProviderAxis::Dictionary);
        assert_eq!(dictionary[2].display_name, "OpenAI-compatible (ollama)");
    }

    #[test]
    fn provider_list_reports_missing_required_options_per_axis() {
        let mut config = provider_list_config();
        config.provider_options.insert("ollama", "type", "openai");
        config
            .provider_options
            .insert("ollama", "model", "qwen3:8b");

        let entries = config.provider_list_using(|_| None);

        let translation = axis_entries(&entries, ProviderAxis::Translation);
        assert_eq!(translation[0].missing, Vec::<String>::new());
        assert_eq!(translation[1].missing, ["api_key"]);
        assert_eq!(translation[2].missing, ["endpoint", "model"]);
        assert_eq!(translation[3].name, "ollama");
        assert_eq!(translation[3].missing, ["endpoint"]);
        assert_eq!(translation[4].missing, Vec::<String>::new());
        let dictionary = axis_entries(&entries, ProviderAxis::Dictionary);
        assert_eq!(dictionary[1].missing, ["endpoint", "model"]);
        assert_eq!(dictionary[2].missing, ["endpoint"]);
    }

    /// A required option supplied through `TAGENT_<NAME>_<KEY>` counts as set.
    #[test]
    fn provider_list_counts_env_overrides() {
        let config = provider_list_config();

        let entries = config.provider_list_using(|var| {
            (var == "TAGENT_DEEPL_API_KEY").then(|| "key:fx".to_string())
        });

        assert_eq!(entries[1].name, "deepl");
        assert_eq!(entries[1].missing, Vec::<String>::new());
    }

    #[test]
    fn provider_list_marks_the_active_profile_per_axis() {
        let mut config = provider_list_config();
        config.translate_provider = "Work".to_string();
        config.dictionary_provider = "OpenAI".to_string();

        let entries = config.provider_list_using(|_| None);

        let active: Vec<(ProviderAxis, &str)> = entries
            .iter()
            .filter(|e| e.active)
            .map(|e| (e.axis, e.name.as_str()))
            .collect();
        assert_eq!(
            active,
            [
                (ProviderAxis::Translation, "work"),
                (ProviderAxis::Dictionary, "openai"),
                (ProviderAxis::Speech, "google"),
            ]
        );
    }

    #[test]
    fn provider_list_lines_number_continuously_across_axes() {
        let config = provider_list_config();

        let lines = provider_list_lines(&config.provider_list_using(|_| None), &config);

        assert_eq!(
            lines,
            [
                "Providers:",
                " Translation",
                " * 1  google  Google Translate",
                "   2  deepl   DeepL (missing: api_key)",
                "   3  openai  OpenAI-compatible (missing: endpoint, model)",
                "   4  work    DeepL (work)",
                " Dictionary",
                " * 5  google  Google Dictionary",
                "   6  openai  OpenAI-compatible (missing: endpoint, model)",
                " Speech",
                " * 7  google  Google TTS",
                "Switch with /p <number>, /p <name> (translation) or /p t|d|s <name>; \
                 /save keeps the choice.",
            ]
        );
    }

    /// A turned-off axis is still listed (switching it is allowed), with a note.
    #[test]
    fn provider_list_lines_note_a_disabled_axis() {
        let mut config = provider_list_config();
        config.show_dictionary = false;
        config.enable_text_to_speech = false;

        let lines = provider_list_lines(&config.provider_list_using(|_| None), &config);

        assert!(lines.contains(&" Translation".to_string()), "{lines:?}");
        assert!(
            lines.contains(&" Dictionary (off: show_dictionary = false)".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&" Speech (off: enable_text_to_speech = false)".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&" * 7  google  Google TTS".to_string()),
            "{lines:?}"
        );
    }

    /// Two-digit numbers keep the names aligned.
    #[test]
    fn provider_list_lines_right_align_numbers() {
        let mut config = provider_list_config();
        for name in ["a1", "a2", "a3", "a4"] {
            config.provider_options.insert(name, "type", "google");
        }

        let lines = provider_list_lines(&config.provider_list_using(|_| None), &config);

        assert_eq!(lines[2], " *  1  google  Google Translate");
        assert!(
            lines
                .iter()
                .any(|l| l == " *  9  google  Google Dictionary"),
            "{lines:?}"
        );
    }
}
