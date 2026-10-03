use crate::cli::CliHandler;
use crate::config::{self, ConfigManager, LanguagePair};
use crate::platform::{ClipboardManager, TerminalTitle};
use crate::speech::SpeechManager;
use crate::translator::{DictionaryLookup, LastTranslation, Translation, Translator};
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
use std::sync::{Arc, Mutex};
use tagent::providers::ProviderAxis;

/// Slash-commands offered for Tab-completion at the interactive prompt.
pub(crate) const SLASH_COMMANDS: &[&str] = &[
    "/help",
    "/h",
    "/?",
    "/config",
    "/config update",
    "/c",
    "/lang",
    "/l",
    "/save",
    "/provider",
    "/p",
    "/speech",
    "/s",
    "/ss",
    "/clear",
    "/cls",
    "/quit",
    "/q",
    "/exit",
    "/e",
    "/version",
    "/v",
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

/// A parsed `/p` or `/provider` command.
#[derive(Debug, PartialEq, Eq)]
enum ProviderCommand<'a> {
    /// Bare `/p`: list the providers of every axis.
    List,
    /// `/p <digits>`: the entry with that number in the list last shown.
    Number(usize),
    /// `/p <name>` (translation) or `/p <axis> <name>`: switch that axis's provider for
    /// this session.
    Switch { axis: ProviderAxis, name: &'a str },
    /// `/p <axis>` alone, when `<axis>` isn't a translation provider's name.
    AxisUsage(ProviderAxis),
    /// Anything else after `/p`: three or more words, or an unknown axis word.
    Usage,
}

/// The axis an axis word names (`t`/`translation`, `d`/`dict`/`dictionary`,
/// `s`/`speech`, any case), if any.
fn parse_axis(word: &str) -> Option<ProviderAxis> {
    match word.to_ascii_lowercase().as_str() {
        "t" | "translation" => Some(ProviderAxis::Translation),
        "d" | "dict" | "dictionary" => Some(ProviderAxis::Dictionary),
        "s" | "speech" => Some(ProviderAxis::Speech),
        _ => None,
    }
}

/// The short axis word `/p <axis> <name>` takes for `axis`.
fn axis_word(axis: ProviderAxis) -> &'static str {
    match axis {
        ProviderAxis::Translation => "t",
        ProviderAxis::Dictionary => "d",
        ProviderAxis::Speech => "s",
    }
}

/// Parses `text` (already trimmed) as a provider command; `None` if it isn't one.
///
/// Profile names are `[a-z0-9_-]+`, so a lone argument could be a number, an axis word
/// and a profile name at once. Digits are always a number; a lone axis word is a
/// translation provider if `is_translation_name` says one has that name, else a usage
/// hint for that axis; the second of two words is always a name, so `/p t 2` reaches a
/// profile named `2`.
fn parse_provider_command(
    text: &str,
    is_translation_name: impl Fn(&str) -> bool,
) -> Option<ProviderCommand<'_>> {
    if text == "/p" || text == "/provider" {
        return Some(ProviderCommand::List);
    }
    let rest = text
        .strip_prefix("/p ")
        .or_else(|| text.strip_prefix("/provider "))?;
    let args: Vec<&str> = rest.split_whitespace().collect();
    Some(match args[..] {
        [arg] if arg.bytes().all(|b| b.is_ascii_digit()) => {
            // Too many digits to be any entry's number: out of range like 0.
            ProviderCommand::Number(arg.parse().unwrap_or(0))
        }
        [arg] => match parse_axis(arg) {
            Some(axis) if !is_translation_name(arg) => ProviderCommand::AxisUsage(axis),
            _ => ProviderCommand::Switch {
                axis: ProviderAxis::Translation,
                name: arg,
            },
        },
        [axis, name] => match parse_axis(axis) {
            Some(axis) => ProviderCommand::Switch { axis, name },
            None => ProviderCommand::Usage,
        },
        _ => ProviderCommand::Usage,
    })
}

/// Whether `config` offers a translation provider (kind or profile) named `name`.
fn is_translation_name(config: &config::Config, name: &str) -> bool {
    config.provider_list().iter().any(|entry| {
        entry.axis == ProviderAxis::Translation && entry.name.eq_ignore_ascii_case(name)
    })
}

