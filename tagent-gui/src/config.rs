use crate::platform::keycodes;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tagent::languages;
use tagent::providers::{ProviderAxis, ProviderOptions, ProviderProfiles};

fn default_translate_provider() -> String {
    "google".to_string()
}

/// Default for [`GuiConfig::speech_provider`], matching `tagent-cli`'s own
/// `SpeechProvider` default.
fn default_speech_provider() -> String {
    "google".to_string()
}

/// Default for [`GuiConfig::dictionary_provider`], matching `tagent-cli`'s own
/// `DictionaryProvider` default.
fn default_dictionary_provider() -> String {
    "google".to_string()
}

/// Default for [`GuiConfig::source_language`]: auto-detect.
fn default_source_language() -> String {
    "auto".to_string()
}

/// Default for [`GuiConfig::target_language`]: the system's preferred language (see
/// [`target_language_for_locales`]).
fn default_target_language() -> String {
    target_language_for_locales(sys_locale::get_locales())
}

/// The first of `locales` (BCP 47 tags such as `"ru-UA"`, most preferred first) whose
/// language `tagent` knows, as its code; `"en"` when none is known.
pub fn target_language_for_locales(locales: impl IntoIterator<Item = String>) -> String {
    languages::language_for_locales(locales)
        .unwrap_or("en")
        .to_string()
}

/// `code` as it appears in `tagent`'s language table (lowercase), if it's listed there.
/// Only codes count: a language name or `"auto"` gives `None`.
fn known_language_code(code: &str) -> Option<String> {
    languages::language_code(code)
        .filter(|known| *known != "auto" && known.eq_ignore_ascii_case(code))
        .map(str::to_string)
}

fn default_theme() -> String {
    "auto".to_string()
}

/// Default font family for transcript style fields: `"monospace"`,
/// `"sans-serif"`, or `"serif"`.
fn default_style_font() -> String {
    "monospace".to_string()
}

/// Default font size (px) for transcript style fields.
fn default_style_size() -> i32 {
    13
}

/// Default color for transcript style fields: empty means "follow the theme".
fn default_style_color() -> String {
    String::new()
}

/// Default vertical gap (px) between transcript blocks.
fn default_block_spacing_px() -> i32 {
    20
}

/// Default vertical gap (px) between a phrase and its own translation.
fn default_phrases_spacing_px() -> i32 {
    2
}

/// Default: show the "[Auto]:"/"[Russian]:" prompt in front of the text.
fn default_show_prompt() -> bool {
    true
}

/// Default: the popup shows the "[Auto]:"/"[Russian]:"-style prompt, same
/// default as the transcript's own [`default_show_prompt`], but tracked as
/// its own independent setting.
fn default_popup_show_prompt() -> bool {
    true
}

/// Default: the global translate hotkey pops up the translation next to the cursor.
fn default_show_popup() -> bool {
    true
}

/// Default: the popup shows the original phrase line, not just the translation.
fn default_popup_show_phrase() -> bool {
    true
}

/// Default max width (px) of the popup.
fn default_popup_max_width() -> i32 {
    600
}

/// Default max height (px) of the popup before its content becomes scrollable.
fn default_popup_max_height() -> i32 {
    600
}

/// Default border width (px) of the popup.
fn default_popup_border_width() -> i32 {
    1
}

/// Default global hotkey, same format and default value as `tagent-cli`'s `translate_hotkey`.
fn default_translate_hotkey() -> String {
    "Alt+A".to_string()
}

/// Default speech hotkey (Stage 10 follow-up), same default value as `tagent-cli`'s
/// `SpeechHotkey` (which was `Alt+E` until `tagent-cli` `0.16.0+001`, when it was
/// aligned with this one).
fn default_speech_hotkey() -> String {
    "Alt+S".to_string()
}

/// Default for [`GuiConfig::enable_speech_hotkey`]: on.
fn default_enable_speech_hotkey() -> bool {
    true
}

/// Default delay (seconds) before the hotkey-triggered popup auto-hides, same
/// default as `tagent-cli`'s `AutoHideTerminalSeconds`.
fn default_popup_auto_hide_seconds() -> u64 {
    3
}

/// Default for [`GuiConfig::start_minimized`] (Stage 7): the app launches
/// straight into the tray with no window shown.
fn default_start_minimized() -> bool {
    true
}

/// Default for [`GuiConfig::remember_window_geometry`]: on.
fn default_remember_window_geometry() -> bool {
    true
}

/// Default for [`GuiConfig::remember_popup_position`]: off — the popup appears
/// next to the mouse cursor, as it always has.
fn default_remember_popup_position() -> bool {
    false
}

/// Default for [`GuiConfig::show_dictionary`], matching `tagent-cli`'s `ShowDictionary`.
fn default_show_dictionary() -> bool {
    true
}

/// Default for [`GuiConfig::spell_check`], matching `tagent-cli`'s `SpellCheck`.
fn default_spell_check() -> bool {
    true
}

/// Default for [`GuiConfig::enable_text_to_speech`], matching `tagent-cli`'s
/// `EnableTextToSpeech`.
fn default_enable_text_to_speech() -> bool {
    true
}

/// Default for [`GuiConfig::show_context_menu`]: off -- a single-item "Copy" menu is
/// pure friction over just copying the block directly on right-click.
fn default_show_context_menu() -> bool {
    false
}

