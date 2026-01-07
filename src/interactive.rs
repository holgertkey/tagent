// interactive.rs
use crate::translator::Translator;
use crate::config::ConfigManager;
use crate::cli::CliHandler;
use crate::speech::SpeechManager;
use std::error::Error;
use std::sync::Arc;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;
use colored::Colorize;

pub struct InteractiveMode {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
    should_exit: Arc<AtomicBool>,
    speech_manager: SpeechManager,
}

impl InteractiveMode {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let translator = Translator::new()?;
        let config_path = ConfigManager::get_default_config_path()?;
        let config_manager = Arc::new(ConfigManager::new(config_path.to_string_lossy().as_ref())?);
        let should_exit = Arc::new(AtomicBool::new(false));
        let speech_manager = SpeechManager::new();

        Ok(Self {
            translator,
            config_manager,
            should_exit,
            speech_manager,
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
        
        loop {
            // Check if we should exit
            if self.should_exit.load(Ordering::Relaxed) {
                // println!("\nExiting program...");
                break;
            }

            // Check if config file was modified and reload if necessary
            self.config_manager.check_and_reload().ok();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();

            // Show colored prompt
            let prompt = format!("[{}]: ", config.source_language);

            // Choose color based on whether it's Auto or a specific language
            let prompt_color = if config.source_language.to_lowercase() == "auto" {
                &config.auto_prompt_color
            } else {
                &config.translation_prompt_color
            };

            if let Some(color) = ConfigManager::parse_color(prompt_color) {
                print!("{}", prompt.color(color));
            } else {
                print!("{}", prompt); // No color if None or parsing fails
            }
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
        // Check for speech commands with arguments
        if text.starts_with("/s ") || text.starts_with("/speech ") {
            let speech_text = if text.starts_with("/s ") {
                &text[3..]
            } else {
                &text[8..]
            };

            if speech_text.is_empty() {
                println!("Error: No text provided for speech");
                println!("Usage: /s <text to speak> or /speech <text to speak>");
                println!();
                return Ok(true);
            }

            if let Err(e) = self.speak_interactive_text(speech_text).await {
                println!("Speech error: {}", e);
            }
            println!(); // Add spacing
            return Ok(true);
        }

        match text {
            "" => return Ok(true), // Skip empty lines

            // Exit commands (only with slash)
            "/q" | "/quit" | "/exit" => {
                println!();
                println!("Goodbye!");
                self.should_exit.store(true, Ordering::SeqCst);
                return Ok(true);
            }

            // Help commands (only with slash)
            "/h" | "/help" | "/?" => {
                self.show_unified_help();
                return Ok(true);
            }

            // Config commands (only with slash)
            "/c" | "/config" => {
                if let Err(e) = self.show_current_config() {
                    println!("Config error: {}", e);
                }
                return Ok(true);
            }

            // Version commands (only with slash)
            "/v" | "/version" => {
                CliHandler::show_version();
                return Ok(true);
            }

            // Clear screen commands (only with slash)
            "/clear" | "/cls" => {
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush().map_err(|e| format!("IO error: {}", e))?;
                println!("=== Text Translator v{} ===", env!("CARGO_PKG_VERSION"));
                println!("Interactive and Hotkey modes active");
                println!("Type '/h' or '/help' for commands or just type text to translate");
                println!();
                return Ok(true);
            }

            _ => return Ok(false), // Not a command, should be translated
        }
    }

    /// Show unified mode help
    fn show_unified_help(&self) {
        println!();
        println!("=== Text Translator v{} - Unified Mode Help ===", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Translation Methods:");
        println!();
        println!("1. Interactive (This Terminal):");
        println!("   - Type any text and press Enter to translate");
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
        println!("Commands (must start with slash):");
        println!("  /h, /help, /?           - Show this help");
        println!("  /c, /config             - Show current translation settings");
        println!("  /v, /version            - Show version information");
        println!("  /s, /speech <text>      - Speak text using text-to-speech (press Esc to cancel)");
        println!("  /clear, /cls            - Clear screen");
        println!("  /q, /quit, /exit        - Exit program");
        println!();
        println!("Translation:");
        println!("- Same translation engine for both interactive and hotkey methods");

        // Show config file location
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("- Uses current configuration from: {}", config_path.display());
        } else {
            println!("- Uses current configuration from tagent.conf");
        }

        println!("- Configuration changes take effect immediately");
        println!("- Results copied to clipboard (if enabled in config)");
        println!("- Translation history saved automatically (if enabled in config)");
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

        println!("Translation Hotkey: {}", config.translate_hotkey);

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

        // Show config file location
        if let Ok(config_path) = ConfigManager::get_default_config_path() {
            println!("Config file: {}", config_path.display());
        } else {
            println!("Config file: tagent.conf");
        }
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
                    // Print colored dictionary label
                    let dict_label = "[Word]: ";
                    if let Some(color) = ConfigManager::parse_color(&config.dictionary_prompt_color) {
                        print!("{}", dict_label.color(color));
                    } else {
                        print!("{}", dict_label);
                    }
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
                // Print colored translation label
                let trans_label = format!("[{}]: ", config.target_language);
                if let Some(color) = ConfigManager::parse_color(&config.translation_prompt_color) {
                    print!("{}", trans_label.color(color));
                } else {
                    print!("{}", trans_label);
                }
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

    /// Speak text using text-to-speech in interactive mode
    async fn speak_interactive_text(&self, text: &str) -> Result<(), String> {
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

        if text.trim().is_empty() {
            return Err("Empty text provided".to_string());
        }

        // Load current configuration to get source language
        self.config_manager.check_and_reload().ok();
        let (source_code, _) = self.config_manager.get_language_codes();

        // If source language is "auto", use English by default for TTS
        let speech_lang = if source_code == "auto" {
            "en"
        } else {
            &source_code
        };

        let config = self.config_manager.get_config();

        // Show speech label with color
        let speech_label = "[Speech]: ";
        if let Some(color) = ConfigManager::parse_color(&config.translation_prompt_color) {
            print!("{}", speech_label.color(color));
        } else {
            print!("{}", speech_label);
        }
        println!("{}", text);

        // Create stop flag for cancellation
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Spawn task to monitor Esc key
        let esc_monitor = tokio::spawn(async move {
            loop {
                unsafe {
                    if GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000 != 0 {
                        stop_flag_clone.store(true, Ordering::Relaxed);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        // Start speech with cancellation support
        let speech_result = self.speech_manager.speak_text_with_cancel(
            text,
            speech_lang,
            stop_flag.clone()
        ).await;

        // Cancel the Esc monitor task
        esc_monitor.abort();

        match speech_result {
            Ok(_) => {
                if stop_flag.load(Ordering::Relaxed) {
                    println!("Speech cancelled by user (Esc)");
                } else {
                    println!("Speech completed successfully.");
                }
                Ok(())
            }
            Err(e) => {
                Err(format!("Speech error: {}", e))
            }
        }
    }
}