/// The `(axis, name)` of entry `number` (1-based) of a list `/p` showed, or the message
/// to show when there is no such entry.
fn resolve_provider_number(
    list: &[(ProviderAxis, String)],
    number: usize,
) -> Result<(ProviderAxis, String), String> {
    match number.checked_sub(1).and_then(|index| list.get(index)) {
        Some(entry) => Ok(entry.clone()),
        None if list.is_empty() => Err("No providers to choose from".to_string()),
        None => Err(format!(
            "No provider number {number}: choose 1-{} (/p shows the list)",
            list.len()
        )),
    }
}

/// Switches `config_manager`'s provider of `axis` to `name` (in memory) if it can be
/// built, otherwise leaves it unchanged. Returns the message to show, which names the
/// axis (the user may have typed only a number).
fn switch_provider(config_manager: &ConfigManager, axis: ProviderAxis, name: &str) -> String {
    let title = config::axis_title(axis);
    let name = name.to_lowercase();
    let mut config = config_manager.get_config();
    if config.provider_name(axis).eq_ignore_ascii_case(&name) {
        return match config.build_provider_name(axis) {
            Ok(display_name) => format!("{title} provider: {display_name} (already active)"),
            Err(message) => message,
        };
    }
    let field = match axis {
        ProviderAxis::Translation => &mut config.translate_provider,
        ProviderAxis::Dictionary => &mut config.dictionary_provider,
        ProviderAxis::Speech => &mut config.speech_provider,
    };
    *field = name.clone();
    match config.build_provider_name(axis) {
        Ok(display_name) => {
            config_manager.set_provider(axis, &name);
            format!("{title} provider: {display_name} (this session; /save to keep)")
        }
        Err(message) => config::colorize(
            &format!("{message}\n({} provider unchanged)", axis.label()),
            &config.error_color,
        ),
    }
}

