use crate::translator::Translator;
use crate::config::ConfigManager;
use crate::speech::SpeechManager;
use std::error::Error;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;
use std::io::Write;

pub struct CliHandler {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
    speech_manager: SpeechManager,
}

impl CliHandler {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let translator = Translator::new()?;
        let config_path = ConfigManager::get_default_config_path()?;
        let config_manager = Arc::new(ConfigManager::new(config_path.to_string_lossy().as_ref())?);
        let speech_manager = SpeechManager::new();

        Ok(Self {
            translator,
            config_manager,
            speech_manager,
        })
    }

    /// Save translation history to file (CLI version)
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

    /// Display CLI help information
    pub fn show_help() {
        ConfigManager::display_help();
    }

    /// Show version information
    pub fn show_version() {
        println!("Text Translator v{}", env!("CARGO_PKG_VERSION"));
        println!("Translation tool with unified GUI/Interactive interface and CLI mode");
        println!();
    }

    /// Show current configuration
    pub fn show_config(&self) -> Result<(), Box<dyn Error>> {
        self.config_manager.display_config()
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
            "-s" | "--speech" => {
                // Speak the following text
                if args.len() < 3 {
                    eprintln!("Error: No text provided for speech");
                    eprintln!("Usage: tagent -s \"text to speak\"");
                    return Ok(());
                }
                let text_to_speak = args[2..].join(" ");
                self.speak_text(&text_to_speak).await
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

                    // Сохраняем словарную статью в историю
                    if let Err(e) = self.save_translation_history(text, &dictionary_info, &source_code, &target_code, &config) {
                        println!("History save error: {}", e);
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

                // Сохраняем перевод в историю
                if let Err(e) = self.save_translation_history(text, &translated_text, source_code, target_code, config) {
                    println!("History save error: {}", e);
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

    /// Speak text using text-to-speech
    async fn speak_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

        if text.trim().is_empty() {
            eprintln!("Error: Empty text provided");
            eprintln!("Usage: tagent -s \"text to speak\"");
            return Ok(());
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
        let text_owned = text.to_string();
        let speech_lang_owned = speech_lang.to_string();
        let stop_flag_for_speech = stop_flag.clone();

        let speech_result = self.speech_manager.speak_text_with_cancel(
            &text_owned,
            &speech_lang_owned,
            stop_flag_for_speech
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
                eprintln!("Speech error: {}", e);
                Err(Box::new(e))
            }
        }
    }
}