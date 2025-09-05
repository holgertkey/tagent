// interactive.rs
use crate::translator::Translator;
use crate::config::ConfigManager;
use std::error::Error;
use std::sync::Arc;
use std::io::{self, Write};

pub struct InteractiveMode {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
}

impl InteractiveMode {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let translator = Translator::new()?;
        let config_manager = Arc::new(ConfigManager::new("tagent.conf")?);
        
        Ok(Self {
            translator,
            config_manager,
        })
    }

    /// Start interactive translation mode
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        self.show_interactive_banner();
        
        loop {
            // Check if config file was modified and reload if necessary
            self.config_manager.check_and_reload().ok();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();
            
            // Show prompt
            print!("[{}]: ", config.source_language);
            io::stdout().flush()?;
            
            // Read user input
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let text = input.trim();
                    
                    // Handle special commands
                    match text {
                        "" => continue, // Skip empty lines
                        "exit" | "quit" | "q" => {
                            println!("Goodbye!");
                            break;
                        }
                        "help" | "?" => {
                            self.show_interactive_help();
                            continue;
                        }
                        "config" => {
                            self.show_current_config()?;
                            continue;
                        }
                        "clear" | "cls" => {
                            // Clear screen (Windows)
                            print!("\x1B[2J\x1B[1;1H");
                            io::stdout().flush()?;
                            self.show_interactive_banner();
                            continue;
                        }
                        _ => {
                            // Translate the text
                            self.translate_interactive_text(text, &source_code, &target_code, &config).await?;
                        }
                    }
                }
                Err(e) => {
                    println!("Input error: {}", e);
                    continue;
                }
            }
        }
        
        Ok(())
    }

    /// Show interactive mode banner
    fn show_interactive_banner(&self) {
        println!("=== Text Translator v0.7.0 - Interactive Mode ===");
        println!();
        println!("Type text to translate, or use these commands:");
        println!("  help    - Show help");
        println!("  config  - Show current configuration");
        println!("  clear   - Clear screen");
        println!("  exit    - Exit interactive mode");
        println!();
        println!("Note: GUI hotkeys (Ctrl+Ctrl) still work in background");
        println!("Press F12 to exit program completely");
        println!("=================================================");
        println!();
    }

    /// Show interactive help
    fn show_interactive_help(&self) {
        println!();
        println!("=== Interactive Mode Help ===");
        println!();
        println!("Commands:");
        println!("  help, ?     - Show this help");
        println!("  config      - Show current translation settings");
        println!("  clear, cls  - Clear screen");
        println!("  exit, quit, q - Exit interactive mode");
        println!();
        println!("Translation:");
        println!("- Type any text and press Enter to translate");
        println!("- Single words will show dictionary entries (if enabled)");
        println!("- Phrases will be translated normally");
        println!("- Empty line - skip (continue)");
        println!();
        println!("Features:");
        println!("- Same translation engine as GUI mode");
        println!("- Uses current configuration from tagent.conf");
        println!("- Configuration changes take effect immediately");
        println!("- Results are copied to clipboard (if enabled in config)");
        println!();
        println!("Background:");
        println!("- GUI hotkeys (Ctrl+Ctrl) still work while in interactive mode");
        println!("- Press F12 anywhere to exit program completely");
        println!("===============================");
        println!();
    }

    /// Show current configuration in interactive mode
    fn show_current_config(&self) -> Result<(), Box<dyn Error>> {
        self.config_manager.check_and_reload()?;
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();
        
        println!();
        println!("=== Current Configuration ===");
        println!("Source Language: {} ({})", config.source_language, source_code);
        println!("Target Language: {} ({})", config.target_language, target_code);
        println!("Show Dictionary: {}", if config.show_dictionary { "Enabled" } else { "Disabled" });
        println!("Copy to Clipboard: {}", if config.copy_to_clipboard { "Enabled" } else { "Disabled" });
        println!("============================");
        println!();
        
        Ok(())
    }

    /// Translate text in interactive mode
    async fn translate_interactive_text(&self, text: &str, source_code: &str, target_code: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && self.is_single_word(text) {
            match self.translator.get_dictionary_entry_public(text, source_code, target_code).await {
                Ok(dictionary_info) => {
                    println!("{}", dictionary_info);
                    
                    if config.copy_to_clipboard {
                        if let Err(e) = self.copy_to_clipboard(&dictionary_info) {
                            println!("Clipboard error: {}", e);
                        }
                    }
                    
                    println!(); // Add spacing
                    return Ok(());
                }
                Err(_) => {
                    // Fall back to regular translation
                }
            }
        }

        // Regular translation
        match self.translator.translate_text_public(text, source_code, target_code).await {
            Ok(translated_text) => {
                println!("{}", translated_text);
                
                if config.copy_to_clipboard {
                    self.copy_to_clipboard(&translated_text).ok();
                }
            }
            Err(e) => {
                println!("Translation error: {}", e);
            }
        }
        
        println!(); // Add spacing
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