/// The words Tab offers after `/p ` (`args` is what follows it): the axis words and the
/// translation providers for the first word, the providers of that axis for the second.
/// Each candidate is a whole word; what the user typed of it is matched as a prefix.
fn provider_completions(config: &config::Config, args: &str) -> Vec<String> {
    let mut words: Vec<&str> = args.split_whitespace().collect();
    if args.is_empty() || args.ends_with(char::is_whitespace) {
        words.push("");
    }
    let (axis, partial) = match words[..] {
        [partial] => (None, partial),
        [axis, partial] => match parse_axis(axis) {
            Some(axis) => (Some(axis), partial),
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let names = |axis: ProviderAxis| {
        config
            .provider_list()
            .into_iter()
            .filter(move |entry| entry.axis == axis)
            .map(|entry| entry.name)
    };
    let candidates: Vec<String> = match axis {
        Some(axis) => names(axis).collect(),
        None => ["translation", "dictionary", "speech"]
            .into_iter()
            .map(String::from)
            .chain(names(ProviderAxis::Translation))
            .collect(),
    };
    let partial = partial.to_ascii_lowercase();
    let mut matching: Vec<String> = Vec::new();
    for candidate in candidates {
        if candidate.starts_with(&partial) && !matching.contains(&candidate) {
            matching.push(candidate);
        }
    }
    matching
}

/// Rustyline [`Helper`] that Tab-completes slash-commands, and the axis words and provider
/// names after `/p`. Hints, highlighting, and validation are left at rustyline's no-op
/// defaults.
struct TagentHelper {
    /// Where `/p`'s completions read the provider profiles from; `None` completes
    /// slash-commands only.
    config_manager: Option<Arc<ConfigManager>>,
}

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

        let provider_args = line
            .strip_prefix("/p ")
            .or_else(|| line.strip_prefix("/provider "));
        if let (Some(args), Some(manager)) = (provider_args, &self.config_manager) {
            let word_start = line.rfind(char::is_whitespace).map_or(0, |space| space + 1);
            let candidates = provider_completions(&manager.get_config(), args);
            return Ok((word_start, candidates));
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
    /// The `(axis, name)` pairs of the list `/p` last printed, in order, so `/p <number>`
    /// means the entry as it was shown even if profiles changed since.
    provider_snapshot: Mutex<Option<Vec<(ProviderAxis, String)>>>,
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
            provider_snapshot: Mutex::new(None),
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
        editor.set_helper(Some(TagentHelper {
            config_manager: Some(self.config_manager.clone()),
        }));

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
            self.config_manager.reload_or_warn();
            let config = self.config_manager.get_config();
            let (source_code, target_code) = self.config_manager.get_language_codes();

            // Rebuilt every iteration, so both reflect `/l` and config-file edits.
            let pair_label = config::language_pair_label(&source_code, &target_code);
            terminal_title.set(&format!("Tagent — {}", pair_label));
            let prompt =
                config::colorize(&format!("[{}]: ", pair_label), &config.source_prompt_color);

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
                let pair = LanguagePair::swapped(&config.source_language, &config.target_language);
                for notice in &pair.notices {
                    println!("{}", notice);
                }

                self.config_manager
                    .set_languages(&pair.source, &pair.target);
                println!(
                    "Languages swapped: {}",
                    config::language_pair_description(&pair.source, &pair.target)
                );
                println!();
                return Ok(true);
            }

            let (warnings, pair) = resolve_language_args(&parts);
            for warning in &warnings {
                println!("{}", warning);
            }
            for notice in &pair.notices {
                println!("{}", notice);
            }
            self.config_manager
                .set_languages(&pair.source, &pair.target);
            println!(
                "Languages set: {}",
                config::language_pair_description(&pair.source, &pair.target)
            );
            println!();
            return Ok(true);
        }

        if self.handle_provider_command(text) {
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

                // Add the settings the config file lacks
                "/c update" | "/config update" => {
                    match self.config_manager.update_config_file() {
                        Ok(update) => {
                            for line in update.lines() {
                                println!("{line}");
                            }
                        }
                        Err(e) => println!("Error updating configuration: {}", e),
                    }
                    println!();
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
                    let config = self.config_manager.get_config();
                    ConfigManager::display_banner(
                        &config,
                        &self.translator.active_providers(&config),
                    );
                    Ok(true)
                }

                _ => Ok(false), // Not a command, should be translated
            }
        }
    }

    /// Runs `text` if it is a `/p` command (see [`parse_provider_command`]); `false` if it
    /// isn't one. The file is reloaded first, so the list and a switch see its profiles.
    fn handle_provider_command(&self, text: &str) -> bool {
        if parse_provider_command(text, |_| false).is_none() {
            return false;
        }
        self.config_manager.reload_or_warn();
        let config = self.config_manager.get_config();
        let snapshot = |entries: Vec<config::ProviderListEntry>| -> Vec<(ProviderAxis, String)> {
            entries
                .into_iter()
                .map(|entry| (entry.axis, entry.name))
                .collect()
        };
        match parse_provider_command(text, |name| is_translation_name(&config, name)) {
            None => return false,
            Some(ProviderCommand::List) => {
                let entries = config.provider_list();
                for line in config::provider_list_lines(&entries, &config) {
                    println!("{line}");
                }
                *self.provider_snapshot.lock().unwrap() = Some(snapshot(entries));
            }
            Some(ProviderCommand::Number(number)) => {
                let shown = self.provider_snapshot.lock().unwrap().clone();
                let list = shown.unwrap_or_else(|| snapshot(config.provider_list()));
                match resolve_provider_number(&list, number) {
                    Ok((axis, name)) => {
                        println!("{}", switch_provider(&self.config_manager, axis, &name));
                    }
                    Err(message) => println!("{}", config::colorize(&message, &config.error_color)),
                }
            }
            Some(ProviderCommand::Switch { axis, name }) => {
                println!("{}", switch_provider(&self.config_manager, axis, name));
            }
            Some(ProviderCommand::AxisUsage(axis)) => {
                println!("Usage: /p {} <name>", axis_word(axis));
            }
            Some(ProviderCommand::Usage) => println!("Usage: /p [number | name | t|d|s name]"),
        }
        true
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
                                    crate::translator::Translator::correction_notice(
                                        corrected,
                                        target_code,
                                        config
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
            Ok(Translation {
                text: translated_text,
                provider,
            }) => {
                self.translator.record_last_translation(
                    text,
                    Some(&translated_text),
                    source_code,
                    target_code,
                );

                // Print colored translation label: the provider that translated
                let trans_label = format!("[{}]: ", provider);
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

/// The language pair `/l <target>` or `/l <source> <target>` asks for (`parts` has one or
/// two items; one means an `auto` source), with a warning per value that isn't a known
/// language name or code. Names and codes in any case become codes; an unknown value is
/// kept as a language code.
fn resolve_language_args(parts: &[&str]) -> (Vec<String>, LanguagePair) {
    let (raw_source, raw_target) = match parts {
        [target] => ("auto", *target),
        [source, target, ..] => (*source, *target),
        [] => ("auto", ""),
    };
    let warnings = [raw_source, raw_target]
        .into_iter()
        .filter(|raw| tagent::languages::language_code(raw).is_none())
        .map(|raw| {
            format!(
                "Warning: Unknown language '{}', using as language code",
                raw
            )
        })
        .collect();
    let pair = LanguagePair::new(
        &config::language_code(raw_source),
        &config::language_code(raw_target),
    );
    (warnings, pair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::history::DefaultHistory;

    /// `/l German`, `/l de` and `/l DE` all store the code.
    #[test]
    fn language_args_store_codes_for_names_and_codes() {
        for parts in [["German"], ["de"], ["DE"]] {
            let (warnings, pair) = resolve_language_args(&parts);
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!((pair.source.as_str(), pair.target.as_str()), ("auto", "de"));
        }
        let (_, pair) = resolve_language_args(&["English", "ru"]);
        assert_eq!((pair.source.as_str(), pair.target.as_str()), ("en", "ru"));
    }

    #[test]
    fn language_args_keep_an_unknown_value_with_a_warning() {
        let (warnings, pair) = resolve_language_args(&["en", "zh-TW"]);
        assert_eq!(
            warnings,
            vec!["Warning: Unknown language 'zh-TW', using as language code".to_string()]
        );
        assert_eq!(pair.target, "zh-TW");

        let (warnings, pair) = resolve_language_args(&["auto"]);
        assert!(warnings.is_empty());
        assert_eq!(pair.target, config::AUTO_TARGET_FALLBACK);
    }

    fn switch(axis: ProviderAxis, name: &str) -> Option<ProviderCommand<'_>> {
        Some(ProviderCommand::Switch { axis, name })
    }

    /// The grammar table of Stage U, row by row, with no profiles named like axis words.
    #[test]
    fn parses_provider_commands() {
        let parse = |text| parse_provider_command(text, |_| false);
        assert_eq!(parse("/p"), Some(ProviderCommand::List));
        assert_eq!(parse("/provider"), Some(ProviderCommand::List));
        assert_eq!(parse("/p 5"), Some(ProviderCommand::Number(5)));
        assert_eq!(parse("/provider 12"), Some(ProviderCommand::Number(12)));
        assert_eq!(
            parse("/p deepl"),
            switch(ProviderAxis::Translation, "deepl")
        );
        assert_eq!(
            parse("/provider   deepl-work"),
            switch(ProviderAxis::Translation, "deepl-work")
        );
        assert_eq!(
            parse("/p d ollama"),
            switch(ProviderAxis::Dictionary, "ollama")
        );
        assert_eq!(
            parse("/p DICT ollama"),
            switch(ProviderAxis::Dictionary, "ollama")
        );
        assert_eq!(
            parse("/p dictionary x"),
            switch(ProviderAxis::Dictionary, "x")
        );
        assert_eq!(parse("/p s google"), switch(ProviderAxis::Speech, "google"));
        assert_eq!(
            parse("/p speech google"),
            switch(ProviderAxis::Speech, "google")
        );
        assert_eq!(
            parse("/p t deepl"),
            switch(ProviderAxis::Translation, "deepl")
        );
        assert_eq!(
            parse("/p translation deepl"),
            switch(ProviderAxis::Translation, "deepl")
        );
        assert_eq!(
            parse("/p d"),
            Some(ProviderCommand::AxisUsage(ProviderAxis::Dictionary))
        );
        assert_eq!(
            parse("/p S"),
            Some(ProviderCommand::AxisUsage(ProviderAxis::Speech))
        );
        assert_eq!(parse("/p deepl google"), Some(ProviderCommand::Usage));
        assert_eq!(parse("/p d ollama extra"), Some(ProviderCommand::Usage));
        assert_eq!(parse("/pp"), None);
        assert_eq!(parse("/print"), None);
        assert_eq!(parse("/providers"), None);
        assert_eq!(parse("p deepl"), None);
    }

    /// Digits are always a number, even when a profile has that name; the axis form
    /// reaches the profile.
    #[test]
    fn digits_are_a_number_and_the_axis_form_reaches_a_numeric_profile() {
        let parse = |text| parse_provider_command(text, |name| name == "2");
        assert_eq!(parse("/p 2"), Some(ProviderCommand::Number(2)));
        assert_eq!(parse("/p t 2"), switch(ProviderAxis::Translation, "2"));
        // Too many digits for any entry: out of range like 0.
        assert_eq!(
            parse("/p 99999999999999999999999"),
            Some(ProviderCommand::Number(0))
        );
    }

    /// A lone axis word is a translation provider if one has that name.
    #[test]
    fn lone_axis_word_selects_a_translation_profile_of_that_name() {
        let parse = |text| parse_provider_command(text, |name| name == "t");
        assert_eq!(parse("/p t"), switch(ProviderAxis::Translation, "t"));
        assert_eq!(
            parse("/p d"),
            Some(ProviderCommand::AxisUsage(ProviderAxis::Dictionary))
        );
    }

    #[test]
    fn provider_numbers_resolve_against_the_list() {
        let list = vec![
            (ProviderAxis::Translation, "google".to_string()),
            (ProviderAxis::Dictionary, "ollama".to_string()),
        ];

        assert_eq!(
            resolve_provider_number(&list, 2),
            Ok((ProviderAxis::Dictionary, "ollama".to_string()))
        );
        for number in [0, 3] {
            let error = resolve_provider_number(&list, number).unwrap_err();
            assert!(error.contains(&format!("number {number}")), "{error}");
            assert!(error.contains("1-2"), "{error}");
        }
        assert!(resolve_provider_number(&[], 1).is_err());
    }

    /// A config manager over a file with a `work` profile (Google), a `nokey` profile
    /// (DeepL without its required `api_key`), an `ollama` profile (OpenAI-compatible,
    /// translation and dictionary; builds offline) and a profile named `t`.
    fn provider_test_manager(unique: &str) -> Arc<ConfigManager> {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_interactive_{}_{}.toml",
            unique,
            std::process::id()
        ));
        std::fs::write(
            &path,
            "[provider_options.work]\ntype = \"google\"\n\
             [provider_options.nokey]\ntype = \"deepl\"\n\
             [provider_options.ollama]\ntype = \"openai\"\n\
             endpoint = \"http://localhost:11434/v1\"\nmodel = \"dummy\"\n\
             [provider_options.t]\ntype = \"google\"\n",
        )
        .unwrap();
        let manager = ConfigManager::new(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        Arc::new(manager)
    }

    #[test]
    fn switching_provider_changes_the_config_in_memory() {
        let manager = provider_test_manager("switch_ok");

        let message = switch_provider(&manager, ProviderAxis::Translation, "WORK");

        assert_eq!(manager.get_config().translate_provider, "work");
        assert!(message.starts_with("Translation provider: "), "{message}");
        assert!(message.contains("Google Translate (work)"), "{message}");
        assert!(message.contains("/save"), "{message}");
    }

    #[test]
    fn switching_dictionary_and_speech_changes_only_that_axis() {
        let manager = provider_test_manager("switch_axes");

        let message = switch_provider(&manager, ProviderAxis::Dictionary, "ollama");
        assert_eq!(
            message,
            "Dictionary provider: OpenAI-compatible (ollama) (this session; /save to keep)"
        );
        let message = switch_provider(&manager, ProviderAxis::Speech, "work");
        assert_eq!(
            message,
            "Speech provider: Google TTS (work) (this session; /save to keep)"
        );

        let config = manager.get_config();
        assert_eq!(config.translate_provider, "google");
        assert_eq!(config.dictionary_provider, "ollama");
        assert_eq!(config.speech_provider, "work");
    }

    #[test]
    fn switching_to_an_unusable_provider_keeps_the_current_one() {
        let manager = provider_test_manager("switch_bad");

        for name in ["no-such-provider", "nokey"] {
            let message = switch_provider(&manager, ProviderAxis::Translation, name);

            assert_eq!(manager.get_config().translate_provider, "google");
            assert!(
                message.contains("translation provider unchanged"),
                "{name}: {message}"
            );
        }
        // DeepL serves no dictionary, so its profile can't be the dictionary provider.
        let message = switch_provider(&manager, ProviderAxis::Dictionary, "nokey");
        assert_eq!(manager.get_config().dictionary_provider, "google");
        assert!(
            message.contains("dictionary provider unchanged"),
            "{message}"
        );
        let message = switch_provider(&manager, ProviderAxis::Speech, "ollama");
        assert_eq!(manager.get_config().speech_provider, "google");
        assert!(message.contains("speech provider unchanged"), "{message}");
    }

    #[test]
    fn switching_to_the_active_provider_is_a_no_op() {
        let manager = provider_test_manager("switch_same");

        let message = switch_provider(&manager, ProviderAxis::Translation, "google");
        assert_eq!(manager.get_config().translate_provider, "google");
        assert!(message.contains("already active"), "{message}");

        let message = switch_provider(&manager, ProviderAxis::Dictionary, "Google");
        assert_eq!(
            message,
            "Dictionary provider: Google Dictionary (already active)"
        );
    }

    /// `is_translation_name` sees profiles, so `/p t` reaches the profile `t`.
    #[test]
    fn translation_names_include_profiles() {
        let manager = provider_test_manager("names");
        let config = manager.get_config();

        assert!(is_translation_name(&config, "t"));
        assert!(is_translation_name(&config, "OLLAMA"));
        assert!(!is_translation_name(&config, "d"));
    }

    #[test]
    fn provider_command_numbers_follow_the_printed_list() {
        let manager = provider_test_manager("numbers");
        let mode = InteractiveMode::with_translator(
            Translator::new_cli(manager.clone()).unwrap(),
            manager.clone(),
        );
        let entries = manager.get_config().provider_list();
        let number = entries
            .iter()
            .position(|e| e.axis == ProviderAxis::Dictionary && e.name == "ollama")
            .unwrap()
            + 1;

        // No list printed yet: a fresh one is used.
        assert!(mode.handle_provider_command(&format!("/p {number}")));
        assert_eq!(manager.get_config().dictionary_provider, "ollama");

        // The snapshot wins over the current list.
        *mode.provider_snapshot.lock().unwrap() =
            Some(vec![(ProviderAxis::Speech, "work".to_string())]);
        assert!(mode.handle_provider_command("/p 1"));
        assert_eq!(manager.get_config().speech_provider, "work");
        assert!(mode.handle_provider_command("/p 2"));
        assert_eq!(manager.get_config().translate_provider, "google");

        // `/p` replaces the snapshot with what it printed.
        assert!(mode.handle_provider_command("/p"));
        assert_eq!(
            mode.provider_snapshot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .len(),
            entries.len()
        );
        assert!(!mode.handle_provider_command("/pp"));
    }

    #[test]
    fn completes_axis_words_and_provider_names_after_p() {
        let manager = provider_test_manager("complete");
        let helper = TagentHelper {
            config_manager: Some(manager),
        };
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);
        let complete = |line: &str| helper.complete(line, line.len(), &ctx).unwrap();

        let (start, candidates) = complete("/p d");
        assert_eq!(start, 3);
        assert_eq!(candidates, ["dictionary", "deepl"]);

        let (start, candidates) = complete("/p d ");
        assert_eq!(start, 5);
        assert_eq!(candidates, ["google", "openai", "ollama", "t", "work"]);

        let (start, candidates) = complete("/provider speech w");
        assert_eq!(start, 17);
        assert_eq!(candidates, ["work"]);

        let (_, candidates) = complete("/p o");
        assert_eq!(candidates, ["openai", "ollama"]);
        assert!(complete("/p x y").1.is_empty());
        assert!(complete("/p d google x").1.is_empty());
        // Slash-commands still complete.
        assert!(complete("/pro").1.contains(&"/provider".to_string()));
    }

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
        let helper = TagentHelper {
            config_manager: None,
        };
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
        let helper = TagentHelper {
            config_manager: None,
        };
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (_, candidates) = helper.complete("hello", 5, &ctx).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn no_completion_when_cursor_not_at_end() {
        let helper = TagentHelper {
            config_manager: None,
        };
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (_, candidates) = helper.complete("/help", 1, &ctx).unwrap();
        assert!(candidates.is_empty());
    }
}
