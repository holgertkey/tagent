use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

#[derive(Debug, Clone)]
pub struct Config {
    pub source_language: String,
    pub target_language: String,
    pub show_terminal_on_translate: bool,
    pub auto_hide_terminal_seconds: u64,
    pub show_dictionary: bool,
    pub copy_to_clipboard: bool,
    pub save_translation_history: bool,   // Новое поле
    pub history_file: String,             // Новое поле
    pub translation_prompt_color: String, // Color for translation prompt
    pub dictionary_prompt_color: String,  // Color for dictionary prompt
    pub auto_prompt_color: String,        // Color for Auto prompt
    pub translate_hotkey: String,         // Translation hotkey (e.g., "Ctrl+Ctrl", "Alt+Q", "F9")
    pub enable_text_to_speech: bool,      // Enable text-to-speech functionality
    pub speech_hotkey: String,            // Hotkey for speech (e.g., "Alt+E")
    pub enable_speech_hotkey: bool,       // Enable/disable speech hotkey
    pub translate_provider: String,       // Translation provider (e.g., "google")
}

impl Default for Config {
    fn default() -> Self {
        // Try to get AppData path for history file, fallback to current directory
        let default_history = if let Some(config_dir) = dirs::config_dir() {
            let history_path = config_dir.join("Tagent").join("translation_history.txt");
            history_path.to_string_lossy().to_string()
        } else {
            "translation_history.txt".to_string()
        };

        Self {
            source_language: "Auto".to_string(),
            target_language: "Russian".to_string(),
            show_terminal_on_translate: true,
            auto_hide_terminal_seconds: 5,
            show_dictionary: true,
            copy_to_clipboard: true,
            save_translation_history: false, // По умолчанию отключено
            history_file: default_history,
            translation_prompt_color: "BrightYellow".to_string(), // Default bright yellow for translation
            dictionary_prompt_color: "BrightYellow".to_string(), // Default bright yellow for dictionary
            auto_prompt_color: "None".to_string(),               // Default no color for Auto
            translate_hotkey: "Ctrl+Ctrl".to_string(),           // Default translation hotkey
            enable_text_to_speech: true,                         // TTS enabled by default
            speech_hotkey: "Alt+E".to_string(),                  // Default speech hotkey
            enable_speech_hotkey: true,                          // Enable speech hotkey by default
            translate_provider: "google".to_string(),            // Default translation provider
        }
    }
}

pub struct ConfigManager {
    config_path: String,
    config: Arc<Mutex<Config>>,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
}

impl ConfigManager {
    /// Get default configuration file path in AppData\Roaming\Tagent
    pub fn get_default_config_path() -> Result<PathBuf, Box<dyn Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("Tagent");

