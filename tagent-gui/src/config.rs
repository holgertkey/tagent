use crate::platform::keycodes;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn default_translate_provider() -> String {
    "google".to_string()
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

/// Default global hotkey, same format and default value as `tagent-cli`'s `TranslateHotkey`.
fn default_translate_hotkey() -> String {
    "Alt+Q".to_string()
}

/// `tagent-gui`'s own configuration, independent of `tagent-cli.conf`.
///
/// Stored as plain, pretty-printed JSON at [`config_path`] and meant to be
/// hand-editable (not only written via a future Settings window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuiConfig {
    #[serde(default = "default_translate_provider")]
    pub translate_provider: String,
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
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            translate_provider: default_translate_provider(),
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
            block_spacing_px: default_block_spacing_px(),
            phrases_spacing_px: default_phrases_spacing_px(),
            show_prompt: default_show_prompt(),
            translate_hotkey: default_translate_hotkey(),
        }
    }
}

/// Returns the platform-default path for `tagent-gui.json` (not created here).
///
/// - **Windows**: `%APPDATA%\tagent-gui\tagent-gui.json`
/// - **Linux/macOS**: `~/.config/tagent-gui/tagent-gui.json`
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
            Ok(config) => config,
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

/// Writes `config` to `path` as pretty-printed JSON, creating parent directories.
pub fn save_to_path(path: &Path, config: &GuiConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
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
                Ok(config) => {
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

    /// Validate that the hotkey doesn't conflict with critical system shortcuts
    pub fn validate_hotkey(hotkey: &HotkeyType) -> Result<(), String> {
        match hotkey {
            // Only allow F1-F12 as single keys
            HotkeyType::SingleKey { vk_code }
                if *vk_code < keycodes::KEY_F1 || *vk_code > keycodes::KEY_F12 =>
            {
                return Err("Single keys are only allowed for F1-F12. For other keys like Space, Tab, etc., use modifier combinations (e.g., Alt+Space, Ctrl+T)".to_string());
            }
            HotkeyType::SingleKey { .. } => {}
            HotkeyType::ModifierCombo { modifiers, key } => {
                // Forbid Shift-only combinations (Shift+Key interferes with text input)
                // Allow multi-modifier combinations (Ctrl+Shift+Key, Alt+Shift+Key, etc.)
                let only_shift = modifiers.iter().all(|&m| {
                    m == keycodes::KEY_SHIFT
                        || m == keycodes::KEY_LSHIFT
                        || m == keycodes::KEY_RSHIFT
                });

                if only_shift {
                    return Err("Shift+Key combinations are not allowed (interferes with text input). Use multi-modifier combinations like Ctrl+Shift+T or Alt+Shift+Space instead.".to_string());
                }

                // Warn about common system shortcuts
                let has_ctrl = modifiers.iter().any(|&m| {
                    m == keycodes::KEY_CONTROL
                        || m == keycodes::KEY_LCONTROL
                        || m == keycodes::KEY_RCONTROL
                });
                let has_alt = modifiers.iter().any(|&m| {
                    m == keycodes::KEY_ALT || m == keycodes::KEY_LALT || m == keycodes::KEY_RALT
                });
                let has_win = modifiers
                    .iter()
                    .any(|&m| m == keycodes::KEY_LWIN || m == keycodes::KEY_RWIN);

                // Block dangerous combinations
                if has_ctrl && has_alt && *key == keycodes::KEY_DELETE {
                    return Err("Ctrl+Alt+Delete is reserved by the system".to_string());
                }

                if has_win && *key == 'L' as u32 {
                    return Err("Win+L (lock screen) is reserved by the system".to_string());
                }

                // Warnings for common shortcuts (don't block, just warn in logs)
                if has_alt && *key == keycodes::KEY_F4 {
                    eprintln!("Warning: Alt+F4 may close windows");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;

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

        let hotkey = HotkeyParser::parse("Alt+Shift+Space").unwrap();
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
    fn old_file_without_hotkey_field_defaults_to_alt_q() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, br#"{"translate_provider": "google", "theme": "dark"}"#).unwrap();

        let config = load_from_path(&path);

        assert_eq!(config.translate_hotkey, "Alt+Q");
    }

    #[test]
    fn old_file_without_style_fields_defaults_to_monospace_and_theme_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        fs::write(&path, br#"{"translate_provider": "google", "theme": "dark"}"#).unwrap();

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
        assert_eq!(config.block_spacing_px, 20);
        assert_eq!(config.phrases_spacing_px, 2);
        assert!(config.show_prompt);
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

    impl GuiConfigManager {
        /// Test-only constructor pointed at an arbitrary path instead of the
        /// platform-default one.
        fn new_for_test(path: PathBuf) -> Self {
            let config = load_from_path(&path);
            let last_modified = mtime(&path);
            Self {
                path,
                config,
                last_modified,
            }
        }
    }
}
