use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn default_translate_provider() -> String {
    "google".to_string()
}

/// `tagent-gui`'s own configuration, independent of `tagent-cli.conf`.
///
/// Stored as plain, pretty-printed JSON at [`config_path`] and meant to be
/// hand-editable (not only written via a future Settings window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuiConfig {
    #[serde(default = "default_translate_provider")]
    pub translate_provider: String,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            translate_provider: default_translate_provider(),
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
        };
        save_to_path(&path, &config).unwrap();

        assert_eq!(load_from_path(&path), config);
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
            },
        )
        .unwrap();

        let mut manager = GuiConfigManager::new_for_test(path);

        assert!(!manager.check_and_reload());
        assert_eq!(manager.config().translate_provider, "google");
    }

    #[test]
    fn reload_picks_up_valid_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_config_path(&dir);
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "google".to_string(),
            },
        )
        .unwrap();
        let mut manager = GuiConfigManager::new_for_test(path.clone());

        wait_for_mtime_tick();
        save_to_path(
            &path,
            &GuiConfig {
                translate_provider: "deepl".to_string(),
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