/// `tagent-gui`'s own configuration, independent of `tagent-cli.toml`.
///
/// Stored as plain, pretty-printed JSON at [`config_path`] and meant to be
/// hand-editable (not only written via a future Settings window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuiConfig {
    #[serde(default = "default_translate_provider")]
    pub translate_provider: String,
    /// The source language the main window starts with: `"auto"` (detect) or a code from
    /// `tagent`'s language table, e.g. `"en"`. Settings > General; the main window's own
    /// pick holds for the run only. An unknown code falls back to `"auto"` on load.
    #[serde(default = "default_source_language")]
    pub source_language: String,
    /// The target language the main window starts with, a code from `tagent`'s language
    /// table (never `"auto"`). Defaults to the system's preferred language, else `"en"`;
    /// an unknown code falls back to that on load.
    #[serde(default = "default_target_language")]
    pub target_language: String,
    /// One of `"auto"`, `"light"`, `"dark"`. `"auto"` follows the system setting.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Shared background color for both main panels (transcript log and
    /// input box), as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub background_color: String,
    /// Font family for the original-phrase lines in the transcript.
    #[serde(default = "default_style_font")]
    pub phrase_font: String,
    /// Font size (px) for the original-phrase lines in the transcript.
    #[serde(default = "default_style_size")]
    pub phrase_size: i32,
    /// Text color for the original-phrase lines, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub phrase_color: String,
    /// Background color for the original-phrase lines, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub phrase_background: String,
    /// Font family for the translation lines in the transcript.
    #[serde(default = "default_style_font")]
    pub translation_font: String,
    /// Font size (px) for the translation lines in the transcript.
    #[serde(default = "default_style_size")]
    pub translation_size: i32,
    /// Text color for the translation lines, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub translation_color: String,
    /// Background color for the translation lines, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub translation_background: String,
    /// Font size (px) for the rest of the main window's text: the transcript
    /// header, the input box's `[auto → ru]:` label and the text typed in the
    /// input box. The transcript's own prompts follow
    /// [`Self::phrase_size`]/[`Self::translation_size`] instead, being part of
    /// those lines.
    #[serde(default = "default_style_size")]
    pub input_size: i32,
    /// Text color for the `[Language]:` prompt prefix shown before the phrase and
    /// translation text in the transcript and the input box (and, since Stage 13,
    /// the transcript's own highlighted prefix -- see `styled::Role::Prompt`), as
    /// `"#RRGGBB"`, or `""` to follow the theme. Shared by the phrase and
    /// translation sides -- there is only one prompt accent, not a separate one
    /// per side.
    #[serde(default = "default_style_color")]
    pub prompt_color: String,
    /// Font family for the hotkey-triggered popup's text (phrase and
    /// translation share one style, unlike the transcript's separate
    /// phrase/translation styling).
    #[serde(default = "default_style_font")]
    pub popup_font: String,
    /// Font size (px) for the popup's text.
    #[serde(default = "default_style_size")]
    pub popup_size: i32,
    /// Text color for the popup, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub popup_color: String,
    /// Background color for the popup, as `"#RRGGBB"`, or `""` to follow the theme.
    #[serde(default = "default_style_color")]
    pub popup_background: String,
    /// Text color for the popup's own `[Language]:` prompt prefix highlighting
    /// (Stage 13), as `"#RRGGBB"`, or `""` to follow [`Self::prompt_color`] (which
    /// itself follows the theme when that's also empty) -- mirrors how
    /// [`Self::popup_color`] follows [`Self::translation_color`].
    #[serde(default = "default_style_color")]
    pub popup_prompt_color: String,
    /// Whether the global translate hotkey shows the popup at all. When `false`,
    /// the hotkey still translates the selection into the transcript, just with
    /// no popup. Live-reloaded, no restart needed.
    #[serde(default = "default_show_popup")]
    pub show_popup: bool,
    /// Whether the popup shows the "[Auto]:"/"[Russian]:"-style prompt before
    /// its phrase/translation text. Independent of the transcript's own
    /// [`Self::show_prompt`].
    #[serde(default = "default_popup_show_prompt")]
    pub popup_show_prompt: bool,
    /// Whether the popup shows the original phrase line at all, or only the
    /// translation.
    #[serde(default = "default_popup_show_phrase")]
    pub popup_show_phrase: bool,
    /// Max width (px) of the popup — content wraps to fit within it.
    #[serde(default = "default_popup_max_width")]
    pub popup_max_width: i32,
    /// Max height (px) of the popup before its content becomes scrollable.
    #[serde(default = "default_popup_max_height")]
    pub popup_max_height: i32,
    /// Border width (px) of the popup's outer panel; color stays theme-derived
    /// regardless.
    #[serde(default = "default_popup_border_width")]
    pub popup_border_width: i32,
    /// Vertical gap (px) between one phrase/translation pair and the next.
    #[serde(default = "default_block_spacing_px")]
    pub block_spacing_px: i32,
    /// Vertical gap (px) between a phrase and its own translation, within one pair.
    #[serde(default = "default_phrases_spacing_px")]
    pub phrases_spacing_px: i32,
    /// Whether to show the "[Auto]:"/"[Russian]:"-style prompt before the
    /// phrase and translation text.
    #[serde(default = "default_show_prompt")]
    pub show_prompt: bool,
    /// Global hotkey that copies the current selection and translates it
    /// (see [`HotkeyParser`] for the supported string formats). Takes effect
    /// only on restart. Linux and Windows only — no effect on macOS yet.
    #[serde(default = "default_translate_hotkey")]
    pub translate_hotkey: String,
    /// Global hotkey that speaks the current selection directly, with no
    /// translation step (Stage 10 follow-up; see [`HotkeyParser`] for the
    /// supported string formats). Takes effect only on restart. Linux and
    /// Windows only — no effect on macOS yet.
    #[serde(default = "default_speech_hotkey")]
    pub speech_hotkey: String,
    /// Enable the speech hotkey. When `false`, it isn't registered at all —
    /// unlike [`Self::enable_text_to_speech`], which only gates whether *any*
    /// speech plays, this gates hotkey *registration* specifically. Takes
    /// effect only on restart.
    #[serde(default = "default_enable_speech_hotkey")]
    pub enable_speech_hotkey: bool,
    /// Delay (seconds) before the hotkey-triggered popup (Stage 6) auto-hides,
    /// once the cursor is no longer over it. Live-reloaded, no restart needed.
    /// `0` is treated the same as the default (`3`) rather than "never
    /// auto-hide" — see [`Self::popup_auto_hide_seconds_or_default`] — since the
    /// popup is a no-frame window with no close button, so `0` would otherwise
    /// leave it stuck on screen for the rest of the process's life.
    #[serde(default = "default_popup_auto_hide_seconds")]
    pub popup_auto_hide_seconds: u64,
    /// Whether the app launches with the main window hidden (living only in the
    /// system tray, Stage 7) or shown, on the *next* launch — read once at
    /// startup, not live-reloaded. Default `true`. The main window is always
    /// reachable regardless of this setting: the tray icon's "Show Tagent" entry
    /// (or a left click on it) reveals it, and the global hotkey
    /// ([`Self::translate_hotkey`]) still works even while both the window and
    /// the tray icon are invisible (e.g. on a desktop with no StatusNotifierItem
    /// host) — that's the practical way back in if the tray never appears.
    #[serde(default = "default_start_minimized")]
    pub start_minimized: bool,
    /// Whether the main window's size and position are saved when it's hidden
    /// (to the tray) or the app quits, and restored the next time it's shown.
    /// Default `true`. Applied once, the first time the window is actually
    /// shown in a given run (at startup if not [`Self::start_minimized`],
    /// otherwise the first time it's revealed from the tray) — not
    /// live-reloaded, since re-applying it on every later show would fight
    /// with the user moving/resizing the already-visible window.
    #[serde(default = "default_remember_window_geometry")]
    pub remember_window_geometry: bool,
    /// The main window's last known position/size, in physical pixels — `None`
    /// until it's been shown and hidden (or the app quit) at least once.
    /// Ignored entirely when [`Self::remember_window_geometry`] is `false`, but
    /// still kept on disk either way, so toggling the setting back on later
    /// restores the last position from before it was turned off rather than
    /// starting over.
    #[serde(default)]
    pub window_geometry: Option<WindowGeometry>,
    /// Whether the hotkey popup reappears where the user last dragged it,
    /// instead of next to the mouse cursor. Default `false`. Dragging the popup
    /// works either way; this only controls whether the dropped position is saved
    /// ([`Self::popup_position`]) and used for later popups. Live-reloaded, no
    /// restart needed.
    #[serde(default = "default_remember_popup_position")]
    pub remember_popup_position: bool,
    /// Where the popup's top-left corner was last dropped after a drag, in
    /// physical pixels — `None` until it's been dragged at least once with
    /// [`Self::remember_popup_position`] on. Ignored (the popup follows the cursor)
    /// while that setting is `false`, but kept on disk either way, same as
    /// [`Self::window_geometry`].
    #[serde(default)]
    pub popup_position: Option<PopupPosition>,
    /// Whether single-word input triggers a dictionary lookup (definitions
    /// grouped by part of speech) instead of a plain translation. Live-reloaded,
    /// no restart needed.
    #[serde(default = "default_show_dictionary")]
    pub show_dictionary: bool,
    /// Whether a spelling-correction notice is shown when the provider silently
    /// corrected a misspelled word during dictionary lookup. Has no effect while
    /// [`Self::show_dictionary`] is `false`. Live-reloaded, no restart needed.
    #[serde(default = "default_spell_check")]
    pub spell_check: bool,
    /// Whether per-entry text-to-speech speaker buttons are shown in the
    /// transcript (Stage 10). Live-reloaded, no restart needed.
    #[serde(default = "default_enable_text_to_speech")]
    pub enable_text_to_speech: bool,
    /// Whether right-click on a transcript block opens a "Copy" context menu, or
    /// copies the block immediately with no menu at all. Default `false` (a
    /// single-item menu is pure friction over copying directly) -- when off, a
    /// brief border flash on the copied block is the only feedback, since there's
    /// no menu-click to see. Live-reloaded, no restart needed.
    #[serde(default = "default_show_context_menu")]
    pub show_context_menu: bool,
    /// Name of the text-to-speech backend (Stage 11), independent of
    /// [`Self::translate_provider`]. Chosen from a dropdown on Settings > "Providers"
    /// (or hand-edited); live-reloaded, no restart needed.
    #[serde(default = "default_speech_provider")]
    pub speech_provider: String,
    /// Name of the dictionary backend (Stage 12), independent of
    /// [`Self::translate_provider`]. Chosen from a dropdown on Settings > "Providers"
    /// (or hand-edited); live-reloaded, no restart needed. A bad name disables
    /// dictionary lookups (single words fall back to a plain translation) rather than
    /// breaking translation.
    #[serde(default = "default_dictionary_provider")]
    pub dictionary_provider: String,
    /// Provider profiles: profile name → option key → value, e.g.
    /// `{"deepl": {"api_key": "..."}}`. `translate_provider`, `dictionary_provider` and
    /// `speech_provider` name a profile (a built-in name like `"google"` works without an
    /// entry); an optional `"type"` entry picks the provider kind, defaulting to the
    /// profile name. Names and keys are case-insensitive. Edited on Settings > "Providers"
    /// or by hand; `TAGENT_<NAME>_<KEY>` environment variables override values. Modeled here so
    /// [`GuiConfigManager::update`], which rewrites the whole file, keeps it; its `Debug`
    /// masks secret values, so options never reach `tagent-gui.log` in full.
    #[serde(default)]
    pub provider_options: ProviderProfiles,
    /// Providers and profiles left out of the provider pickers (Settings > "Providers"
    /// "Show in lists" unchecked), lowercase. Only hides them from the lists: a hidden
    /// entry still works wherever it is selected. Live-reloaded.
    #[serde(default, deserialize_with = "lowercase_names")]
    pub hidden_providers: Vec<String>,
}

