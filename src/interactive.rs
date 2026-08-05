use crate::cli::CliHandler;
use crate::platform::ClipboardManager;
use crate::config::{self, ConfigManager};
use crate::speech::SpeechManager;
use crate::translator::Translator;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, EditMode, Editor, Helper};
use std::error::Error;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Slash-commands offered for Tab-completion at the interactive prompt.
const SLASH_COMMANDS: &[&str] = &[
    "/help", "/h", "/?",
    "/config", "/c",
    "/lang", "/l",
    "/save",
    "/speech", "/s",
    "/clear", "/cls",
    "/quit", "/q", "/exit",
    "/version", "/v",
];

/// Rustyline [`Helper`] that Tab-completes slash-commands. Hints, highlighting, and
/// validation are left at rustyline's no-op defaults.
struct TagentHelper;

impl Completer for TagentHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        // Only complete slash-commands, and only while the cursor sits at the end of them.
        if pos != line.len() || !line.starts_with('/') {
            return Ok((0, Vec::new()));
        }

        let candidates: Vec<String> = SLASH_COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(line))
            .map(|cmd| cmd.to_string())
            .collect();

        Ok((0, candidates))
    }
}

impl Hinter for TagentHelper {
    type Hint = String;
}

impl Highlighter for TagentHelper {}

impl Validator for TagentHelper {}

impl Helper for TagentHelper {}

pub struct InteractiveMode {
    translator: Translator,
    config_manager: Arc<ConfigManager>,
    should_exit: Arc<AtomicBool>,
    speech_manager: SpeechManager,
}

impl InteractiveMode {
    pub fn with_translator(translator: Translator, config_manager: Arc<ConfigManager>) -> Self {
        let should_exit = Arc::new(AtomicBool::new(false));
        let speech_manager = SpeechManager::new();

        Self {
            translator,
            config_manager,
            should_exit,
            speech_manager,
        }
    }

    pub fn get_exit_flag(&self) -> Arc<AtomicBool> {
        self.should_exit.clone()
    }

    /// Start interactive translation mode (unified with GUI)
    pub async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let rl_config = rustyline::Config::builder()
            .max_history_size(1000)?
            .history_ignore_dups(true)?
            .edit_mode(EditMode::Emacs)
            .build();

        let mut editor = Editor::<TagentHelper, DefaultHistory>::with_config(rl_config)?;
        editor.set_helper(Some(TagentHelper));

        let history_path = ConfigManager::get_default_interactive_history_path()
            .map_err(|e| format!("Failed to resolve interactive history path: {}", e))?;

        match editor.load_history(&history_path) {
            Ok(()) => {}
            Err(ReadlineError::Io(ref e)) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => println!("Warning: failed to load interactive history: {}", e),
        }

        // Route hotkey-triggered translation output through this printer so it can't
        // corrupt the prompt/typed-so-far text if the hotkey fires while readline() is
        // reading a line in raw mode.
        match editor.create_external_printer() {
            Ok(printer) => self.translator.set_external_printer(printer),
            Err(e) => println!(
                "Warning: could not set up safe hotkey output routing: {}",
                e
            ),
        }

        loop {
            // Check if we should exit
            if self.should_exit.load(Ordering::Relaxed) {
                break;
            }

            // Check if config file was modified and reload if necessary
            self.config_manager.check_and_reload().ok();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();

            let prompt = config::colorize(
                &format!("[{}]: ", config.source_language),
                &config.source_prompt_color,
            );

            match editor.readline(&prompt) {
                Ok(line) => {
                    editor.add_history_entry(line.as_str()).ok();
                    editor.append_history(&history_path).ok();

                    let text = line.trim();

                    // Handle commands first
                    if self.handle_command(text).await? {
                        continue;
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
                Err(ReadlineError::Interrupted) => {
                    // True bash behavior: Ctrl+C never exits on its own, just reprints the prompt.
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl+D on an empty line: same as /quit.
                    println!();
                    println!("Goodbye!");
                    self.should_exit.store(true, Ordering::SeqCst);
                    break;
                }
                Err(e) => {
                    println!("Input error: {}", e);
                    break;
                }
            }
        }

        editor.save_history(&history_path).ok();

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
            println!();
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
                    ConfigManager::display_help();
                    Ok(true)
                }

                // Config commands (only with slash)
                "/c" | "/config" => {
                    if let Err(e) = self.config_manager.display_config() {
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

    /// Translate text in interactive mode
    async fn translate_interactive_text(
        &self,
        text: &str,
        source_code: &str,
        target_code: &str,
        config: &crate::config::Config,
    ) -> Result<(), String> {
        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && config::is_single_word(text) {
            match self
                .translator
                .get_dictionary_entry(text, source_code, target_code)
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
                                        target_code
                                    )
                                );
                            }
                        }
                    }

                    // Print colored dictionary label
                    config::print_colored("[Word]: ", &config.dictionary_prompt_color);
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
                        source_code,
                        target_code,
                        config,
                    ) {
                        println!("History save error: {}", e);
                    }

                    println!();
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
                config::print_colored(&trans_label, &config.target_prompt_color);
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
                return Err(format!("Translation failed: {}", e));
            }
        }

        println!();
        Ok(())
    }

    /// Speak text using text-to-speech in interactive mode
    async fn speak_interactive_text(&self, text: &str) -> Result<(), String> {
        self.speech_manager
            .speak_text_full(text, &self.config_manager)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::history::DefaultHistory;

    #[test]
    fn completes_slash_commands_by_prefix() {
        let helper = TagentHelper;
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper.complete("/h", 2, &ctx).unwrap();

        assert_eq!(start, 0);
        assert!(candidates.contains(&"/help".to_string()));
        assert!(candidates.contains(&"/h".to_string()));
        assert!(!candidates.iter().any(|c| c == "/lang"));
    }

    #[test]
    fn no_completion_without_leading_slash() {
        let helper = TagentHelper;
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (_, candidates) = helper.complete("hello", 5, &ctx).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn no_completion_when_cursor_not_at_end() {
        let helper = TagentHelper;
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (_, candidates) = helper.complete("/help", 1, &ctx).unwrap();
        assert!(candidates.is_empty());
    }
}
