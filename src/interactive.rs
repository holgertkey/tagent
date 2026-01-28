// interactive.rs
use crate::cli::CliHandler;
use crate::config::ConfigManager;
use crate::speech::SpeechManager;
use crate::translator::Translator;
use chrono::{DateTime, Utc};
use colored::Colorize;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    fn save_translation_history(
        &self,
        original: &str,
        translated: &str,
        source_lang: &str,
        target_lang: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error>> {
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

            // Use source prompt color for source language
            if let Some(color) = ConfigManager::parse_color(&config.source_prompt_color) {
                print!("{}", prompt.color(color));
            } else {
                print!("{}", prompt); // No color if None or parsing fails
            }
            io::stdout()
                .flush()
                .map_err(|e| format!("IO error: {}", e))?;

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
                        if let Err(e) = self
                            .translate_interactive_text(text, &source_code, &target_code, &config)
                            .await
                        {
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
        // Check for language switch commands
        if text == "/l" || text == "/lang"
            || text.starts_with("/l ") || text.starts_with("/lang ")
        {
            let lang_args = if text == "/l" || text == "/lang" {
                ""
            } else if let Some(stripped) = text.strip_prefix("/l ") {
                stripped
            } else {
                text.strip_prefix("/lang ").unwrap_or("")
            };

            let parts: Vec<&str> = lang_args.split_whitespace().collect();
            if parts.is_empty() {
                // Swap source and target languages
                let config = self.config_manager.get_config();
                let mut source = config.source_language.clone();
                let target = config.target_language.clone();

                // If source is Auto, treat it as English before swapping
                if source.to_lowercase() == "auto" {
                    source = "English".to_string();
                }

                self.config_manager.set_languages(&target, &source);
                let new_source_code = ConfigManager::language_to_code(&target);
                let new_target_code = ConfigManager::language_to_code(&source);
                println!("Languages swapped: {} ({}) -> {} ({})", target, new_source_code, source, new_target_code);
                println!();
                return Ok(true);
            }

            let (raw_source, raw_target) = if parts.len() == 1 {
                ("Auto", parts[0])
            } else {
                (parts[0], parts[1])
            };

            // Normalize: accept both names ("English") and codes ("en")
            let source = ConfigManager::normalize_language(raw_source);
            let target = ConfigManager::normalize_language(raw_target);

            let source_code = ConfigManager::language_to_code(&source);
            let target_code = ConfigManager::language_to_code(&target);

            // Warn if language is completely unknown
            if source.to_lowercase() != "auto" && source_code == source.as_str() {
                println!("Warning: Unknown language '{}', using as language code", source);
            }
            if target_code == target.as_str() {
                println!("Warning: Unknown language '{}', using as language code", target);
            }

            self.config_manager.set_languages(&source, &target);
            println!("Languages set: {} ({}) -> {} ({})", source, source_code, target, target_code);
            println!();
            return Ok(true);
        }

        // Check for speech commands with arguments
        if text.starts_with("/s ") || text.starts_with("/speech ") {
            let speech_text = if let Some(stripped) = text.strip_prefix("/s ") {
                stripped
            } else {
                text.strip_prefix("/speech ").unwrap_or("")
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
            Ok(true)
        } else {
            match text {
                "" => Ok(true), // Skip empty lines

                // Exit commands (only with slash)
                "/q" | "/quit" | "/exit" => {
                    println!();
                    println!("Goodbye!");
                    self.should_exit.store(true, Ordering::SeqCst);
                    Ok(true)
                }

                // Help commands (only with slash)
                "/h" | "/help" | "/?" => {
                    self.show_unified_help();
                    Ok(true)
                }

                // Config commands (only with slash)
                "/c" | "/config" => {
                    if let Err(e) = self.show_current_config() {
                        println!("Config error: {}", e);
                    }
                    Ok(true)
                }

                // Save configuration to file
                "/save" => {
                    match self.config_manager.save_config() {
                        Ok(()) => println!("Configuration saved successfully."),
                        Err(e) => println!("Error saving configuration: {}", e),
                    }
                    println!();
                    Ok(true)
                }

                // Version commands (only with slash)
                "/v" | "/version" => {
                    CliHandler::show_version();
                    Ok(true)
                }

                // Clear screen commands (only with slash)
                "/clear" | "/cls" => {
                    print!("\x1B[2J\x1B[1;1H");
                    io::stdout()
                        .flush()
                        .map_err(|e| format!("IO error: {}", e))?;
                    println!("=== Text Translator v{} ===", env!("CARGO_PKG_VERSION"));
                    println!("Interactive and Hotkey modes active");
                    println!("Type '/h' or '/help' for commands or just type text to translate");
                    println!();
                    Ok(true)
                }

                _ => Ok(false), // Not a command, should be translated
            }
        }
    }

    /// Show unified mode help
    fn show_unified_help(&self) {
        ConfigManager::display_help();
    }

    /// Show current configuration in unified mode
    fn show_current_config(&self) -> Result<(), String> {
        self.config_manager
            .display_config()
            .map_err(|e| format!("Config display error: {}", e))
    }

    /// Translate text in interactive mode
    async fn translate_interactive_text(
        &self,
        text: &str,
        source_code: &str,
        target_code: &str,
        config: &crate::config::Config,
    ) -> Result<(), String> {
        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && self.is_single_word(text) {
            match self
                .translator
                .get_dictionary_entry_public(text, source_code, target_code)
                .await
            {
                Ok(dictionary_info) => {
                    // Print colored dictionary label
                    let dict_label = "[Word]: ";
                    if let Some(color) = ConfigManager::parse_color(&config.dictionary_prompt_color)
                    {
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
                    if let Err(e) = self.save_translation_history(
                        text,
                        &dictionary_info,
                        source_code,
                        target_code,
                        config,
                    ) {
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
        match self
            .translator
            .translate_text_public(text, source_code, target_code)
            .await
        {
            Ok(translated_text) => {
                // Print colored translation label
                let trans_label = format!("[{}]: ", config.target_language);
                if let Some(color) = ConfigManager::parse_color(&config.target_prompt_color) {
                    print!("{}", trans_label.color(color));
                } else {
                    print!("{}", trans_label);
                }
                println!("{}", translated_text);

                if config.copy_to_clipboard {
                    self.copy_to_clipboard(&translated_text).ok();
                }

                // Сохраняем перевод в историю
                if let Err(e) = self.save_translation_history(
                    text,
                    &translated_text,
                    source_code,
                    target_code,
                    config,
                ) {
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
        !cleaned.is_empty()
            && !cleaned.contains(' ')
            && cleaned
                .chars()
                .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
    }

    /// Copy text to clipboard
    fn copy_to_clipboard(&self, text: &str) -> Result<(), String> {
        use crate::clipboard::ClipboardManager;
        let clipboard = ClipboardManager::new();
        clipboard
            .set_text(text)
            .map_err(|e| format!("Clipboard error: {}", e))
    }

    /// Speak text using text-to-speech in interactive mode
    async fn speak_interactive_text(&self, text: &str) -> Result<(), String> {
        self.speech_manager
            .speak_text_full(text, &self.config_manager)
            .await
            .map(|_| ())
    }
}