/// Deserializes a list of names, trimmed and lowercased, empty ones and duplicates dropped.
fn lowercase_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let names = Vec::<String>::deserialize(deserializer)?;
    let mut normalized: Vec<String> = Vec::new();
    for name in names.iter().map(|name| name.trim().to_lowercase()) {
        if !name.is_empty() && !normalized.contains(&name) {
            normalized.push(name);
        }
    }
    Ok(normalized)
}

/// A provider profile selected for one axis: its name and effective options (config
/// entry plus environment overrides), ready for `tagent`'s `*_with` factories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderChoice {
    /// The profile name, as configured.
    pub name: String,
    /// The profile's options.
    pub options: ProviderOptions,
}

/// The main window's saved position/size ([`GuiConfig::window_geometry`]), in
/// physical pixels — the same units [`slint::PhysicalPosition`]/
/// [`slint::PhysicalSize`] use, so no conversion is needed at the call sites
/// that read or write this.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Horizontal position of the window's top-left corner.
    pub x: i32,
    /// Vertical position of the window's top-left corner.
    pub y: i32,
    /// Window width.
    pub width: u32,
    /// Window height.
    pub height: u32,
}

/// A popup position, in physical pixels — the same coordinate space
/// [`slint::PhysicalPosition`] uses, so no conversion is needed at the call sites
/// that read or write this.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PopupPosition {
    /// Horizontal position of the popup's top-left corner.
    pub x: i32,
    /// Vertical position of the popup's top-left corner.
    pub y: i32,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            translate_provider: default_translate_provider(),
            source_language: default_source_language(),
            target_language: default_target_language(),
            theme: default_theme(),
            background_color: default_style_color(),
            phrase_font: default_style_font(),
            phrase_size: default_style_size(),
            phrase_color: default_style_color(),
            phrase_background: default_style_color(),
            translation_font: default_style_font(),
            translation_size: default_style_size(),
            translation_color: default_style_color(),
            translation_background: default_style_color(),
            input_size: default_style_size(),
            prompt_color: default_style_color(),
            popup_font: default_style_font(),
            popup_size: default_style_size(),
            popup_color: default_style_color(),
            popup_background: default_style_color(),
            popup_prompt_color: default_style_color(),
            popup_show_prompt: default_popup_show_prompt(),
            show_popup: default_show_popup(),
            popup_show_phrase: default_popup_show_phrase(),
            popup_max_width: default_popup_max_width(),
            popup_max_height: default_popup_max_height(),
            popup_border_width: default_popup_border_width(),
            block_spacing_px: default_block_spacing_px(),
            phrases_spacing_px: default_phrases_spacing_px(),
            show_prompt: default_show_prompt(),
            translate_hotkey: default_translate_hotkey(),
            speech_hotkey: default_speech_hotkey(),
            enable_speech_hotkey: default_enable_speech_hotkey(),
            popup_auto_hide_seconds: default_popup_auto_hide_seconds(),
            start_minimized: default_start_minimized(),
            remember_window_geometry: default_remember_window_geometry(),
            window_geometry: None,
            remember_popup_position: default_remember_popup_position(),
            popup_position: None,
            show_dictionary: default_show_dictionary(),
            spell_check: default_spell_check(),
            enable_text_to_speech: default_enable_text_to_speech(),
            show_context_menu: default_show_context_menu(),
            speech_provider: default_speech_provider(),
            dictionary_provider: default_dictionary_provider(),
            provider_options: ProviderProfiles::new(),
            hidden_providers: Vec::new(),
        }
    }
}

