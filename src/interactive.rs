// interactive.rs
use crate::translator::Translator;
use crate::config::ConfigManager;
use crate::cli::CliHandler;
use std::error::Error;
use std::sync::Arc;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;

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

    /// Save translation history to file (Interactive version)
    fn save_translation_history(&self, original: &str, translated: &str, source_lang: &str, target_lang: &str, config: &crate::config::Config) -> Result<(), Box<dyn Error>> {
        if !config.save_translation_history {
            return Ok(()); // История отключена
        }

        let timestamp: DateTime<Utc> = Utc::now();
        let formatted_time = timestamp.format("%Y-%m-%d %H:%M:%S UTC");
        
        let entry = format!(
            "[{}] {} -> {}\nIN:  {}\nOUT: {}\n---\n\n",
            formatted_time, source_lang, target_lang, original, translated
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.history_file)?;

        file.write_all(entry.as_bytes())?;
        file.flush()?; // Принудительно записываем на диск
        
        Ok(())
    }

    /// Start interactive translation mode (unified with GUI)
    pub async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("Ready for interactive translation and hotkey commands...");
        println!();
        
        loop {
            // Check if we should exit
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
            
            // Read user input
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let text = input.trim();
                    
                    // Handle commands first
                    if self.handle_command(text).await? {
                        continue; // Command was handled, continue to next iteration
                    }
                    
                    // If not a command, try to translate the text
                    if !text.is_empty() {
                        if let Err(e) = self.translate_interactive_text(text, &source_code, &target_code, &config).await {
                            println!("Translation error: {}", e);
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

    /// Handle interactive commands, returns true if command was processed
    async fn handle_command(&self, text: &str) -> Result<bool, String> {
        match text {
            "" => return Ok(true), // Skip empty lines
            
            // Exit commands
            "exit" | "quit" | "q" | "-q" => {
                println!("Goodbye!");
                self.should_exit.store(true, Ordering::SeqCst);
                return Ok(true);
            }
            
            // Help commands
            "help" | "?" | "-h" | "--help" => {
                self.show_unified_help();
                return Ok(true);
            }
            
            // Config commands
            "config" | "-c" | "--config" => {
                if let Err(e) = self.show_current_config() {
                    println!("Config error: {}", e);
                }
                return Ok(true);
            }
            
            // Version commands
            "version" | "-v" | "--version" => {
                CliHandler::show_version();
                return Ok(true);
            }
            
            // Clear screen commands
            "clear" | "cls" => {
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush().map_err(|e| format!("IO error: {}", e))?;
                println!("=== Text Translator v0.8.0 ===");
                println!("Interactive and Hotkey modes active");
                println!("Type 'help' for commands or just type text to translate");
                println!();
                return Ok(true);
            }
            
            _ => return Ok(false), // Not a command, should be translated
        }
    }

    /// Show unified mode help
    fn show_unified_help(&self) {
        println!();
        println!("=== Text Translator v0.8.0 - Unified Mode Help ===");
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
        println!("  help, ?, -h, --help     - Show this help");
        println!("  config, -c, --config    - Show current translation settings");
        println!("  version, -v, --version  - Show version information");
        println!("  clear, cls              - Clear screen");
        println!("  exit, quit, q, -q       - Exit program");
        println!();
        println!("Translation:");
        println!("- Any text not recognized as a command will be translated");
        println!("- Same translation engine for both interactive and hotkey methods");
        println!("- Uses current configuration from tagent.conf");
        println!("- Configuration changes take effect immediately");
        println!("- Results copied to clipboard (if enabled in config)");
        println!("- Translation history saved automatically (if enabled in config)");
        println!();
        println!("Exit Program:");
        println!("- Type 'exit', 'quit', 'q', or '-q' in this terminal");
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
        println!("Save Translation History: {}", if config.save_translation_history { "Enabled" } else { "Disabled" });
        println!("History File: {}", config.history_file);
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

                    // Сохраняем словарную статью в историю
                    if let Err(e) = self.save_translation_history(text, &dictionary_info, source_code, target_code, config) {
                        println!("History save error: {}", e);
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

                // Сохраняем перевод в историю
                if let Err(e) = self.save_translation_history(text, &translated_text, source_code, target_code, config) {
                    println!("History save error: {}", e);
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