use crate::platform::keycodes;
use chrono::{DateTime, Utc};
use colored::Colorize;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Runtime configuration loaded from `tagent-cli.conf`.
///
/// All fields correspond directly to INI keys documented inside the generated
/// configuration file. The [`Default`] impl reflects the same defaults that are
/// written when a new configuration file is created.
///
/// The config file is located at:
/// - **Windows**: `%APPDATA%\tagent-cli\tagent-cli.conf`
/// - **Linux/macOS**: `~/.config/tagent-cli/tagent-cli.conf`
#[derive(Debug, Clone)]
pub struct Config {
    /// BCP-47 language code for the source language, or `"Auto"` for auto-detection.
    pub source_language: String,
    /// BCP-47 language code for the translation target (e.g. `"Russian"`, `"English"`).
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
    /// Terminal color for the target-language prompt (e.g. `"BrightYellow"`). `"None"` disables.
    pub target_prompt_color: String,
    /// Terminal color for the dictionary prompt. `"None"` disables.
    pub dictionary_prompt_color: String,
    /// Terminal color for the source-language prompt. `"None"` disables.
    pub source_prompt_color: String,
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
            source_language: "Auto".to_string(),
            target_language: "Russian".to_string(),
            show_terminal_on_translate: true,
            auto_hide_terminal_seconds: 3,
            show_dictionary: true,
            spell_check: true,
            dictionary_provider: "google".to_string(), // Default dictionary provider
            copy_to_clipboard: false,
            save_translation_history: false,
            history_file: default_history,
            target_prompt_color: "BrightYellow".to_string(), // Default bright yellow for target
            dictionary_prompt_color: "BrightYellow".to_string(), // Default bright yellow for dictionary
            source_prompt_color: "None".to_string(),             // Default no color for source
            translate_hotkey: "Alt+A".to_string(),               // Default translation hotkey
            enable_text_to_speech: true,                         // TTS enabled by default
            speech_hotkey: "Alt+S".to_string(),                  // Default speech hotkey
            enable_speech_hotkey: true,                          // Enable speech hotkey by default
            translate_provider: "google".to_string(),            // Default translation provider
            speech_provider: "google".to_string(),               // Default speech provider
        }
    }
}

/// Target language substituted wherever `"Auto"` would otherwise become the
/// target: auto-detection only makes sense for the source language.
pub const AUTO_TARGET_FALLBACK: &str = "English";

/// A source/target language pair resolved from user input (`/l`, `-l`, or the
/// config file), with `"Auto"` never left as the target.
///
/// `notices` holds the messages to show the user about what was adjusted or is
/// worth knowing (an `"Auto"` target replaced with [`AUTO_TARGET_FALLBACK`], or a
/// same-language pair). A same-language pair is allowed on purpose: it's the
/// natural way to express a future monolingual (explanatory) dictionary lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePair {
    /// Source language name (may be `"Auto"`).
    pub source: String,
    /// Target language name (never `"Auto"`).
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
                AUTO_TARGET_FALLBACK
            ));
            AUTO_TARGET_FALLBACK.to_string()
        } else {
            target.to_string()
        };
        Self::finish(source.to_string(), target, notices)
    }

    /// Resolves the pair produced by swapping `source` and `target`. An `"Auto"`
    /// source has no concrete language to become the new target, so
    /// [`AUTO_TARGET_FALLBACK`] takes its place.
    pub fn swapped(source: &str, target: &str) -> Self {
        let mut notices = Vec::new();
        let new_target = if is_auto(source) {
            notices.push(format!(
                "Source was Auto; using {} as the new target",
                AUTO_TARGET_FALLBACK
            ));
            AUTO_TARGET_FALLBACK.to_string()
        } else {
            source.to_string()
        };
        Self::finish(target.to_string(), new_target, notices)
    }

    fn finish(source: String, target: String, mut notices: Vec<String>) -> Self {
        if !is_auto(&source)
            && tagent::languages::name_to_code(&source).eq_ignore_ascii_case(
                tagent::languages::name_to_code(&target),
            )
        {
            notices.push("Note: source and target are the same language".to_string());
        }
        Self {
            source,
            target,
            notices,
        }
    }
}

/// Compact `source → target` label from language codes (e.g. `auto → ru`), used in
/// the interactive prompt and the terminal window title.
pub fn language_pair_label(source_code: &str, target_code: &str) -> String {
    format!("{} → {}", source_code, target_code)
}

/// Whether a language name/code means auto-detection.
fn is_auto(language: &str) -> bool {
    language.trim().eq_ignore_ascii_case("auto")
}