impl GuiConfig {
    /// The configured provider of `axis`: `translate_provider`, `dictionary_provider` or
    /// `speech_provider`.
    pub fn provider_name(&self, axis: ProviderAxis) -> &str {
        match axis {
            ProviderAxis::Translation => &self.translate_provider,
            ProviderAxis::Dictionary => &self.dictionary_provider,
            ProviderAxis::Speech => &self.speech_provider,
        }
    }

    /// Whether `axis` is turned on: always for translation, `show_dictionary` and
    /// `enable_text_to_speech` for the other two.
    pub fn axis_enabled(&self, axis: ProviderAxis) -> bool {
        match axis {
            ProviderAxis::Translation => true,
            ProviderAxis::Dictionary => self.show_dictionary,
            ProviderAxis::Speech => self.enable_text_to_speech,
        }
    }

    /// Profile `name` with its effective options: its `provider_options` entry (matched
    /// case-insensitively), with `TAGENT_<NAME>_<KEY>` environment variables taking
    /// precedence.
    pub fn provider_choice(&self, name: &str) -> ProviderChoice {
        self.provider_choice_using(name, |var| std::env::var(var).ok())
    }

    /// [`Self::provider_choice`] with the environment replaced by `lookup`.
    fn provider_choice_using(
        &self,
        name: &str,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> ProviderChoice {
        ProviderChoice {
            name: name.to_string(),
            options: self.provider_options.options_using(name, lookup),
        }
    }

    /// The configured profiles whose provider kind (their `"type"`, or else their name)
    /// is one of `kinds`, i.e. what a picker for that axis offers besides the built-in
    /// names; sorted, lowercase.
    pub fn profiles_of_kinds(&self, kinds: &[&str]) -> Vec<String> {
        self.provider_options.profiles_of_kinds(kinds)
    }

    /// Replaces a [`Self::source_language`]/[`Self::target_language`] that isn't a known
    /// code (or is `"auto"` as the target) with its default, lowercases the rest, and
    /// returns a warning per replaced value. Run on every load, so the rest of the app
    /// only ever sees valid codes; the file itself is fixed by the next save.
    pub fn normalize_languages(&mut self) -> Vec<String> {
        self.normalize_languages_with(default_target_language)
    }

    /// [`Self::normalize_languages`] with the target default produced by `target_default`.
    fn normalize_languages_with(&mut self, target_default: impl FnOnce() -> String) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.source_language.eq_ignore_ascii_case("auto") {
            self.source_language = default_source_language();
        } else if let Some(code) = known_language_code(&self.source_language) {
            self.source_language = code;
        } else {
            warnings.push(format!(
                "unknown source_language {:?}, using \"auto\"",
                self.source_language
            ));
            self.source_language = default_source_language();
        }
        if let Some(code) = known_language_code(&self.target_language) {
            self.target_language = code;
        } else {
            let fallback = target_default();
            warnings.push(format!(
                "unknown target_language {:?}, using {fallback:?}",
                self.target_language
            ));
            self.target_language = fallback;
        }
        warnings
    }

    /// [`Self::popup_auto_hide_seconds`], with `0` clamped to the default (`3`).
    pub fn popup_auto_hide_seconds_or_default(&self) -> u64 {
        if self.popup_auto_hide_seconds == 0 {
            default_popup_auto_hide_seconds()
        } else {
            self.popup_auto_hide_seconds
        }
    }
}

