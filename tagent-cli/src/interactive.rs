use crate::cli::CliHandler;
use crate::config::{self, ConfigManager, LanguagePair};
use crate::platform::{ClipboardManager, TerminalTitle};
use crate::speech::SpeechManager;
use crate::translator::{DictionaryLookup, LastTranslation, Translator};
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
    "/help", "/h", "/?", "/config", "/c", "/lang", "/l", "/save", "/speech", "/s", "/ss", "/clear",
    "/cls", "/quit", "/q", "/exit", "/e", "/version", "/v",
];

/// A parsed `/s`, `/speech` or `/ss` command.
#[derive(Debug, PartialEq, Eq)]
enum SpeechCommand<'a> {
    /// `/s <text>`: speak the given text.
    Text(&'a str),
    /// Bare `/s`: speak the last translated phrase.
    LastPhrase,
    /// `/ss`: speak the translation of the last phrase.
    LastTranslation,
    /// `/ss <anything>`: `/ss` takes no arguments.
    Usage,
}

/// Parses `text` (already trimmed) as a speech command; `None` if it isn't one.
fn parse_speech_command(text: &str) -> Option<SpeechCommand<'_>> {
    match text {
        "/s" | "/speech" => return Some(SpeechCommand::LastPhrase),
        "/ss" => return Some(SpeechCommand::LastTranslation),
        _ => {}
    }
    if text.starts_with("/ss ") {
        return Some(SpeechCommand::Usage);
    }
    text.strip_prefix("/s ")
        .or_else(|| text.strip_prefix("/speech "))
        .map(|rest| SpeechCommand::Text(rest.trim()))
}

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

        // Shows the current language pair in the terminal window title, which stays
        // visible while a hotkey translation is triggered from another application.
        // Restored to the previous title when dropped at the end of this function.
        let mut terminal_title = TerminalTitle::new();

        loop {
            // Check if we should exit
            if self.should_exit.load(Ordering::Relaxed) {
                break;
            }

            // Check if config file was modified and reload if necessary
            self.config_manager.check_and_reload().ok();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();

            // Rebuilt every iteration, so both reflect `/l` and config-file edits.
            let pair_label = config::language_pair_label(&source_code, &target_code);
            terminal_title.set(&format!("Tagent — {}", pair_label));
            let prompt = config::colorize(
                &format!("[{}]: ", pair_label),
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
                            println!(
                                "{}",
                                config::colorize(
                                    &format!("Translation error: {}", e),
                                    &config.error_color
                                )
                            );
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
        if text == "/l" || text == "/lang" || text.starts_with("/l ") || text.starts_with("/lang ")
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
                let pair =
                    LanguagePair::swapped(&config.source_language, &config.target_language);
                for notice in &pair.notices {
                    println!("{}", notice);
                }

                self.config_manager.set_languages(&pair.source, &pair.target);
                println!(
                    "Languages swapped: {} ({}) -> {} ({})",
                    pair.source,
                    tagent::languages::name_to_code(&pair.source),
                    pair.target,
                    tagent::languages::name_to_code(&pair.target)
                );
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

            let source_code = tagent::languages::name_to_code(&source);
            let target_code = tagent::languages::name_to_code(&target);

            // Warn if language is completely unknown
            if source.to_lowercase() != "auto" && source_code == source.as_str() {
                println!(
                    "Warning: Unknown language '{}', using as language code",
                    source
                );
            }
            if target_code == target.as_str() {
                println!(
                    "Warning: Unknown language '{}', using as language code",
                    target
                );
            }

            let pair = LanguagePair::new(&source, &target);
            for notice in &pair.notices {
                println!("{}", notice);
            }
            self.config_manager.set_languages(&pair.source, &pair.target);
            println!(
                "Languages set: {} ({}) -> {} ({})",
                pair.source,
                source_code,
                pair.target,
                tagent::languages::name_to_code(&pair.target)
            );
            println!();
            return Ok(true);
        }

        if let Some(command) = parse_speech_command(text) {
            let result = match command {
                SpeechCommand::Text(speech_text) => self.speak_interactive_text(speech_text).await,
                SpeechCommand::LastPhrase => match self.translator.last_translation() {
                    Some(LastTranslation {
                        phrase,
                        source_code,
                        ..
                    }) => self.speak_in(&phrase, &source_code).await,
                    None => {
                        println!("Nothing to speak yet: translate something first");
                        Ok(())
                    }
                },
                SpeechCommand::LastTranslation => match self.translator.last_translation() {
                    Some(LastTranslation {
                        translation: Some(translation),
                        target_code,
                        ..
                    }) => self.speak_in(&translation, &target_code).await,
                    Some(_) => {
                        println!("No translation to speak for the last phrase");
                        Ok(())
                    }
                    None => {
                        println!("Nothing to speak yet: translate something first");
                        Ok(())
                    }
                },
                SpeechCommand::Usage => {
                    println!("Usage: /ss (speaks the translation of the last phrase)");
                    Ok(())
                }
            };
            if let Err(e) = result {
                println!(
                    "{}",
                    config::colorize(
                        &format!("Speech error: {}", e),
                        &self.config_manager.get_config().error_color
                    )
                );
            }
            println!();
            Ok(true)
        } else {
            match text {
                "" => Ok(true), // Skip empty lines

                // Exit commands (only with slash)
                "/q" | "/quit" | "/exit" | "/e" => {
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
                    ConfigManager::display_banner(Some(&self.config_manager.get_config()));
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
                Ok(DictionaryLookup {
                    formatted: dictionary_info,
                    lines,
                    corrected_word,
                    primary_translation,
                }) => {
                    self.translator.record_last_translation(
                        text,
                        primary_translation.as_deref(),
                        source_code,
                        target_code,
                    );
                    // If a spelling correction was applied, notify the user
                    if config.spell_check {
                        if let Some(ref corrected) = corrected_word {
                            if corrected.to_lowercase() != text.to_lowercase() {
                                println!(
                                    "{}",
                                    config::colorize(
                                        &crate::translator::Translator::correction_notice(
                                            corrected,
                                            target_code
                                        ),
                                        &config.notice_color
                                    )
                                );
                            }
                        }
                    }

                    // Print colored dictionary label
                    config::print_colored("[Word]: ", &config.dictionary_prompt_color);
                    println!("{}", config::render_article(&lines, config));

                    if config.copy_to_clipboard {
                        let clipboard = ClipboardManager::new();
                        if let Err(e) = clipboard.set_text(&dictionary_info) {
                            println!(
                                "{}",
                                config::colorize(
                                    &format!("Clipboard error: {}", e),
                                    &config.error_color
                                )
                            );
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
                        println!(
                            "{}",
                            config::colorize(
                                &format!("History save error: {}", e),
                                &config.error_color
                            )
                        );
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
                self.translator.record_last_translation(
                    text,
                    Some(&translated_text),
                    source_code,
                    target_code,
                );

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
                    println!(
                        "{}",
                        config::colorize(
                            &format!("History save error: {}", e),
                            &config.error_color
                        )
                    );
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

    /// Speak text in `lang_code` (`"auto"` is detected), e.g. for `/s` and `/ss` replay
    async fn speak_in(&self, text: &str, lang_code: &str) -> Result<(), String> {
        self.speech_manager
            .speak_text_in(text, lang_code, &self.config_manager)
            .await
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::history::DefaultHistory;

    #[test]
    fn parses_speech_commands() {
        assert_eq!(parse_speech_command("/s"), Some(SpeechCommand::LastPhrase));
        assert_eq!(
            parse_speech_command("/speech"),
            Some(SpeechCommand::LastPhrase)
        );
        assert_eq!(
            parse_speech_command("/ss"),
            Some(SpeechCommand::LastTranslation)
        );
        assert_eq!(
            parse_speech_command("/ss hello"),
            Some(SpeechCommand::Usage)
        );
        assert_eq!(
            parse_speech_command("/s hello world"),
            Some(SpeechCommand::Text("hello world"))
        );
        assert_eq!(
            parse_speech_command("/speech   hi"),
            Some(SpeechCommand::Text("hi"))
        );
        assert_eq!(parse_speech_command("/save"), None);
        assert_eq!(parse_speech_command("/sss"), None);
        assert_eq!(parse_speech_command("so"), None);
    }

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
