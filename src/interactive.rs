// interactive.rs
use crate::translator::Translator;
use crate::config::ConfigManager;
use std::error::Error;
use std::sync::Arc;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct InteractiveMode {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
    should_exit: Arc<AtomicBool>,
}

impl InteractiveMode {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let translator = Translator::new()?;
        let config_manager = Arc::new(ConfigManager::new("tagent.conf")?);
        let should_exit = Arc::new(AtomicBool::new(false));
        
        Ok(Self {
            translator,
            config_manager,
            should_exit,
        })
    }

    pub fn get_exit_flag(&self) -> Arc<AtomicBool> {
        self.should_exit.clone()
    }

    /// Start interactive translation mode (unified with GUI)
    pub async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("Ready for interactive translation and hotkey commands...");
        println!();
        
        loop {
            // Check if we should exit (F12 was pressed)
            if self.should_exit.load(Ordering::Relaxed) {
                println!("\nExiting program...");
                break;
            }

            // Check if config file was modified and reload if necessary
            self.config_manager.check_and_reload().ok();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();
            
            // Show prompt
            print!("[{}]: ", config.source_language);
            io::stdout().flush().map_err(|e| format!("IO error: {}", e))?;
            
            // Read user input - this will block until user enters something
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let text = input.trim();
                    
                    // Handle special commands
                    match text {
                        "" => continue, // Skip empty lines
                        "exit" | "quit" | "q" => {
                            println!("Goodbye!");
                            self.should_exit.store(true, Ordering::SeqCst);
                            break;
                        }
                        "help" | "?" => {
                            self.show_unified_help();
                            continue;
                        }
                        "config" => {
                            if let Err(e) = self.show_current_config() {
                                println!("Config error: {}", e);
                            }
                            continue;
                        }
                        "clear" | "cls" => {
                            // Clear screen (Windows)
                            print!("\x1B[2J\x1B[1;1H");
                            io::stdout().flush().map_err(|e| format!("IO error: {}", e))?;
                            println!("=== Text Translator v0.7.0 ===");
                            println!("Interactive and Hotkey modes active");
                            println!("Type 'help' for commands or just type text to translate");
                            println!();
                            continue;
                        }
                        _ => {
                            // Translate the text
                            if let Err(e) = self.translate_interactive_text(text, &source_code, &target_code, &config).await {
                                println!("Translation error: {}", e);
                            }
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

    /// Show unified mode help
    fn show_unified_help(&self) {
        println!();
        println!("=== Text Translator v0.7.0 - Unified Mode Help ===");
        println!();
        println!("Translation Methods:");
        println!();
        println!("1. Interactive (This Terminal):");
        println!("   - Type text and press Enter to translate");
        println!("   - Single words show dictionary entries (if enabled)");
        println!("   - Phrases show translations");
        println!("   - Empty line = skip/continue");
        println!();
        println!("2. Hotkeys (Any Application):");
        println!("   - Select text anywhere in Windows");
        println!("   - Double-press Ctrl quickly (Ctrl + Ctrl)");
        println!("   - Result copied to clipboard automatically");
        println!("   - Prompt returns automatically after hotkey translation");
        println!();
        println!("Interactive Commands:");
        println!("  help, ?       - Show this help");
        println!("  config        - Show current translation settings");
        println!("  clear, cls    - Clear screen");
        println!("  exit, quit, q - Exit program");
        println!();
        println!("Features:");
        println!("- Same translation engine for both methods");
        println!("- Uses current configuration from tagent.conf");
        println!("- Configuration changes take effect immediately");
        println!("- Results copied to clipboard (if enabled in config)");
        println!();
        println!("Exit Program:");
        println!("- Type 'exit' in this terminal, OR");
        println!("- Press F12 anywhere in Windows");
        println!("===============================================");
        println!();
    }

    /// Show current configuration in unified mode
    fn show_current_config(&self) -> Result<(), String> {
        self.config_manager.check_and_reload().map_err(|e| format!("Config reload error: {}", e))?;
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();
        
        println!();
        println!("=== Current Configuration ===");
        println!("Source Language: {} ({})", config.source_language, source_code);
        println!("Target Language: {} ({})", config.target_language, target_code);
        println!("Show Dictionary: {}", if config.show_dictionary { "Enabled" } else { "Disabled" });
        println!("Copy to Clipboard: {}", if config.copy_to_clipboard { "Enabled" } else { "Disabled" });
        println!("Show Terminal on Hotkey: {}", if config.show_terminal_on_translate { "Enabled" } else { "Disabled" });
        println!("Auto-hide Terminal: {} seconds", 
            if config.auto_hide_terminal_seconds == 0 { 
                "Disabled".to_string() 
            } else { 
                config.auto_hide_terminal_seconds.to_string() 
            }
        );
        println!("Config file: tagent.conf");
        println!("============================");
        println!();
        
        Ok(())
    }

    /// Translate text in interactive mode
    async fn translate_interactive_text(&self, text: &str, source_code: &str, target_code: &str, config: &crate::config::Config) -> Result<(), String> {
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
                return Err(format!("Translation failed: {}", e));
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
    fn copy_to_clipboard(&self, text: &str) -> Result<(), String> {
        use crate::clipboard::ClipboardManager;
        let clipboard = ClipboardManager::new();
        clipboard.set_text(text).map_err(|e| format!("Clipboard error: {}", e))
    }
}