/// Returns the platform-default path for `tagent-gui.json` (not created here).
///
/// - **Windows**: `%APPDATA%\tagent-gui\tagent-gui.json`
/// - **Linux**: `~/.config/tagent-gui/tagent-gui.json`
/// - **macOS**: `~/Library/Application Support/tagent-gui/tagent-gui.json`
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tagent-gui")
        .join("tagent-gui.json")
}

/// Loads config from `path`, tolerating a missing or corrupt file.
///
/// - Missing file: writes a default config to `path` (best-effort — a failed write
///   to a read-only directory doesn't block startup) and returns the default.
/// - Present but unparseable: logs a warning to stderr and returns the default
///   **without** touching the file, since it's meant to be hand-edited and a typo
///   shouldn't get silently clobbered.
/// - Present and valid: returns the parsed config.
pub fn load_from_path(path: &Path) -> GuiConfig {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(mut config) => {
                normalize_and_warn(path, &mut config);
                config
            }
            Err(err) => {
                eprintln!(
                    "Warning: failed to parse {}: {err} — using defaults for this run",
                    path.display()
                );
                GuiConfig::default()
            }
        },
        Err(_) => {
            let config = GuiConfig::default();
            if let Err(err) = save_to_path(path, &config) {
                eprintln!(
                    "Warning: failed to write default config to {}: {err}",
                    path.display()
                );
            }
            config
        }
    }
}

/// [`GuiConfig::normalize_languages`], logging each fix to stderr.
fn normalize_and_warn(path: &Path, config: &mut GuiConfig) {
    for warning in config.normalize_languages() {
        eprintln!("Warning: {}: {warning}", path.display());
    }
}

