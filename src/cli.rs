use crate::translator::Translator;
use crate::config::ConfigManager;
use std::error::Error;
use std::sync::Arc;

pub struct CliHandler {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
}

impl CliHandler {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let translator = Translator::new()?;
        let config_manager = Arc::new(ConfigManager::new("tagent.conf")?);
        
        Ok(Self {
            translator,
            config_manager,
        })
    }

    /// Display CLI help information
    pub fn show_help() {
        println!("=== Text Translator v0.6.0 - CLI Mode ===");
        println!();
        println!("USAGE:");
        println!("  tagent [OPTIONS] <text>");
        println!();
        println!("ARGUMENTS:");
        println!("  <text>    Text to translate (use quotes for phrases with spaces)");
        println!();
        println!("OPTIONS:");
        println!("  -h, --help     Show this help message");
        println!("  --version      Show version information");
        println!("  --config       Show current configuration");
        println!();
        println!("EXAMPLES:");
        println!("  tagent hello");
        println!("  tagent \"Hello world\"");
        println!("  tagent \"This is a longer phrase to translate\"");
        println!();
        println!("CONFIGURATION:");
        println!("  Edit 'tagent.conf' to change translation settings:");
        println!("  - SourceLanguage: Source language (Auto, English, Russian, etc.)");
        println!("  - TargetLanguage: Target language (Russian, English, etc.)");
        println!("  - ShowDictionary: Enable dictionary lookup for single words");
        println!("  - CopyToClipboard: Copy results to clipboard");
        println!();
        println!("Run without arguments to start GUI mode with hotkeys.");
    }

    /// Show version information
    pub fn show_version() {
        println!("Text Translator v0.6.0");
        println!("Translation tool with GUI hotkeys and CLI interface");
        println!();
        println!("Features:");
        println!("- GUI mode: Double-press Ctrl to translate selected text");
        println!("- CLI mode: Direct text translation from command line");
        println!("- Dictionary lookup for single words");
        println!("- Multi-language support");
        println!("- Configurable settings");
    }

    /// Show current configuration
    pub fn show_config(&self) -> Result<(), Box<dyn Error>> {
        // Reload config to get latest values
        self.config_manager.check_and_reload()?;
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();
        
        println!("=== Current Configuration ===");
        println!("Source Language: {} ({})", config.source_language, source_code);
        println!("Target Language: {} ({})", config.target_language, target_code);
        println!("Show Dictionary: {}", if config.show_dictionary { "Enabled" } else { "Disabled" });
        println!("Copy to Clipboard: {}", if config.copy_to_clipboard { "Enabled" } else { "Disabled" });
        println!("Show Terminal on Translate: {}", if config.show_terminal_on_translate { "Enabled" } else { "Disabled" });
        println!("Auto-hide Terminal (seconds): {}", 
            if config.auto_hide_terminal_seconds == 0 { 
                "Disabled".to_string() 
            } else { 
                config.auto_hide_terminal_seconds.to_string() 
            }
        );
        println!();
        println!("Config file: tagent.conf");
        println!("Edit this file to change settings (changes take effect immediately)");
        
        Ok(())
    }

    /// Process CLI arguments and determine action
    pub async fn process_args(&self, args: Vec<String>) -> Result<(), Box<dyn Error>> {
        if args.len() < 2 {
            println!("Error: No arguments provided");
            println!("Use --help for usage information");
            return Ok(());
        }

        let command = &args[1];
        
        match command.as_str() {
            "-h" | "--help" => {
                Self::show_help();
                Ok(())
            },
            "--version" => {
                Self::show_version();
                Ok(())
            },
            "--config" => {
                self.show_config()
            },
            _ => {
                // Treat as text to translate
                let text_to_translate = args[1..].join(" ");
                self.translate_text(&text_to_translate).await
            }
        }
    }

    /// Main translation function for CLI
    pub async fn translate_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        if text.trim().is_empty() {
            println!("Error: Empty text provided");
            println!("Usage: tagent <text to translate>");
            return Ok(());
        }

        // Load current configuration
        self.config_manager.check_and_reload().ok(); // Ignore errors, use defaults
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();

        println!("=== Text Translator v0.6.0 - CLI Mode ===");
        
        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && self.is_single_word(text) {
            println!("\n--- Dictionary lookup ---");
            
            match self.translator.get_dictionary_entry_public(text, &source_code, &target_code).await {
                Ok(dictionary_info) => {
                    println!("{}", dictionary_info);
                    
                    if config.copy_to_clipboard {
                        if let Err(e) = self.copy_to_clipboard(&dictionary_info) {
                            println!("Clipboard error: {}", e);
                        } else {
                            println!("\nDictionary entry copied to clipboard");
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    println!("Dictionary lookup failed: {}", e);
                    println!("Falling back to translation...");
                }
            }
        }

        // Regular translation
        self.perform_translation(text, &source_code, &target_code, &config).await
    }

    /// Perform translation and display results
    async fn perform_translation(&self, text: &str, source_code: &str, target_code: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        println!("\n--- Translation ---");
        
        // Show source language info
        let source_display = if source_code == "auto" {
            "Auto".to_string()
        } else {
            config.source_language.clone()
        };
        
        println!("From [{}]: {}", source_display, text);

        // Language validation warning for non-auto languages
        if source_code != "auto" && !self.is_expected_language(text, source_code) {
            println!("⚠️  Warning: Text may not be in {} language", config.source_language);
            println!("   Consider setting SourceLanguage = Auto in tagent.conf");
        }

        // Perform translation
        match self.translator.translate_text_public(text, source_code, target_code).await {
            Ok(translated_text) => {
                println!("To   [{}]: {}", config.target_language, translated_text);
                
                if config.copy_to_clipboard {
                    if let Err(e) = self.copy_to_clipboard(&translated_text) {
                        println!("Clipboard error: {}", e);
                    } else {
                        println!("\nTranslation copied to clipboard");
                    }
                }
            }
            Err(e) => {
                println!("Translation failed: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    /// Check if text is a single word
    fn is_single_word(&self, text: &str) -> bool {
        let cleaned = text.trim_matches(|c: char| !c.is_alphabetic());
        !cleaned.is_empty() && !cleaned.contains(' ') && 
        cleaned.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
    }

    /// Copy text to clipboard
    fn copy_to_clipboard(&self, text: &str) -> Result<(), Box<dyn Error>> {
        use crate::clipboard::ClipboardManager;
        let clipboard = ClipboardManager::new();
        clipboard.set_text(text)
    }

    /// Check if text appears to be in expected language
    fn is_expected_language(&self, text: &str, language_code: &str) -> bool {
        match language_code {
            "en" => self.is_english_text(text),
            "ru" => self.is_russian_text(text),
            _ => true, // For other languages, assume it's correct
        }
    }

    /// Check if text contains English characters
    fn is_english_text(&self, text: &str) -> bool {
        let english_chars = text.chars().filter(|c| c.is_alphabetic()).count();
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();
        
        if total_chars == 0 {
            return false;
        }

        let english_ratio = english_chars as f64 / total_chars as f64;
        english_ratio > 0.7 && text.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Check if text contains Russian characters
    fn is_russian_text(&self, text: &str) -> bool {
        let russian_chars = text
            .chars()
            .filter(|c| c.is_alphabetic() && (*c as u32) >= 0x0400 && (*c as u32) <= 0x04FF)
            .count();
        
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();
        
        if total_chars == 0 {
            return false;
        }

        let russian_ratio = russian_chars as f64 / total_chars as f64;
        russian_ratio > 0.3
    }
}