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
        println!("Text Translator v0.7.0 - CLI Mode");
        println!();
        println!("USAGE:");
        println!("  tagent [OPTIONS] <text>");
        println!();
        println!("ARGUMENTS:");
        println!("  <text>    Text to translate (use quotes for phrases with spaces)");
        println!();
        println!("OPTIONS:");
        println!("  -h, --help     Show this help message");
        println!("  -c, --config   Show current configuration");
        println!("  -v, --version  Show version information");
        println!("  -q             Exit (for interactive mode compatibility)");
        println!();
        println!("EXAMPLES:");
        println!("  tagent hello");
        println!("  tagent \"Hello world\"");
        println!("  tagent \"This is a longer phrase to translate\"");
        println!();
        println!("MODES:");
        println!("  Unified Mode (default): Run without arguments");
        println!("    - Interactive prompt in terminal + GUI hotkeys (Ctrl+Ctrl)");
        println!("    - Both methods work simultaneously");
        println!("    - Interactive commands: -h, -c, -v, -q, help, config, etc.");
        println!("  CLI Mode: Run 'tagent <text>' for one-time translation");
        println!();
        println!("CONFIGURATION:");
        println!("  Edit 'tagent.conf' to change translation settings:");
        println!("  - SourceLanguage: Source language (Auto, English, Russian, etc.)");
        println!("  - TargetLanguage: Target language (Russian, English, etc.)");
        println!("  - ShowDictionary: Enable dictionary lookup for single words");
        println!("  - CopyToClipboard: Copy results to clipboard");
        println!();
        println!("Run without arguments to start unified mode with interactive prompt and hotkeys.");
    }

    /// Show version information
    pub fn show_version() {
        println!("Text Translator v0.7.0");
        println!("Translation tool with unified GUI/Interactive interface and CLI mode");
        println!();
        println!("Features:");
        println!("- Unified mode: Interactive prompt + GUI hotkeys work simultaneously");
        println!("- CLI mode: Direct text translation from command line");
        println!("- Interactive commands: -h, -c, -v, -q and full names");
        println!("- Dictionary lookup for single words");
        println!("- Multi-language support");
        println!("- Configurable settings");
        println!();
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
            "-c" | "--config" => {
                self.show_config()
            },
            "-v" | "--version" => {
                Self::show_version();
                Ok(())
            },
            "-q" => {
                // Exit command for CLI mode (though it doesn't make much sense here)
                println!("Exiting...");
                Ok(())
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
            eprintln!("Error: Empty text provided");
            eprintln!("Usage: tagent <text to translate>");
            return Ok(());
        }

        // Load current configuration
        self.config_manager.check_and_reload().ok(); // Ignore errors, use defaults
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();

        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && self.is_single_word(text) {
            match self.translator.get_dictionary_entry_public(text, &source_code, &target_code).await {
                Ok(dictionary_info) => {
                    println!("{}", dictionary_info);
                    
                    if config.copy_to_clipboard {
                        if let Err(e) = self.copy_to_clipboard(&dictionary_info) {
                            println!("Clipboard error: {}", e);
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
        // Perform translation
        match self.translator.translate_text_public(text, source_code, target_code).await {
            Ok(translated_text) => {
                println!("{}", translated_text);
                
                if config.copy_to_clipboard {
                    self.copy_to_clipboard(&translated_text).ok(); // Ignore clipboard errors
                }
            }
            Err(e) => {
                eprintln!("Translation failed: {}", e);
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
}