/// Writes `config` to `path` as pretty-printed JSON, creating parent directories.
///
/// On Unix the file is created, or tightened, to mode `0600` before anything is written,
/// since `provider_options` can hold API keys; on Windows `%APPDATA%` is already per-user.
pub fn save_to_path(path: &Path, config: &GuiConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut open = fs::OpenOptions::new();
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
    serde_json::to_writer_pretty(&mut file, config)?;
    file.flush()
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Holds a loaded [`GuiConfig`] and reloads it from disk when the file's mtime
/// advances, so hand-edits to `tagent-gui.json` are picked up without a restart.
///
/// Mirrors `tagent-cli`'s `ConfigManager`/`check_and_reload()`
/// (`tagent-cli/src/config.rs`): on a reload attempt that fails to parse, the
/// last-known-good in-memory config is kept rather than falling back to
/// [`GuiConfig::default`] — that default-on-corruption behavior only applies to
/// the very first load, not to a reload of a config that was previously valid.
pub struct GuiConfigManager {
    path: PathBuf,
    config: GuiConfig,
    last_modified: Option<SystemTime>,
}

impl GuiConfigManager {
    /// Loads from the platform-default path, creating a default file if none exists.
    pub fn new() -> Self {
        let path = config_path();
        let config = load_from_path(&path);
        let last_modified = mtime(&path);
        Self {
            path,
            config,
            last_modified,
        }
    }

    /// Test-only constructor pointed at an arbitrary path instead of the
    /// platform-default one.
    #[cfg(test)]
    pub fn new_for_test(path: PathBuf) -> Self {
        let config = load_from_path(&path);
        let last_modified = mtime(&path);
        Self {
            path,
            config,
            last_modified,
        }
    }

    /// The currently loaded config.
    pub fn config(&self) -> &GuiConfig {
        &self.config
    }

    /// Applies `config` in memory immediately and persists it to disk.
    ///
    /// The in-memory config is updated regardless of whether the write succeeds —
    /// a user-initiated change (e.g. from a Settings dialog) shouldn't be silently
    /// dropped just because the disk write failed. On a successful write,
    /// `last_modified` is refreshed to the file's new mtime so the next
    /// [`Self::check_and_reload`] doesn't immediately re-read what was just
    /// written here.
    pub fn update(&mut self, config: GuiConfig) -> io::Result<()> {
        self.config = config;
        let result = save_to_path(&self.path, &self.config);
        if result.is_ok() {
            self.last_modified = mtime(&self.path);
        }
        result
    }

    /// Reloads from disk if the file's mtime has advanced since the last load.
    ///
    /// Returns `true` if the in-memory config changed. On a parse failure the
    /// in-memory config is left untouched, a warning is logged to stderr, and the
    /// file's mtime is still recorded — so a standing bad edit is reported once,
    /// not on every call, until it's fixed (or changed again).
    pub fn check_and_reload(&mut self) -> bool {
        let current = mtime(&self.path);
        let should_reload = match (current, self.last_modified) {
            (Some(current), Some(last)) => current > last,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if !should_reload {
            return false;
        }
        self.last_modified = current;

        match fs::read_to_string(&self.path) {
            Ok(content) => match serde_json::from_str::<GuiConfig>(&content) {
                Ok(mut config) => {
                    normalize_and_warn(&self.path, &mut config);
                    self.config = config;
                    true
                }
                Err(err) => {
                    eprintln!(
                        "Warning: failed to reload {}: {err} — keeping previous config",
                        self.path.display()
                    );
                    false
                }
            },
            Err(err) => {
                eprintln!(
                    "Warning: failed to read {} for reload: {err}",
                    self.path.display()
                );
                false
            }
        }
    }
}

impl Default for GuiConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

// Hotkey configuration types and parser, ported from `tagent-cli`'s
// `config.rs` (same string format, same defaults) — see Stage 5 of the
// development plan.
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
mod hotkey_tests {
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

    #[test]
    fn parse_single_key() {
        let result = HotkeyParser::parse("F9").unwrap();
        assert!(matches!(result, HotkeyType::SingleKey { vk_code: _ }));

        let result = HotkeyParser::parse("f9").unwrap();
        assert!(matches!(result, HotkeyType::SingleKey { vk_code: _ }));
    }

    #[test]
    fn validate_single_key_only_allows_f1_to_f12() {
        let hotkey = HotkeyParser::parse("F9").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("F1").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("F12").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Space").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Tab").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());
    }

    #[test]
    fn parse_modifier_combo() {
        let result = HotkeyParser::parse("Alt+Space").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));

        let result = HotkeyParser::parse("Ctrl+Shift+C").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));

        let result = HotkeyParser::parse("Win+T").unwrap();
        assert!(matches!(result, HotkeyType::ModifierCombo { .. }));
    }

    #[test]
    fn validate_rejects_shift_only_combo() {
        let hotkey = HotkeyParser::parse("Shift+T").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Shift+Space").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_err());

        let hotkey = HotkeyParser::parse("Ctrl+Shift+T").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Alt+Shift+Q").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());
    }

    #[test]
    fn parse_double_press() {
        let result = HotkeyParser::parse("Ctrl+Ctrl").unwrap();
        assert!(matches!(result, HotkeyType::DoublePress { .. }));

        let result = HotkeyParser::parse("F8+F8").unwrap();
        assert!(matches!(result, HotkeyType::DoublePress { .. }));
    }

    #[test]
    fn parse_left_right_modifier_normalization() {
        let result = HotkeyParser::parse("LAlt+Q").unwrap();
        match result {
            HotkeyType::ModifierCombo { modifiers, .. } => {
                assert_eq!(modifiers, vec![keycodes::KEY_ALT]);
            }
            _ => panic!("expected ModifierCombo"),
        }

        let result = HotkeyParser::parse("RCtrl+Shift+T").unwrap();
        match result {
            HotkeyType::ModifierCombo { modifiers, .. } => {
                assert!(modifiers.contains(&keycodes::KEY_CONTROL));
            }
            _ => panic!("expected ModifierCombo"),
        }
    }

    #[test]
    fn parse_double_press_normalizes_vk_code() {
        let result = HotkeyParser::parse("LCtrl+LCtrl").unwrap();
        match result {
            HotkeyType::DoublePress { vk_code, .. } => {
                assert_eq!(vk_code, keycodes::KEY_CONTROL);
            }
            _ => panic!("expected DoublePress"),
        }
    }

    #[test]
    fn parse_invalid_input_errors() {
        assert!(HotkeyParser::parse("InvalidKey").is_err());
        assert!(HotkeyParser::parse("").is_err());
    }

    #[test]
    fn validate_blocks_dangerous_combos() {
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
    fn validate_double_press_only_allows_f1_to_f12_or_modifiers() {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    fn temp_config_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("tagent-gui.json")
    }

    /// Filesystem mtime resolution isn't guaranteed sub-millisecond on every
    /// platform/filesystem; sleeping a bit between writes keeps mtime comparisons
    /// in tests reliable.
    fn wait_for_mtime_tick() {
        sleep(Duration::from_millis(20));
    }

    fn locales(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|tag| tag.to_string()).collect()
    }

    #[test]
    fn target_language_is_the_first_known_system_language() {
        // The locale matching itself is tested in `tagent::languages`.
        assert_eq!(target_language_for_locales(locales(&["eo", "de-AT"])), "de");
        assert_eq!(target_language_for_locales(locales(&["eo"])), "en");
        assert_eq!(target_language_for_locales(locales(&[])), "en");
    }

    #[test]
    fn normalize_languages_keeps_known_codes_lowercased() {
        let mut config = GuiConfig {
            source_language: "EN".to_string(),
            target_language: "Ru".to_string(),
            ..GuiConfig::default()
        };
        assert!(config
            .normalize_languages_with(|| unreachable!())
            .is_empty());
        assert_eq!(config.source_language, "en");
        assert_eq!(config.target_language, "ru");

        config.source_language = "Auto".to_string();
        assert!(config
            .normalize_languages_with(|| unreachable!())
            .is_empty());
        assert_eq!(config.source_language, "auto");
    }

    #[test]
    fn normalize_languages_replaces_unknown_codes_and_auto_target() {
        let mut config = GuiConfig {
            source_language: "Russian".to_string(),
            target_language: "auto".to_string(),
            ..GuiConfig::default()
        };
        let warnings = config.normalize_languages_with(|| "de".to_string());
        assert_eq!(config.source_language, "auto");
        assert_eq!(config.target_language, "de");
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("source_language") && warnings[0].contains("Russian"));
        assert!(warnings[1].contains("target_language") && warnings[1].contains("\"de\""));
    }

    #[test]
    fn languages_load_from_the_file_and_are_normalized() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"source_language": "EN", "target_language": "de"}"#,
        )
        .unwrap();
        let config = load_from_path(&path);
        assert_eq!(config.source_language, "en");
        assert_eq!(config.target_language, "de");

        // Missing keys get the defaults.
        fs::write(&path, br#"{"translate_provider": "google"}"#).unwrap();
        let config = load_from_path(&path);
        assert_eq!(config.source_language, "auto");
        assert_eq!(config.target_language, default_target_language());

        // A bad value is replaced in memory; the file keeps it until the next save.
        fs::write(&path, br#"{"source_language": "xx"}"#).unwrap();
        assert_eq!(load_from_path(&path).source_language, "auto");
        assert!(fs::read_to_string(&path).unwrap().contains("\"xx\""));
    }

    #[test]
    fn missing_file_returns_default_and_writes_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);

        let config = load_from_path(&path);

        assert_eq!(config, GuiConfig::default());
        assert!(path.exists());
        let on_disk: GuiConfig = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk, GuiConfig::default());
    }

    #[test]
    fn valid_file_is_returned_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            translate_provider: "deepl".to_string(),
            theme: default_theme(),
            ..Default::default()
        };
        save_to_path(&path, &config).unwrap();

        assert_eq!(load_from_path(&path), config);
    }

    #[test]
    fn old_file_without_theme_field_defaults_to_auto() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, br#"{"translate_provider": "google"}"#).unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.translate_provider, "google");
        assert_eq!(config.theme, "auto");
    }

    #[test]
    fn old_file_without_hotkey_field_defaults_to_alt_a() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.translate_hotkey, "Alt+A");
    }

    #[test]
    fn old_file_without_speech_hotkey_fields_defaults_to_alt_s_and_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.speech_hotkey, "Alt+S");
        assert!(config.enable_speech_hotkey);
    }

    #[test]
    fn old_file_without_popup_auto_hide_field_defaults_to_three() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.popup_auto_hide_seconds, 3);
    }

    #[test]
    fn old_file_without_start_minimized_field_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(config.start_minimized);
    }

    #[test]
    fn old_file_without_window_geometry_fields_defaults_to_remembering_with_none_saved() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(config.remember_window_geometry);
        assert_eq!(config.window_geometry, None);
    }

    #[test]
    fn window_geometry_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            window_geometry: Some(WindowGeometry {
                x: 100,
                y: 50,
                width: 480,
                height: 480,
            }),
            ..Default::default()
        };
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path);

        assert_eq!(
            loaded.window_geometry,
            Some(WindowGeometry {
                x: 100,
                y: 50,
                width: 480,
                height: 480
            })
        );
    }

    #[test]
    fn old_file_without_popup_position_fields_defaults_to_following_the_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(!config.remember_popup_position);
        assert_eq!(config.popup_position, None);
    }

    #[test]
    fn popup_position_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            remember_popup_position: true,
            popup_position: Some(PopupPosition { x: -40, y: 300 }),
            ..Default::default()
        };
        save_to_path(&path, &config).unwrap();

        let loaded = load_from_path(&path);

        assert!(loaded.remember_popup_position);
        assert_eq!(
            loaded.popup_position,
            Some(PopupPosition { x: -40, y: 300 })
        );
    }

    #[test]
    fn popup_auto_hide_seconds_or_default_clamps_zero() {
        let config = GuiConfig {
            popup_auto_hide_seconds: 0,
            ..Default::default()
        };
        assert_eq!(config.popup_auto_hide_seconds_or_default(), 3);

        let config = GuiConfig {
            popup_auto_hide_seconds: 10,
            ..Default::default()
        };
        assert_eq!(config.popup_auto_hide_seconds_or_default(), 10);
    }

    #[test]
    fn old_file_without_style_fields_defaults_to_monospace_and_theme_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.background_color, "");
        assert_eq!(config.phrase_font, "monospace");
        assert_eq!(config.phrase_size, 13);
        assert_eq!(config.phrase_color, "");
        assert_eq!(config.phrase_background, "");
        assert_eq!(config.translation_font, "monospace");
        assert_eq!(config.translation_size, 13);
        assert_eq!(config.translation_color, "");
        assert_eq!(config.translation_background, "");
        assert_eq!(config.input_size, 13);
        assert_eq!(config.prompt_color, "");
        assert_eq!(config.block_spacing_px, 20);
        assert_eq!(config.phrases_spacing_px, 2);
        assert!(config.show_prompt);
    }

    #[test]
    fn old_file_without_popup_style_fields_defaults_to_monospace_and_theme_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.popup_font, "monospace");
        assert_eq!(config.popup_size, 13);
        assert_eq!(config.popup_color, "");
        assert_eq!(config.popup_background, "");
        assert_eq!(config.popup_prompt_color, "");
    }

    #[test]
    fn old_file_without_popup_show_fields_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(config.popup_show_prompt);
        assert!(config.popup_show_phrase);
    }

    #[test]
    fn old_file_without_show_popup_field_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        assert!(load_from_path(&path).show_popup);
    }

    #[test]
    fn show_popup_off_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            show_popup: false,
            ..Default::default()
        };
        save_to_path(&path, &config).unwrap();

        assert!(!load_from_path(&path).show_popup);
    }

    #[test]
    fn old_file_without_popup_max_size_fields_defaults_to_600_by_600() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.popup_max_width, 600);
        assert_eq!(config.popup_max_height, 600);
    }

    #[test]
    fn old_file_without_popup_border_width_field_defaults_to_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.popup_border_width, 1);
    }

    #[test]
    fn old_file_without_dictionary_fields_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(config.show_dictionary);
        assert!(config.spell_check);
    }

    #[test]
    fn old_file_without_show_context_menu_field_defaults_to_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(!config.show_context_menu);
    }

    #[test]
    fn old_file_without_speech_field_defaults_to_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "google", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert!(config.enable_text_to_speech);
    }

    #[test]
    fn old_file_without_dictionary_provider_field_defaults_to_google() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "deepl", "speech_provider": "other", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.dictionary_provider, "google");
        // Independent of the other two provider axes.
        assert_eq!(config.translate_provider, "deepl");
        assert_eq!(config.speech_provider, "other");
    }

    #[test]
    fn explicit_dictionary_provider_is_respected() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, br#"{"dictionary_provider": "other"}"#).unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.dictionary_provider, "other");
        assert_eq!(config.translate_provider, "google");
    }

    #[test]
    fn old_file_without_speech_provider_field_defaults_to_google() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"translate_provider": "deepl", "theme": "dark"}"#,
        )
        .unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.speech_provider, "google");
        // Independent of the translate provider.
        assert_eq!(config.translate_provider, "deepl");
    }

    #[test]
    fn corrupt_file_returns_default_and_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, b"{ not valid json").unwrap();

        let config = load_from_path(&path);

        assert_eq!(config, GuiConfig::default());
        assert_eq!(fs::read(&path).unwrap(), b"{ not valid json");
    }

    #[test]
    fn round_trip_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            translate_provider: "yandex".to_string(),
            theme: default_theme(),
            ..Default::default()
        };

        save_to_path(&path, &config).unwrap();

        assert_eq!(load_from_path(&path), config);
    }

    #[test]
    fn input_size_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let config = GuiConfig {
            input_size: 20,
            ..Default::default()
        };

        save_to_path(&path, &config).unwrap();

        assert_eq!(load_from_path(&path).input_size, 20);
    }

    #[test]
    fn reload_noop_when_mtime_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();

        let mut manager = GuiConfigManager::new_for_test(path);

        assert!(!manager.check_and_reload());
        assert_eq!(manager.config().translate_provider, "google");
    }

    #[test]
    fn update_applies_in_memory_persists_and_refreshes_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());

        manager
            .update(GuiConfig {
                translate_provider: "deepl".to_string(),
                theme: default_theme(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(manager.config().translate_provider, "deepl");
        let on_disk: GuiConfig = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk.translate_provider, "deepl");
        // last_modified was refreshed by update() itself, so a reload right after
        // finds nothing new to pick up.
        assert!(!manager.check_and_reload());
    }

    #[test]
    fn update_persists_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());

        manager
            .update(GuiConfig {
                translate_provider: "google".to_string(),
                theme: "dark".to_string(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(manager.config().theme, "dark");
        let on_disk: GuiConfig = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk.theme, "dark");
    }

    #[test]
    fn reload_picks_up_valid_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());

        wait_for_mtime_tick();
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "deepl".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(manager.check_and_reload());
        assert_eq!(manager.config().translate_provider, "deepl");
    }

    #[test]
    fn reload_keeps_previous_config_on_corrupt_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
                theme: default_theme(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());

        wait_for_mtime_tick();
        fs::write(&path, b"{ not valid json").unwrap();

        assert!(!manager.check_and_reload());
        assert_eq!(manager.config().translate_provider, "google");
        assert_eq!(fs::read(&path).unwrap(), b"{ not valid json");
    }

    // --- Provider profiles (`provider_options`) -----------------------------------------

    const PROFILES_JSON: &str = r#"{
        "translate_provider": "work",
        "provider_options": {
            "Work": {"type": "google", "API_Key": "file-key", "timeout_secs": "15"},
            "deepl": {"api_key": "deepl-key"},
            "llm": {"type": "openai-compat"}
        }
    }"#;

    #[test]
    fn hidden_providers_are_lowercased_and_survive_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(
            &path,
            br#"{"hidden_providers": [" DeepL ", "work", "deepl", ""]}"#,
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());
        let loaded = manager.config().clone();
        assert_eq!(loaded.hidden_providers, ["deepl", "work"]);
        manager.update(loaded).unwrap();
        assert_eq!(load_from_path(&path).hidden_providers, ["deepl", "work"]);
        // A file without the key hides nothing.
        fs::write(&path, br#"{}"#).unwrap();
        assert!(load_from_path(&path).hidden_providers.is_empty());
    }

    #[test]
    fn old_file_without_provider_options_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, br#"{"translate_provider": "google"}"#).unwrap();
        assert!(load_from_path(&path).provider_options.is_empty());
    }

    /// The trap Stage F guards against: `update()` rewrites the whole file from
    /// `GuiConfig`, so anything not modeled there would be dropped.
    #[test]
    fn provider_options_survive_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, PROFILES_JSON).unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());
        let loaded = manager.config().clone();
        assert_eq!(
            loaded.provider_options.get("work").unwrap()["api_key"],
            "file-key"
        );
        // Debug output (e.g. a line in tagent-gui.log) never shows a configured key.
        assert!(!format!("{loaded:?}").contains("file-key"));

        manager
            .update(GuiConfig {
                theme: "dark".to_string(),
                ..loaded.clone()
            })
            .unwrap();
        let on_disk = load_from_path(&path);
        assert_eq!(on_disk.provider_options, loaded.provider_options);
        assert_eq!(on_disk.theme, "dark");
    }

    #[test]
    fn provider_choice_merges_config_and_environment() {
        let config: GuiConfig = serde_json::from_str(PROFILES_JSON).unwrap();
        let no_env = |_: &str| None;

        let work = config.provider_choice_using("work", no_env);
        assert_eq!(work.name, "work");
        assert_eq!(work.options.get("api_key"), Some("file-key"));
        assert_eq!(work.options.get("type"), Some("google"));

        let env = |var: &str| (var == "TAGENT_WORK_API_KEY").then(|| "env-key".to_string());
        let work = config.provider_choice_using("WORK", env);
        assert_eq!(work.options.get("api_key"), Some("env-key"));
        assert_eq!(work.options.get("timeout_secs"), Some("15"));

        // The options reach the factory: a profile of the google kind builds and is named.
        let provider = tagent::providers::create_provider_with(&work.name, &work.options).unwrap();
        assert_eq!(provider.name(), "Google Translate (work)");
        // Debug output (e.g. a log line) never shows a key.
        assert!(!format!("{work:?}").contains("env-key"));
    }

    #[test]
    fn profiles_are_offered_on_the_axes_their_kind_supports() {
        let config: GuiConfig = serde_json::from_str(PROFILES_JSON).unwrap();
        // "work" is a google profile; "deepl"/"llm" are kinds this axis doesn't have.
        assert_eq!(config.profiles_of_kinds(&["google"]), ["work"]);
        assert_eq!(config.profiles_of_kinds(&["google", "deepl"]), ["work"]);
        assert_eq!(
            config.profiles_of_kinds(&["google", "openai-compat"]),
            ["llm", "work"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn config_file_is_private_after_every_write() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;

        // Created with the default config on first load.
        load_from_path(&path);
        assert_eq!(mode(&path), 0o600);

        // An existing, world-readable file is tightened by update().
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());
        manager.update(GuiConfig::default()).unwrap();
        assert_eq!(mode(&path), 0o600);
    }
}

/// Keeps the user book's `tagent-gui.json` page (`docs/user`) naming every setting. The
/// book lives outside this package, so a copy built from crates.io skips the check.
#[cfg(test)]
mod user_docs_tests {
    use super::*;

    #[test]
    fn every_setting_is_on_the_user_book_page() {
        let page = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/user/src/reference/tagent-gui-json.md");
        let Ok(page) = std::fs::read_to_string(&page) else {
            eprintln!("{} not found; user book check skipped", page.display());
            return;
        };
        let value = serde_json::to_value(GuiConfig::default()).unwrap();
        let missing: Vec<&String> = value
            .as_object()
            .unwrap()
            .keys()
            .filter(|key| !page.contains(&format!("`{key}`")))
            .collect();
        assert!(
            missing.is_empty(),
            "docs/user/src/reference/tagent-gui-json.md doesn't name {missing:?}"
        );
    }
}
