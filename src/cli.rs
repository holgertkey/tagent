use crate::platform::ClipboardManager;
use crate::config::{self, ConfigManager};
use crate::speech::SpeechManager;
use crate::translator::Translator;
use std::error::Error;
use std::sync::Arc;

pub struct CliHandler {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
    speech_manager: SpeechManager,
}

impl CliHandler {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let config_path = ConfigManager::get_default_config_path()?;
        let config_manager = Arc::new(ConfigManager::new(config_path.to_string_lossy().as_ref())?);
        let translator = Translator::new_cli(config_manager.clone())?;
        let speech_manager = SpeechManager::new();

        Ok(Self {
            translator,
            config_manager,
            speech_manager,
        })
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
    pub fn show_config(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.config_manager.display_config()
    }

    /// Process CLI arguments and determine action
    pub async fn process_args(&self, args: Vec<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    pub async fn translate_text(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if text.trim().is_empty() {
            eprintln!("Error: Empty text provided");
            eprintln!("Usage: tagent <text to translate>");
            return Ok(());
        }

        // Load current configuration
        self.config_manager.check_and_reload().ok();
        let config = self.config_manager.get_config();
        let (source_code, target_code) = self.config_manager.get_language_codes();

        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && config::is_single_word(text) {
            match self
                .translator
                .get_dictionary_entry(text, &source_code, &target_code)
                .await
            {
                Ok((dictionary_info, corrected_word)) => {
                    // If a spelling correction was applied, notify the user
                    if config.spell_check {
                        if let Some(ref corrected) = corrected_word {
                            if corrected.to_lowercase() != text.to_lowercase() {
                                println!(
                                    "{}",
                                    crate::translator::Translator::correction_notice(
                                        corrected,
                                        &target_code
                                    )
                                );
                            }
                        }
                    }

                    println!("{}", dictionary_info);

                    if config.copy_to_clipboard {
                        let clipboard = ClipboardManager::new();
                        if let Err(e) = clipboard.set_text(&dictionary_info) {
                            println!("Clipboard error: {}", e);
                        }
                    }

                    // Save dictionary entry to history
                    if let Err(e) = config::save_translation_history(
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
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self
            .translator
            .translate_text_public(text, source_code, target_code)
            .await
        {
            Ok(translated_text) => {
                println!("{}", translated_text);

                if config.copy_to_clipboard {
                    let clipboard = ClipboardManager::new();
                    clipboard.set_text(&translated_text).ok();
                }

                // Save translation to history
                if let Err(e) = config::save_translation_history(
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

    /// Speak text using text-to-speech
    async fn speak_text(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
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