/// Formats a provider factory error for display. When the configured name is unknown, it
/// appends the supported values and the setting to change (`setting` is the config key,
/// e.g. `"SpeechProvider"`), because `tagent`'s own message only echoes the bad name.
/// Any other error is shown as is.
pub fn provider_error_message(
    error: &tagent::error::Error,
    setting: &str,
    supported: &[&str],
) -> String {
    match error {
        tagent::error::Error::UnknownProvider(_) => format!(
            "{error} (supported values for {setting}: {})",
            supported.join(", ")
        ),
        _ => error.to_string(),
    }
}

/// Thread-safe configuration manager with live-reload support.
///
/// `ConfigManager` loads `tagent-cli.conf` on construction and can reload it at
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
    /// Returns the platform-default path for `tagent-cli.conf`, creating parent directories as needed.
    ///
    /// - **Windows**: `%APPDATA%\tagent-cli\tagent-cli.conf`
    /// - **Linux/macOS**: `~/.config/tagent-cli/tagent-cli.conf`
    pub fn get_default_config_path() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("tagent-cli");

        // Create directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("tagent-cli.conf"))
    }

    /// Returns the platform-default path for the interactive-mode line-editing history file,
    /// creating parent directories as needed.
    ///
    /// This is separate from [`Config::history_file`], which logs translation *results* for
    /// the user to read; this file stores rustyline's input-line history instead.
    ///
    /// - **Windows**: `%APPDATA%\tagent-cli\interactive_history.txt`
    /// - **Linux/macOS**: `~/.config/tagent-cli/interactive_history.txt`
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
        let default_config = Config::default();
        let ini_content = self.create_ini_content(&default_config);

        fs::write(&self.config_path, ini_content)?;
        println!("Created default configuration file: {}", self.config_path);

        // Update last modified time
        self.update_last_modified_time()?;

        Ok(())
    }

    /// Create INI format content
    fn create_ini_content(&self, config: &Config) -> String {
        format!(
            r#"; Text Translator Configuration File
; This program translates selected text using keyboard shortcuts
;
; Usage:
; 1. Select text in any application
; 2. Press the translation hotkey (default: Alt+A)
; 3. Translation will be shown (enable CopyToClipboard below to also copy it)
; 4. Type /q or /e in the interactive prompt to exit the program
;
; Configuration changes take effect immediately (no restart required)

[Provider]
; Translation service provider
; Supported values: {translate_providers}
; Default: google
TranslateProvider = {}

[Translation]
; Source language for translation
; Supported values: Auto, English, Russian, Spanish, French, German, Chinese,
; Japanese, Korean, Italian, Portuguese, Dutch, Polish, Turkish, Arabic, Hindi
; Use "Auto" for automatic language detection
SourceLanguage = {}

; Target language for translation
; Supported values: Russian, English, Spanish, French, German, etc.
TargetLanguage = {}

[Dictionary]
; Show dictionary entry for single words instead of simple translation
; Set to true to show detailed word information (definitions, part of speech, examples)
; Set to false to always use simple translation
; This feature works best with English words
ShowDictionary = {}

; Check spelling of single words and suggest the correct word if a typo is detected
; When enabled, misspelled words are automatically corrected and the correction is shown
; Set to false to disable spell checking (typos will fall back to simple translation)
SpellCheck = {}

; Dictionary lookup backend, independent of TranslateProvider
; Supported values: {dictionary_providers}
; Default: google
; Note: Requires application restart to take effect
DictionaryProvider = {}

[Interface]
; Show terminal window on top when translating
; Set to true to show terminal window during translation
; Set to false to keep terminal in background
ShowTerminalOnTranslate = {}

; Auto-hide terminal after translation (in seconds)
; Set to 0 to keep terminal visible (no auto-hide)
; Set to any number > 0 to auto-hide after that many seconds
; Example: 3 = hide terminal after 3 seconds
AutoHideTerminalSeconds = {}

; Automatically copy translation result to clipboard
; Set to true to automatically copy result to clipboard after translation
; Set to false to display result only (without copying to clipboard)
; When enabled, you can paste the result anywhere with Ctrl+V
CopyToClipboard = {}

[Colors]
; Color for source language prompt (e.g., "[Auto]: ", "[English]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: None (no color)
SourcePromptColor = {}

; Color for target language prompt (e.g., "[Russian]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: BrightYellow
TargetPromptColor = {}

; Color for dictionary prompt (e.g., "[Word]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: BrightYellow
DictionaryPromptColor = {}

[History]
; Save translation history to file
; Set to true to save all translations with timestamps to a text file
; Set to false to disable history logging
; History includes original text, translation, language direction, and timestamp
SaveTranslationHistory = {}

; History file path
; File where translation history will be saved
; Path can be absolute or relative to the program directory
; File will be created automatically if it doesn't exist
HistoryFile = {}

[Hotkeys]
; Hotkey for translation
; Supported formats:
;   - Single keys: F1-F12 ONLY (other keys must use modifiers)
;   - Modifier combinations: Alt+Q, Alt+Space, Ctrl+Shift+T, Win+T, etc.
;     NOTE: Shift+Key is NOT allowed (interferes with text input)
;     Use multi-modifier combos instead: Ctrl+Shift+T, Alt+Shift+Space
;   - Double-press: Ctrl+Ctrl, F8+F8, Shift+Shift, Alt+Alt, etc.
; Examples:
;   TranslateHotkey = Alt+A (default)
;   TranslateHotkey = Ctrl+Ctrl
;   TranslateHotkey = F9
;   TranslateHotkey = Alt+Space
;   TranslateHotkey = Ctrl+Shift+C
;   TranslateHotkey = F8+F8
; Note: Hotkey changes require application restart to take effect
TranslateHotkey = {}

[Speech]
; Enable text-to-speech functionality
; Set to true to enable TTS for selected text (default)
; Set to false to disable TTS completely
EnableTextToSpeech = {}

; Hotkey for text-to-speech
; Supported formats (same as alternative hotkey):
;   - Single keys: F1-F12 ONLY
;   - Modifier combinations: Alt+S, Ctrl+Shift+S, etc.
;   - Double-press: Alt+Alt, Shift+Shift, etc.
; Examples:
;   SpeechHotkey = Alt+S
;   SpeechHotkey = F10
;   SpeechHotkey = Ctrl+Shift+S
; Note: Hotkey changes require application restart to take effect
SpeechHotkey = {}

; Enable or disable the speech hotkey
; Set to true to enable the speech hotkey
; Set to false to disable speech hotkey
EnableSpeechHotkey = {}

; Speech synthesis backend, independent of TranslateProvider
; Supported values: {speech_providers}
; Default: google
SpeechProvider = {}
"#,
            config.translate_provider,
            config.source_language,
            config.target_language,
            config.show_dictionary,
            config.spell_check,
            config.dictionary_provider,
            config.show_terminal_on_translate,
            config.auto_hide_terminal_seconds,
            config.copy_to_clipboard,
            config.source_prompt_color,
            config.target_prompt_color,
            config.dictionary_prompt_color,
            config.save_translation_history,
            config.history_file,
            config.translate_hotkey,
            config.enable_text_to_speech,
            config.speech_hotkey,
            config.enable_speech_hotkey,
            config.speech_provider,
            // Named (not positional) so adding them can't shift the positional values.
            translate_providers = tagent::providers::TRANSLATION_PROVIDERS.join(", "),
            dictionary_providers = tagent::providers::DICTIONARY_PROVIDERS.join(", "),
            speech_providers = tagent::providers::SPEECH_PROVIDERS.join(", "),
        )
    }

    /// Load configuration from INI file
    fn load_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let content = fs::read_to_string(&self.config_path)?;
        let parsed_config = self.parse_ini(&content)?;

        let source_lang = parsed_config
            .get("Translation")
            .and_then(|section| section.get("SourceLanguage"))
            .cloned()
            .unwrap_or_else(|| "Auto".to_string());

        let target_lang = parsed_config
            .get("Translation")
            .and_then(|section| section.get("TargetLanguage"))
            .cloned()
            .unwrap_or_else(|| "Russian".to_string());
        // A hand-edited `TargetLanguage = Auto` is replaced in memory only; the file
        // itself is left untouched.
        let resolved = LanguagePair::new(&source_lang, &target_lang);
        for notice in &resolved.notices {
            eprintln!("Warning: {} (TargetLanguage in config)", notice);
        }
        let (source_lang, target_lang) = (resolved.source, resolved.target);

        let show_dictionary = parsed_config
            .get("Dictionary")
            .and_then(|section| section.get("ShowDictionary"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let spell_check = parsed_config
            .get("Dictionary")
            .and_then(|section| section.get("SpellCheck"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let dictionary_provider = parsed_config
            .get("Dictionary")
            .and_then(|section| section.get("DictionaryProvider"))
            .cloned()
            .unwrap_or_else(|| "google".to_string());

        let show_terminal = parsed_config
            .get("Interface")
            .and_then(|section| section.get("ShowTerminalOnTranslate"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let auto_hide_seconds = parsed_config
            .get("Interface")
            .and_then(|section| section.get("AutoHideTerminalSeconds"))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3);

        // Try new location first, fallback to old location for backward compatibility
        let copy_to_clipboard = parsed_config
            .get("Interface")
            .and_then(|section| section.get("CopyToClipboard"))
            .or_else(|| {
                parsed_config
                    .get("Translation")
                    .and_then(|section| section.get("CopyToClipboard"))
            })
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let save_translation_history = parsed_config
            .get("History")
            .and_then(|section| section.get("SaveTranslationHistory"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let history_file = parsed_config
            .get("History")
            .and_then(|section| section.get("HistoryFile"))
            .cloned()
            .unwrap_or_else(|| "translation_history.txt".to_string());

        // Color settings
        // Try new names first, fallback to old names for backward compatibility
        let target_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| {
                section
                    .get("TargetPromptColor")
                    .or_else(|| section.get("TranslationPromptColor"))
            })
            .cloned()
            .unwrap_or_else(|| "BrightYellow".to_string());

        let dictionary_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| section.get("DictionaryPromptColor"))
            .cloned()
            .unwrap_or_else(|| "BrightYellow".to_string());

        let source_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| {
                section
                    .get("SourcePromptColor")
                    .or_else(|| section.get("AutoPromptColor"))
            })
            .cloned()
            .unwrap_or_else(|| "None".to_string());

        // Hotkey settings
        let translate_hotkey = parsed_config
            .get("Hotkeys")
            .and_then(|section| section.get("TranslateHotkey"))
            .cloned()
            .unwrap_or_else(|| "Alt+A".to_string());

        // Speech settings
        let enable_text_to_speech = parsed_config
            .get("Speech")
            .and_then(|section| section.get("EnableTextToSpeech"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let speech_hotkey = parsed_config
            .get("Speech")
            .and_then(|section| section.get("SpeechHotkey"))
            .cloned()
            .unwrap_or_else(|| "Alt+S".to_string());

        let enable_speech_hotkey = parsed_config
            .get("Speech")
            .and_then(|section| section.get("EnableSpeechHotkey"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        // Provider settings
        let translate_provider = parsed_config
            .get("Provider")
            .and_then(|section| section.get("TranslateProvider"))
            .cloned()
            .unwrap_or_else(|| "google".to_string());

        let speech_provider = parsed_config
            .get("Speech")
            .and_then(|section| section.get("SpeechProvider"))
            .cloned()
            .unwrap_or_else(|| "google".to_string());

        let new_config = Config {
            source_language: source_lang,
            target_language: target_lang,
            copy_to_clipboard,
            show_dictionary,
            spell_check,
            dictionary_provider,
            show_terminal_on_translate: show_terminal,
            auto_hide_terminal_seconds: auto_hide_seconds,
            save_translation_history,
            history_file,
            target_prompt_color,
            dictionary_prompt_color,
            source_prompt_color,
            translate_hotkey,
            enable_text_to_speech,
            speech_hotkey,
            enable_speech_hotkey,
            translate_provider,
            speech_provider,
        };

        if let Ok(mut config) = self.config.lock() {
            *config = new_config;
        }

        self.update_last_modified_time()?;

        Ok(())
    }

    /// Parse INI format content
    fn parse_ini(
        &self,
        content: &str,
    ) -> Result<HashMap<String, HashMap<String, String>>, Box<dyn Error + Send + Sync>> {
        let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut current_section: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            // Section header
            if line.starts_with('[') && line.ends_with(']') {
                let section_name = line[1..line.len() - 1].to_string();
                current_section = Some(section_name.clone());
                sections.entry(section_name).or_default();
            }
            // Key-value pair
            else if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();

                if let Some(section_name) = &current_section {
                    if let Some(section) = sections.get_mut(section_name) {
                        section.insert(key, value);
                    }
                }
            }
        }

        Ok(sections)
    }

    /// Save the current in-memory configuration to the config file.
    pub fn save_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let config = self.get_config();
        let ini_content = self.create_ini_content(&config);
        fs::write(&self.config_path, ini_content)?;
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
    /// Callers resolve user input through [`LanguagePair`] first, so `target` is
    /// never `"Auto"`.
    pub fn set_languages(&self, source: &str, target: &str) {
        if let Ok(mut config) = self.config.lock() {
            config.source_language = source.to_string();
            config.target_language = target.to_string();
        }
    }

    /// Print the interactive-mode banner: version, the current language pair,
    /// active hotkeys (when `config` is available) and a command summary.
    ///
    /// Shown at startup and again after `/clear`.
    pub fn display_banner(config: Option<&Config>) {
        println!("Text Translator v{}", env!("CARGO_PKG_VERSION"));
        println!();

        if let Some(config) = config {
            println!(
                "Languages: {} ({}) -> {} ({})",
                config.source_language,
                tagent::languages::name_to_code(&config.source_language),
                config.target_language,
                tagent::languages::name_to_code(&config.target_language)
            );
            println!();

            println!("Active Hotkeys:");
            println!("  Translation: {}", config.translate_hotkey);
            if config.enable_speech_hotkey && config.enable_text_to_speech {
                println!("  Speech: {}", config.speech_hotkey);
            }
            println!();
        }

        println!(
            r#"Commands:
  /h (help), /c (config), /s (speech), /ss (speak translation)
  /l (lang), /save, /clear, /q (quit)"#
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
        println!("  -h, --help     Show this help message");
        println!("  -c, --config   Show current configuration");
        println!("  -v, --version  Show version information");
        println!("  -s, --speech   Speak the following text using text-to-speech");
        println!("  -l, --lang     Set languages: -l <target> or -l <source> <target>");
        println!();

        println!("SUPPORTED LANGUAGES (name or code, e.g. -l German or -l de):");
        println!("  Auto (auto)        English (en)       Russian (ru)");
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
        println!("   - Select text anywhere in Windows");
        println!("   - Press configured hotkey (default: Alt+A)");
        println!("   - Result copied to clipboard automatically");
        println!("   - Configure hotkeys in tagent-cli.conf [Hotkeys] section");
        println!();

        println!("INTERACTIVE COMMANDS (must start with slash):");
        println!("  /h, /help, /?           - Show this help");
        println!("  /c, /config             - Show current configuration");
        println!("  /v, /version            - Show version information");
        println!(
            "  /s, /speech <text>      - Speak text using text-to-speech (press Esc to cancel)"
        );
        println!("  /s, /speech             - Speak the last translated phrase");
        println!("  /ss                     - Speak the translation of the last phrase");
        println!("  /l, /lang               - Swap source and target languages");
        println!("  /l, /lang <target>      - Set target language (source=Auto)");
        println!("  /l, /lang <src> <tgt>   - Set source and target languages");
        println!("  /save                   - Save current configuration to file");
        println!("  /clear, /cls            - Clear screen");
        println!("  /q, /quit, /e, /exit,   - Exit program");
        println!();

        println!("CONFIGURATION:");
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("  Config file: {}", config_path.display());
        } else {
            println!("  Config file: tagent-cli.conf (typically in %APPDATA%\\tagent-cli\\)");
        }
        println!();
        println!("  Edit 'tagent-cli.conf' to change translation settings:");
        println!("  - SourceLanguage: Source language (Auto, English, Russian, etc.)");
        println!("  - TargetLanguage: Target language (Russian, English, etc.)");
        println!(
            "  - TranslateProvider: Translation backend ({})",
            tagent::providers::TRANSLATION_PROVIDERS.join(", ")
        );
        println!("  - ShowDictionary: Enable dictionary lookup for single words");
        println!(
            "  - DictionaryProvider: Dictionary backend ({})",
            tagent::providers::DICTIONARY_PROVIDERS.join(", ")
        );
        println!("  - CopyToClipboard: Copy results to clipboard");
        println!("  - TranslateHotkey: Custom hotkey (Ctrl+Ctrl, Alt+Q, F9, etc.)");
        println!("  - SpeechHotkey: Hotkey for text-to-speech (Alt+S, F10, etc.)");
        println!(
            "  - SpeechProvider: Text-to-speech backend ({})",
            tagent::providers::SPEECH_PROVIDERS.join(", ")
        );
        println!("  - SaveTranslationHistory: Save all translations to file");
        println!();

        println!("FEATURES:");
        println!("- Same translation engine for all modes");
        println!("- Google Translate API with dictionary lookups");
        println!("- Configuration hot-reload (changes take effect immediately)");
        println!("- Configurable hotkeys with various combinations");
        println!("- Text-to-speech support (Google TTS)");
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
        self.check_and_reload()?;
        let config = self.get_config();
        let (source_code, target_code) = self.get_language_codes();

        println!();
        println!("=== Current Configuration ===");
        println!("Translation Provider: {}", config.translate_provider);
        println!("Dictionary Provider: {}", config.dictionary_provider);
        println!("Speech Provider: {}", config.speech_provider);
        println!();
        println!(
            "Source Language: {} ({})",
            config.source_language, source_code
        );
        println!(
            "Target Language: {} ({})",
            config.target_language, target_code
        );
        println!(
            "Show Dictionary: {}",
            if config.show_dictionary {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!(
            "Copy to Clipboard: {}",
            if config.copy_to_clipboard {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!();
        println!("Translation Hotkey: {}", config.translate_hotkey);
        println!(
            "Show Terminal on Translate: {}",
            if config.show_terminal_on_translate {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!(
            "Auto-hide Terminal: {}",
            if config.auto_hide_terminal_seconds == 0 {
                "Disabled".to_string()
            } else {
                format!("{} seconds", config.auto_hide_terminal_seconds)
            }
        );
        println!();
        println!(
            "Text-to-Speech: {}",
            if config.enable_text_to_speech {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!("Speech Hotkey: {}", config.speech_hotkey);
        println!(
            "Speech Hotkey Enabled: {}",
            if config.enable_speech_hotkey {
                "Yes"
            } else {
                "No"
            }
        );
        println!();
        println!(
            "Save Translation History: {}",
            if config.save_translation_history {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        println!("History File: {}", config.history_file);
        println!();

        // Show config file location
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("Config file: {}", config_path.display());
        } else {
            println!("Config file: tagent-cli.conf");
        }
        println!("Edit this file to change settings (changes take effect immediately)");
        println!("============================");
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
            self.load_config()?;
            return Ok(true);
        }

        Ok(false)
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

    /// Normalize language input: accept both names ("English") and codes ("en"),
    /// always return the full language name
    pub fn normalize_language(input: &str) -> String {
        // First check if it's already a known language name
        let code = tagent::languages::name_to_code(input);
        if code != input || input.to_lowercase() == "auto" {
            // It was a known name, return as-is (capitalized)
            return Self::capitalize_first(input);
        }
        // Otherwise try as a code
        let name = tagent::languages::code_to_name(input);
        if name != input {
            return name.to_string();
        }
        // Unknown — return as-is
        input.to_string()
    }

    /// Capitalize the first letter of a string
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
        }
    }

    /// Get language codes for translation
    pub fn get_language_codes(&self) -> (String, String) {
        let config = self.get_config();
        let source_code = tagent::languages::name_to_code(&config.source_language);
        let target_code = tagent::languages::name_to_code(&config.target_language);

        (source_code.to_string(), target_code.to_string())
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
            // Only allow F1-F12 or a modifier key to be double-pressed -- doubling an
            // ordinary letter/digit/etc. key (e.g. "Q+Q") is indistinguishable from just
            // typing that letter twice while using the app normally.
            HotkeyType::DoublePress { vk_code, .. }
                if !(*vk_code >= keycodes::KEY_F1 && *vk_code <= keycodes::KEY_F12)
                    && !matches!(
                        *vk_code,
                        keycodes::KEY_CONTROL
                            | keycodes::KEY_ALT
                            | keycodes::KEY_SHIFT
                            | keycodes::KEY_LWIN
                            | keycodes::KEY_RWIN
                    ) =>
            {
                return Err("Double-press is only allowed for F1-F12 or modifier keys (Ctrl, Alt, Shift, Win). For other keys, use a modifier combination instead (e.g., Ctrl+Q).".to_string());
            }
            HotkeyType::DoublePress { .. } => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_pair_label_uses_codes_with_an_arrow() {
        assert_eq!(language_pair_label("auto", "ru"), "auto → ru");
    }

    /// `/l auto` and `-l auto`: an "Auto" target is replaced, never kept.
    #[test]
    fn language_pair_replaces_auto_target_with_fallback() {
        let pair = LanguagePair::new("Auto", "Auto");
        assert_eq!(pair.source, "Auto");
        assert_eq!(pair.target, AUTO_TARGET_FALLBACK);
        assert_eq!(
            pair.notices,
            vec!["Target can't be Auto; using English instead".to_string()]
        );

        // Any spelling of "auto" counts.
        assert_eq!(LanguagePair::new("Russian", " AUTO ").target, AUTO_TARGET_FALLBACK);
    }

    #[test]
    fn language_pair_keeps_a_normal_pair_without_notices() {
        let pair = LanguagePair::new("Auto", "Russian");
        assert_eq!(
            (pair.source.as_str(), pair.target.as_str()),
            ("Auto", "Russian")
        );
        assert!(pair.notices.is_empty());
    }

    /// Same-language pairs are allowed (future monolingual dictionaries), just
    /// noted -- including one produced by the Auto-target fallback.
    #[test]
    fn language_pair_allows_same_language_with_a_note() {
        let pair = LanguagePair::new("English", "Auto");
        assert_eq!(
            (pair.source.as_str(), pair.target.as_str()),
            ("English", "English")
        );
        assert_eq!(
            pair.notices,
            vec![
                "Target can't be Auto; using English instead".to_string(),
                "Note: source and target are the same language".to_string(),
            ]
        );

        let pair = LanguagePair::new("Russian", "russian");
        assert_eq!(
            pair.notices,
            vec!["Note: source and target are the same language".to_string()]
        );
    }

    #[test]
    fn swapped_pair_uses_fallback_for_auto_source() {
        let pair = LanguagePair::swapped("Auto", "Russian");
        assert_eq!(
            (pair.source.as_str(), pair.target.as_str()),
            ("Russian", "English")
        );
        assert_eq!(
            pair.notices,
            vec!["Source was Auto; using English as the new target".to_string()]
        );
    }

    #[test]
    fn swapped_pair_swaps_two_concrete_languages() {
        let pair = LanguagePair::swapped("English", "Russian");
        assert_eq!(
            (pair.source.as_str(), pair.target.as_str()),
            ("Russian", "English")
        );
        assert!(pair.notices.is_empty());
    }

    /// A hand-edited `TargetLanguage = Auto` is replaced in memory on load.
    #[test]
    fn test_load_config_replaces_auto_target_language() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_auto_target_{}.conf",
            std::process::id()
        ));
        fs::write(
            &path,
            "[Translation]\nSourceLanguage = Auto\nTargetLanguage = Auto\n",
        )
        .unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert_eq!(manager.get_config().source_language, "Auto");
        assert_eq!(manager.get_config().target_language, AUTO_TARGET_FALLBACK);
        // The file itself is not rewritten.
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("TargetLanguage = Auto"));

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

        let hotkey = HotkeyParser::parse("Alt+Shift+Space").unwrap();
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
    fn test_double_press_validation_only_allows_f1_to_f12_or_modifiers() {
        let hotkey = HotkeyParser::parse("F8+F8").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Ctrl+Ctrl").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Shift+Shift").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

        let hotkey = HotkeyParser::parse("Alt+Alt").unwrap();
        assert!(HotkeyParser::validate_hotkey(&hotkey).is_ok());

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
            "tagent_test_missing_speech_{}.conf",
            std::process::id()
        ));
        fs::write(
            &path,
            "[Translation]\nSourceLanguage = Auto\nTargetLanguage = Russian\n",
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
            "tagent_test_speech_hotkey_default_{}.conf",
            std::process::id()
        ));
        fs::write(&path, "[Speech]\nEnableTextToSpeech = true\n").unwrap();
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();
        assert_eq!(manager.get_config().speech_hotkey, "Alt+S");
        let _ = fs::remove_file(&path);

        let generated = manager.create_ini_content(&Config::default());
        // Anchored to the start of a line so the commented example (`;   SpeechHotkey = ...`)
        // cannot satisfy it: this must be the live setting.
        assert!(generated.contains("\nSpeechHotkey = Alt+S\n"));
        assert!(
            !generated.contains("Alt+E"),
            "generated config still mentions the old Alt+E default"
        );
    }

    #[test]
    fn test_load_config_explicit_speech_provider_is_respected() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_explicit_speech_provider_{}.conf",
            std::process::id()
        ));
        fs::write(&path, "[Speech]\nSpeechProvider = other\n").unwrap();

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
    fn test_generated_config_roundtrips_speech_provider() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_roundtrip_speech_provider_{}.conf",
            std::process::id()
        ));
        let config = Config {
            speech_provider: "roundtrip".to_string(),
            ..Config::default()
        };
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        fs::write(&path, manager.create_ini_content(&config)).unwrap();
        manager.load_config().unwrap();

        let loaded = manager.get_config();
        assert_eq!(loaded.speech_provider, "roundtrip");
        // Every other value must survive the positional `format!` template unshifted.
        assert_eq!(loaded.translate_provider, config.translate_provider);
        assert_eq!(loaded.enable_text_to_speech, config.enable_text_to_speech);
        assert_eq!(loaded.speech_hotkey, config.speech_hotkey);
        assert_eq!(loaded.enable_speech_hotkey, config.enable_speech_hotkey);
        assert_eq!(loaded.translate_hotkey, config.translate_hotkey);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_config_dictionary_provider_defaults_when_absent() {
        // Both when the whole `[Dictionary]` section is missing and when the section is
        // present without the key (an older config file).
        for (label, content) in [
            ("no_section", "[Translation]\nSourceLanguage = Auto\n"),
            ("no_key", "[Dictionary]\nShowDictionary = false\n"),
        ] {
            let path = std::env::temp_dir().join(format!(
                "tagent_test_dictionary_provider_default_{}_{}.conf",
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
            "tagent_test_explicit_dictionary_provider_{}.conf",
            std::process::id()
        ));
        fs::write(&path, "[Dictionary]\nDictionaryProvider = other\n").unwrap();

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

    /// `create_ini_content` is one positional `format!`, and `[Dictionary]` sits in the
    /// middle of the template, so a placeholder/argument pair inserted out of step would
    /// shift every later value by one slot. Every field here is deliberately non-default
    /// (bools flipped, strings unique sentinels), because a shifted value that happens to
    /// equal the default would otherwise pass unnoticed.
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
            target_prompt_color: "target-color-sentinel".to_string(),
            dictionary_prompt_color: "dict-color-sentinel".to_string(),
            source_prompt_color: "source-color-sentinel".to_string(),
            translate_hotkey: "translate-hotkey-sentinel".to_string(),
            enable_text_to_speech: !defaults.enable_text_to_speech,
            speech_hotkey: "speech-hotkey-sentinel".to_string(),
            enable_speech_hotkey: !defaults.enable_speech_hotkey,
            translate_provider: "translate-sentinel".to_string(),
            speech_provider: "speech-sentinel".to_string(),
        };

        let path = std::env::temp_dir().join(format!(
            "tagent_test_roundtrip_all_fields_{}.conf",
            std::process::id()
        ));
        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        fs::write(&path, manager.create_ini_content(&config)).unwrap();
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
        assert_eq!(loaded.translate_hotkey, config.translate_hotkey);
        assert_eq!(loaded.enable_text_to_speech, config.enable_text_to_speech);
        assert_eq!(loaded.speech_hotkey, config.speech_hotkey);
        assert_eq!(loaded.enable_speech_hotkey, config.enable_speech_hotkey);
        assert_eq!(loaded.translate_provider, config.translate_provider);
        assert_eq!(loaded.speech_provider, config.speech_provider);
    }

    #[test]
    fn provider_error_message_lists_supported_values_for_unknown_provider() {
        let error = tagent::error::Error::UnknownProvider("bogus".to_string());
        assert_eq!(
            provider_error_message(&error, "SpeechProvider", &["google", "other"]),
            "unknown provider: bogus (supported values for SpeechProvider: google, other)"
        );
    }

    #[test]
    fn provider_error_message_leaves_other_errors_unchanged() {
        let error = tagent::error::Error::Network("down".to_string());
        assert_eq!(
            provider_error_message(&error, "SpeechProvider", &["google"]),
            error.to_string()
        );
    }

    /// The `Supported values:` comments in a generated config come from `tagent`'s own
    /// lists, so they can't go stale when a backend is added.
    #[test]
    fn generated_config_comments_list_the_providers_tagent_offers() {
        let manager = ConfigManager {
            config_path: "unused.conf".to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        let ini = manager.create_ini_content(&Config::default());
        for list in [
            tagent::providers::TRANSLATION_PROVIDERS,
            tagent::providers::DICTIONARY_PROVIDERS,
            tagent::providers::SPEECH_PROVIDERS,
        ] {
            let line = format!("; Supported values: {}\n", list.join(", "));
            assert!(ini.contains(&line), "missing {line:?} in generated config");
        }
        assert!(!ini.contains("{translate_providers}"));
    }

    #[test]
    fn test_load_config_explicit_false_is_respected() {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_explicit_false_speech_{}.conf",
            std::process::id()
        ));
        fs::write(&path, "[Speech]\nEnableTextToSpeech = false\n").unwrap();

        let manager = ConfigManager {
            config_path: path.to_str().unwrap().to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        manager.load_config().unwrap();

        assert!(!manager.get_config().enable_text_to_speech);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_parse_ini_duplicate_section_merges() {
        let manager = ConfigManager {
            config_path: "unused.conf".to_string(),
            config: Arc::new(Mutex::new(Config::default())),
            last_modified: Arc::new(Mutex::new(None)),
        };
        let sections = manager
            .parse_ini(
                "[Translation]\nSourceLanguage = Auto\n\n[Other]\nFoo = Bar\n\n[Translation]\nTargetLanguage = Russian\n",
            )
            .unwrap();

        let translation = &sections["Translation"];
        assert_eq!(translation.get("SourceLanguage"), Some(&"Auto".to_string()));
        assert_eq!(
            translation.get("TargetLanguage"),
            Some(&"Russian".to_string())
        );
        assert_eq!(sections["Other"].get("Foo"), Some(&"Bar".to_string()));
    }
}
