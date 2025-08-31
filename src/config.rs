use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Config {
    pub source_language: String,
    pub target_language: String,
    pub show_terminal_on_translate: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source_language: "Auto".to_string(),
            target_language: "Russian".to_string(),
            show_terminal_on_translate: false,
        }
    }
}

pub struct ConfigManager {
    config_path: String,
    config: Arc<Mutex<Config>>,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
}

impl ConfigManager {
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
; 4. Press Ctrl+Q to exit the program
;
; Configuration changes take effect immediately (no restart required)

[Translation]
; Source language for translation
; Supported values: Auto, English, Russian, Spanish, French, German, etc.
; Use "Auto" for automatic language detection
SourceLanguage = {}

; Target language for translation  
; Supported values: Russian, English, Spanish, French, German, etc.
TargetLanguage = {}

[Interface]
; Show terminal window on top when translating
; Set to true to show terminal window during translation
; Set to false to keep terminal in background
ShowTerminalOnTranslate = {}
"#,
            config.source_language, 
            config.target_language,
            config.show_terminal_on_translate
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

        let show_terminal = parsed_config
            .get("Interface")
            .and_then(|section| section.get("ShowTerminalOnTranslate"))
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let new_config = Config {
            source_language: source_lang,
            target_language: target_lang,
            show_terminal_on_translate: show_terminal,
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
            println!("Configuration reloaded from file");
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
}