        // Create directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(config_dir.join("tagent.conf"))
    }

    pub fn new(config_path: &str) -> Result<Self, Box<dyn Error>> {
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
    fn load_or_create_config(&self) -> Result<(), Box<dyn Error>> {
        if Path::new(&self.config_path).exists() {
            self.load_config()?;
        } else {
            self.create_default_config()?;
        }
        Ok(())
    }

    /// Create default configuration file
    fn create_default_config(&self) -> Result<(), Box<dyn Error>> {
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
; 2. Double-press Ctrl key quickly (Ctrl + Ctrl)
; 3. Translation will be copied to clipboard
; 4. Press F12 to exit the program
;
; Configuration changes take effect immediately (no restart required)

[Provider]
; Translation service provider
; Supported values: google (more providers will be added in the future)
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

; Automatically copy translation result to clipboard
; Set to true to automatically copy result to clipboard after translation
; Set to false to display result only (without copying to clipboard)
; When enabled, you can paste the result anywhere with Ctrl+V
CopyToClipboard = {}

[Dictionary]
; Show dictionary entry for single words instead of simple translation
; Set to true to show detailed word information (definitions, part of speech, examples)
; Set to false to always use simple translation
; This feature works best with English words
ShowDictionary = {}

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

[Colors]
; Color for Auto language prompt (e.g., "[Auto]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: None (no color)
AutoPromptColor = {}

; Color for translation prompt (e.g., "[Russian]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: BrightYellow
TranslationPromptColor = {}

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
;   - Double-press: Ctrl+Ctrl (default), F8+F8, Shift+Shift, Alt+Alt, etc.
; Examples:
;   TranslateHotkey = Ctrl+Ctrl
;   TranslateHotkey = Alt+Q
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
;   - Modifier combinations: Alt+E, Ctrl+Shift+S, etc.
;   - Double-press: Alt+Alt, Shift+Shift, etc.
; Examples:
;   SpeechHotkey = Alt+E
;   SpeechHotkey = F10
;   SpeechHotkey = Ctrl+Shift+S
; Note: Hotkey changes require application restart to take effect
SpeechHotkey = {}

; Enable or disable the speech hotkey
; Set to true to enable the speech hotkey
; Set to false to disable speech hotkey
EnableSpeechHotkey = {}
"#,
            config.translate_provider,
            config.source_language,
            config.target_language,
            config.copy_to_clipboard,
            config.show_dictionary,
            config.show_terminal_on_translate,
            config.auto_hide_terminal_seconds,
            config.auto_prompt_color,
            config.translation_prompt_color,
            config.dictionary_prompt_color,
            config.save_translation_history,
            config.history_file,
            config.translate_hotkey,
            config.enable_text_to_speech,
            config.speech_hotkey,
            config.enable_speech_hotkey
        )
    }

    /// Load configuration from INI file
    fn load_config(&self) -> Result<(), Box<dyn Error>> {
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

        let copy_to_clipboard = parsed_config
            .get("Translation")
            .and_then(|section| section.get("CopyToClipboard"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let show_dictionary = parsed_config
            .get("Dictionary")
            .and_then(|section| section.get("ShowDictionary"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let show_terminal = parsed_config
            .get("Interface")
            .and_then(|section| section.get("ShowTerminalOnTranslate"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);

        let auto_hide_seconds = parsed_config
            .get("Interface")
            .and_then(|section| section.get("AutoHideTerminalSeconds"))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);

        // Новые поля для истории
        let save_translation_history = parsed_config
            .get("History")
            .and_then(|section| section.get("SaveTranslationHistory"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false); // По умолчанию false

        let history_file = parsed_config
            .get("History")
            .and_then(|section| section.get("HistoryFile"))
            .cloned()
            .unwrap_or_else(|| "translation_history.txt".to_string());

        // Color settings
        let translation_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| section.get("TranslationPromptColor"))
            .cloned()
            .unwrap_or_else(|| "BrightYellow".to_string());

        let dictionary_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| section.get("DictionaryPromptColor"))
            .cloned()
            .unwrap_or_else(|| "BrightYellow".to_string());

        let auto_prompt_color = parsed_config
            .get("Colors")
            .and_then(|section| section.get("AutoPromptColor"))
            .cloned()
            .unwrap_or_else(|| "None".to_string());

        // Hotkey settings
        let translate_hotkey = parsed_config
            .get("Hotkeys")
            .and_then(|section| section.get("TranslateHotkey"))
            .cloned()
            .unwrap_or_else(|| "Ctrl+Ctrl".to_string());

        // Speech settings
        let enable_text_to_speech = parsed_config
            .get("Speech")
            .and_then(|section| section.get("EnableTextToSpeech"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let speech_hotkey = parsed_config
            .get("Speech")
            .and_then(|section| section.get("SpeechHotkey"))
            .cloned()
            .unwrap_or_else(|| "Alt+E".to_string());

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

        let new_config = Config {
            source_language: source_lang,
            target_language: target_lang,
            copy_to_clipboard,
            show_dictionary,
            show_terminal_on_translate: show_terminal,
            auto_hide_terminal_seconds: auto_hide_seconds,
            save_translation_history,
            history_file,
            translation_prompt_color,
            dictionary_prompt_color,
            auto_prompt_color,
            translate_hotkey,
            enable_text_to_speech,
            speech_hotkey,
            enable_speech_hotkey,
            translate_provider,
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
    ) -> Result<HashMap<String, HashMap<String, String>>, Box<dyn Error>> {
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
                sections.insert(section_name, HashMap::new());
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

    /// Get current configuration
    pub fn get_config(&self) -> Config {
        self.config.lock().unwrap().clone()
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
        println!("2. CLI Mode: Run 'tagent <text>' for one-time translation");
        println!();

        println!("USAGE:");
        println!("  tagent [OPTIONS] [text]");
        println!();

        println!("ARGUMENTS:");
        println!("  <text>    Text to translate (use quotes for phrases with spaces)");
        println!();

        println!("OPTIONS:");
        println!("  -h, --help     Show this help message");
        println!("  -c, --config   Show current configuration");
        println!("  -v, --version  Show version information");
        println!("  -s, --speech   Speak the following text using text-to-speech");
        println!();

        println!("EXAMPLES:");
        println!("  tagent                           Start unified mode (interactive + hotkeys)");
        println!("  tagent hello                     Translate 'hello' (CLI mode)");
        println!("  tagent \"Hello world\"             Translate phrase (CLI mode)");
        println!("  tagent -s \"Hello world\"          Speak text using TTS");
        println!("  tagent --config                  Show configuration");
        println!();

        println!("UNIFIED MODE - TRANSLATION METHODS:");
        println!();
        println!("1. Interactive Terminal:");
        println!("   - Type any text and press Enter to translate");
        println!("   - Single words show dictionary entries (if enabled)");
        println!("   - Phrases show translations");
        println!("   - Empty line = skip/continue");
        println!();

        println!("2. GUI Hotkeys (Any Application):");
        println!("   - Select text anywhere in Windows");
        println!("   - Press configured hotkey (default: Ctrl+Ctrl)");
        println!("   - Result copied to clipboard automatically");
        println!("   - Configure hotkeys in tagent.conf [Hotkeys] section");
        println!();

        println!("INTERACTIVE COMMANDS (must start with slash):");
        println!("  /h, /help, /?           - Show this help");
        println!("  /c, /config             - Show current configuration");
        println!("  /v, /version            - Show version information");
        println!(
            "  /s, /speech <text>      - Speak text using text-to-speech (press Esc to cancel)"
        );
        println!("  /clear, /cls            - Clear screen");
        println!("  /q, /quit, /exit        - Exit program");
        println!();

        println!("CONFIGURATION:");
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("  Config file: {}", config_path.display());
        } else {
            println!("  Config file: tagent.conf (typically in %APPDATA%\\Tagent\\)");
        }
        println!();
        println!("  Edit 'tagent.conf' to change translation settings:");
        println!("  - SourceLanguage: Source language (Auto, English, Russian, etc.)");
        println!("  - TargetLanguage: Target language (Russian, English, etc.)");
        println!("  - ShowDictionary: Enable dictionary lookup for single words");
        println!("  - CopyToClipboard: Copy results to clipboard");
        println!("  - TranslateHotkey: Custom hotkey (Ctrl+Ctrl, Alt+Q, F9, etc.)");
        println!("  - SpeechHotkey: Hotkey for text-to-speech (Alt+E, F10, etc.)");
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
        println!("Run 'tagent --config' to see current settings.");
        println!("===============================================");
        println!();
    }

    /// Display current configuration (unified for CLI and Interactive modes)
    pub fn display_config(&self) -> Result<(), Box<dyn Error>> {
        // Reload config to get latest values
        self.check_and_reload()?;
        let config = self.get_config();
        let (source_code, target_code) = self.get_language_codes();

        println!();
        println!("=== Current Configuration ===");
        println!("Translation Provider: {}", config.translate_provider);
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
            println!("Config file: tagent.conf");
        }
        println!("Edit this file to change settings (changes take effect immediately)");
        println!("============================");
        println!();

        Ok(())
    }

    /// Check if config file was modified and reload if necessary
    pub fn check_and_reload(&self) -> Result<bool, Box<dyn Error>> {
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
    fn update_last_modified_time(&self) -> Result<(), Box<dyn Error>> {
        if Path::new(&self.config_path).exists() {
            let metadata = fs::metadata(&self.config_path)?;
            let modified = metadata.modified()?;

            if let Ok(mut last_modified) = self.last_modified.lock() {
                *last_modified = Some(modified);
            }
        }
        Ok(())
    }

    /// Convert language name to Google Translate language code
    pub fn language_to_code(language: &str) -> &str {
        match language.to_lowercase().as_str() {
            "auto" => "auto",
            "english" => "en",
            "russian" => "ru",
            "spanish" => "es",
            "french" => "fr",
            "german" => "de",
            "chinese" => "zh",
            "japanese" => "ja",
            "korean" => "ko",
            "italian" => "it",
            "portuguese" => "pt",
            "dutch" => "nl",
            "polish" => "pl",
            "turkish" => "tr",
            "arabic" => "ar",
            "hindi" => "hi",
            _ => language, // Return as-is if not found (might be a code already)
        }
    }

    /// Get language codes for translation
    pub fn get_language_codes(&self) -> (String, String) {
        let config = self.get_config();
        let source_code = Self::language_to_code(&config.source_language);
        let target_code = Self::language_to_code(&config.target_language);

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

// Hotkey configuration types and parser
#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyType {
    SingleKey {
        vk_code: u32,
    },
    ModifierCombo {
        modifiers: Vec<u32>,
        key: u32,
    },
    DoublePress {
        vk_code: u32,
        min_interval_ms: u64,
        max_interval_ms: u64,
    },
}

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
                let vk_code = Self::key_name_to_vk(parts[0])?;
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

            let key = Self::key_name_to_vk(parts.last().unwrap())?;
            let modifiers: Result<Vec<u32>, String> = parts[..parts.len() - 1]
                .iter()
                .map(|m| Self::key_name_to_vk(m))
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

    /// Convert key name to Windows virtual key code
    fn key_name_to_vk(key_name: &str) -> Result<u32, String> {
        let key_lower = key_name.to_lowercase();

        match key_lower.as_str() {
            // Modifiers
            "ctrl" | "control" => Ok(VK_CONTROL.0 as u32),
            "lctrl" | "lcontrol" => Ok(VK_LCONTROL.0 as u32),
            "rctrl" | "rcontrol" => Ok(VK_RCONTROL.0 as u32),
            "alt" => Ok(VK_MENU.0 as u32),
            "lalt" => Ok(VK_LMENU.0 as u32),
            "ralt" => Ok(VK_RMENU.0 as u32),
            "shift" => Ok(VK_SHIFT.0 as u32),
            "lshift" => Ok(VK_LSHIFT.0 as u32),
            "rshift" => Ok(VK_RSHIFT.0 as u32),
            "win" | "windows" => Ok(VK_LWIN.0 as u32),
            "lwin" => Ok(VK_LWIN.0 as u32),
            "rwin" => Ok(VK_RWIN.0 as u32),

            // Function keys
            "f1" => Ok(VK_F1.0 as u32),
            "f2" => Ok(VK_F2.0 as u32),
            "f3" => Ok(VK_F3.0 as u32),
            "f4" => Ok(VK_F4.0 as u32),
            "f5" => Ok(VK_F5.0 as u32),
            "f6" => Ok(VK_F6.0 as u32),
            "f7" => Ok(VK_F7.0 as u32),
            "f8" => Ok(VK_F8.0 as u32),
            "f9" => Ok(VK_F9.0 as u32),
            "f10" => Ok(VK_F10.0 as u32),
            "f11" => Ok(VK_F11.0 as u32),
            "f12" => Ok(VK_F12.0 as u32),

            // Special keys
            "space" => Ok(VK_SPACE.0 as u32),
            "tab" => Ok(VK_TAB.0 as u32),
            "enter" | "return" => Ok(VK_RETURN.0 as u32),
            "esc" | "escape" => Ok(VK_ESCAPE.0 as u32),
            "backspace" => Ok(VK_BACK.0 as u32),
            "delete" | "del" => Ok(VK_DELETE.0 as u32),
            "insert" | "ins" => Ok(VK_INSERT.0 as u32),
            "home" => Ok(VK_HOME.0 as u32),
            "end" => Ok(VK_END.0 as u32),
            "pageup" | "pgup" => Ok(VK_PRIOR.0 as u32),
            "pagedown" | "pgdn" => Ok(VK_NEXT.0 as u32),

            // Arrow keys
            "left" => Ok(VK_LEFT.0 as u32),
            "right" => Ok(VK_RIGHT.0 as u32),
            "up" => Ok(VK_UP.0 as u32),
            "down" => Ok(VK_DOWN.0 as u32),

            // Letters (A-Z)
            s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
                let ch = s.chars().next().unwrap().to_ascii_uppercase();
                Ok(ch as u32)
            }

            // Numbers (0-9)
            s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
                let ch = s.chars().next().unwrap();
                Ok(ch as u32)
            }

            _ => Err(format!("Unknown key name: {}", key_name)),
        }
    }

    /// Validate that the hotkey doesn't conflict with critical system shortcuts
    pub fn validate_hotkey(hotkey: &HotkeyType) -> Result<(), String> {
        match hotkey {
            HotkeyType::SingleKey { vk_code } => {
                // Only allow F1-F12 as single keys
                const VK_F1: u32 = 112;
                const VK_F12: u32 = 123;
                if *vk_code < VK_F1 || *vk_code > VK_F12 {
                    return Err("Single keys are only allowed for F1-F12. For other keys like Space, Tab, etc., use modifier combinations (e.g., Alt+Space, Ctrl+T)".to_string());
                }
            }
            HotkeyType::ModifierCombo { modifiers, key } => {
                // Forbid Shift-only combinations (Shift+Key interferes with text input)
                // Allow multi-modifier combinations (Ctrl+Shift+Key, Alt+Shift+Key, etc.)
                let has_shift = modifiers.contains(&(VK_SHIFT.0 as u32));
                let only_shift = modifiers.len() == 1 && has_shift;

                if only_shift {
                    return Err("Shift+Key combinations are not allowed (interferes with text input). Use multi-modifier combinations like Ctrl+Shift+T or Alt+Shift+Space instead.".to_string());
                }

                // Warn about common system shortcuts
                let has_ctrl = modifiers.iter().any(|&m| {
                    m == VK_CONTROL.0 as u32
                        || m == VK_LCONTROL.0 as u32
                        || m == VK_RCONTROL.0 as u32
                });
                let has_alt = modifiers.iter().any(|&m| {
                    m == VK_MENU.0 as u32 || m == VK_LMENU.0 as u32 || m == VK_RMENU.0 as u32
                });
                let has_win = modifiers
                    .iter()
                    .any(|&m| m == VK_LWIN.0 as u32 || m == VK_RWIN.0 as u32);

                // Block dangerous combinations
                if has_ctrl && has_alt && *key == VK_DELETE.0 as u32 {
                    return Err("Ctrl+Alt+Delete is reserved by the system".to_string());
                }

                if has_win && *key == 'L' as u32 {
                    return Err("Win+L (lock screen) is reserved by the system".to_string());
                }

                // Warnings for common shortcuts (don't block, just warn in logs)
                if has_alt && *key == VK_F4.0 as u32 {
                    eprintln!("Warning: Alt+F4 may close windows");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
