use crate::config::ConfigManager;
use crate::speech::SpeechManager;
use crate::translator::Translator;
use chrono::{DateTime, Utc};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;

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
            }
            "-c" | "--config" => self.show_config(),
            "-v" | "--version" => {
                Self::show_version();
                Ok(())
            }
            "-l" | "--lang" => {
                // Set languages and optionally translate
                if args.len() < 3 {
                    eprintln!("Error: No language provided");
                    eprintln!("Usage: tagent -l <target> [text]");
                    eprintln!("       tagent -l <source> <target> [text]");
                    return Ok(());
                }

                // Determine if second arg is a known language (name or code)
                let arg2 = &args[2];
                let arg2_norm = ConfigManager::normalize_language(arg2);
                let arg2_code = ConfigManager::language_to_code(&arg2_norm);
                let arg2_is_lang = arg2_code != arg2_norm.as_str() || arg2.to_lowercase() == "auto";

                let (source, target, text_start_idx) = if args.len() >= 4 {
                    let arg3 = &args[3];
                    let arg3_norm = ConfigManager::normalize_language(arg3);
                    let arg3_code = ConfigManager::language_to_code(&arg3_norm);
                    let arg3_is_lang = arg3_code != arg3_norm.as_str() || arg3.to_lowercase() == "auto";

                    if arg2_is_lang && arg3_is_lang {
                        // -l Source Target [text...]
                        (arg2_norm, arg3_norm, 4)
                    } else {
                        // -l Target text...
                        ("Auto".to_string(), arg2_norm, 3)
                    }
                } else {
                    // -l Target (no text)
                    ("Auto".to_string(), arg2_norm, 3)
                };

                self.config_manager.set_languages(&source, &target);

                if args.len() > text_start_idx {
                    let text_to_translate = args[text_start_idx..].join(" ");
                    self.translate_text(&text_to_translate).await
                } else {
                    let source_code = ConfigManager::language_to_code(&source);
                    let target_code = ConfigManager::language_to_code(&target);
                    println!("Languages set: {} ({}) -> {} ({})", source, source_code, target, target_code);
                    Ok(())
                }
            }
            "-s" | "--speech" => {
                // Speak the following text
                if args.len() < 3 {
                    eprintln!("Error: No text provided for speech");
                    eprintln!("Usage: tagent -s \"text to speak\"");
                    return Ok(());
                }
                let text_to_speak = args[2..].join(" ");
                self.speak_text(&text_to_speak).await
            }
            "-q" => {
                // Exit command for CLI mode (though it doesn't make much sense here)
                println!("Exiting...");
                Ok(())
            }
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
            match self
                .translator
                .get_dictionary_entry_public(text, &source_code, &target_code)
                .await
            {
                Ok(dictionary_info) => {
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
                        &source_code,
                        &target_code,
                        &config,
                    ) {
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
        self.perform_translation(text, &source_code, &target_code, &config)
            .await
    }

    /// Perform translation and display results
    async fn perform_translation(
        &self,
        text: &str,
        source_code: &str,
        target_code: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error>> {
        // Perform translation
        match self
            .translator
            .translate_text_public(text, source_code, target_code)
            .await
        {
            Ok(translated_text) => {
                println!("{}", translated_text);

                if config.copy_to_clipboard {
                    self.copy_to_clipboard(&translated_text).ok(); // Ignore clipboard errors
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
                eprintln!("Translation failed: {}", e);
                return Err(e);
            }
        }

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
    fn copy_to_clipboard(&self, text: &str) -> Result<(), Box<dyn Error>> {
        use crate::clipboard::ClipboardManager;
        let clipboard = ClipboardManager::new();
        clipboard.set_text(text)
    }

    /// Speak text using text-to-speech
    async fn speak_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        if text.trim().is_empty() {
            eprintln!("Error: Empty text provided");
            eprintln!("Usage: tagent -s \"text to speak\"");
            return Ok(());
        }

        self.speech_manager
            .speak_text_full(text, &self.config_manager)
            .await
            .map(|_| ())
            .map_err(|e| e.into())
    }
}
