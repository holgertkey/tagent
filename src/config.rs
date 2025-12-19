use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Config {
    pub source_language: String,
    pub target_language: String,
    pub show_terminal_on_translate: bool,
    pub auto_hide_terminal_seconds: u64,
    pub show_dictionary: bool,
    pub copy_to_clipboard: bool,
    pub save_translation_history: bool,    // Новое поле
    pub history_file: String,              // Новое поле
    pub translation_prompt_color: String,  // Color for translation prompt
    pub dictionary_prompt_color: String,   // Color for dictionary prompt
    pub auto_prompt_color: String,         // Color for Auto prompt
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
            save_translation_history: false,        // По умолчанию отключено
            history_file: default_history,
            translation_prompt_color: "BrightYellow".to_string(),  // Default bright yellow for translation
            dictionary_prompt_color: "BrightYellow".to_string(),   // Default bright yellow for dictionary
            auto_prompt_color: "None".to_string(),                 // Default no color for Auto
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

; Color for Auto language prompt (e.g., "[Auto]: ")
; Supported values: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; BrightBlack, BrightRed, BrightGreen, BrightYellow, BrightBlue, BrightMagenta, BrightCyan, BrightWhite
; Use "None" to disable color
; Default: None (no color)
AutoPromptColor = {}

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
"#,
            config.source_language,
            config.target_language,
            config.copy_to_clipboard,
            config.show_dictionary,
            config.show_terminal_on_translate,
            config.auto_hide_terminal_seconds,
            config.translation_prompt_color,
            config.dictionary_prompt_color,
            config.auto_prompt_color,
            config.save_translation_history,
            config.history_file
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
        };

        if let Ok(mut config) = self.config.lock() {
            *config = new_config;
        }

        self.update_last_modified_time()?;
        
        Ok(())
    }

    /// Parse INI format content
    fn parse_ini(&self, content: &str) -> Result<HashMap<String, HashMap<String, String>>, Box<dyn Error>> {
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
                let section_name = line[1..line.len()-1].to_string();
                current_section = Some(section_name.clone());
                sections.insert(section_name, HashMap::new());
            }
            // Key-value pair
            else if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos+1..].trim().to_string();
                
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