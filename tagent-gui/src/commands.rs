//! Slash commands typed into the main window's input box (`/l en ru`, `/p deepl`, `/s`,
//! ...): a small subset of `tagent-cli`'s interactive commands, answered in the
//! transcript.
//!
//! Only known commands are commands: the input is one when its first word is one of
//! [`COMMAND_NAMES`] (lowercase only, like `tagent-cli`); anything else (`/usr/bin/env`,
//! `/xyz`, `1/2`) is translated as text. A leading `//` escapes a command: one `/` is
//! dropped and the rest is translated ([`unescape`]). Only the input box is parsed; the
//! translate hotkey's selection and the 📋 button's paste never are.
//!
//! Pure logic (no Slint types, no globals), so it's unit-tested here; `main.rs` runs the
//! parsed commands. The provider and speech command parsers are duplicated from
//! `tagent-cli`'s `interactive.rs` (it has no `[lib]` target, and command parsing is app
//! code, not `tagent`'s).

use crate::provider_menu::MenuSection;
use tagent::languages;
use tagent::providers::ProviderAxis;

/// Every command name and alias [`parse`] accepts.
pub const COMMAND_NAMES: &[&str] = &[
    "/l",
    "/lang",
    "/p",
    "/provider",
    "/s",
    "/speech",
    "/ss",
    "/clear",
    "/cls",
    "/help",
    "/h",
    "/?",
    "/version",
    "/v",
];

/// The target `/l` uses where the pair would otherwise get an `auto` target, as
/// `tagent-cli`'s `AUTO_TARGET_FALLBACK` does.
const AUTO_TARGET_FALLBACK: &str = "en";

/// A parsed command.
#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    /// `/l` or `/lang`, with its arguments (none = swap).
    Languages(Vec<&'a str>),
    /// `/p` or `/provider`.
    Provider(ProviderCommand<'a>),
    /// `/s`, `/speech` or `/ss`.
    Speak(SpeechCommand<'a>),
    /// `/clear` or `/cls`.
    Clear,
    /// `/help`, `/h` or `/?`.
    Help,
    /// `/version` or `/v`.
    Version,
    /// A command that takes no arguments was given some: the usage line to show.
    Usage(&'static str),
}

/// A parsed `/s`, `/speech` or `/ss` command.
#[derive(Debug, PartialEq, Eq)]
pub enum SpeechCommand<'a> {
    /// `/s <text>`: speak the given text.
    Text(&'a str),
    /// Bare `/s`: speak the last row's phrase.
    LastPhrase,
    /// `/ss`: speak the last row's translation.
    LastTranslation,
    /// `/ss <anything>`: `/ss` takes no arguments.
    Usage,
}

/// A parsed `/p` or `/provider` command.
#[derive(Debug, PartialEq, Eq)]
pub enum ProviderCommand<'a> {
    /// Bare `/p`: list the providers of every axis.
    List,
    /// `/p <digits>`: the entry with that number in the list last shown.
    Number(usize),
    /// `/p <name>` (translation) or `/p <axis> <name>`: this session's provider on that
    /// axis.
    Switch {
        /// The axis to switch.
        axis: ProviderAxis,
        /// The kind or profile name, as typed.
        name: &'a str,
    },
    /// `/p <axis>` alone, when `<axis>` isn't a translation provider's name.
    AxisUsage(ProviderAxis),
    /// Anything else after `/p`: three or more words, or an unknown axis word.
    Usage,
}

/// `text` with a leading `//` turned into `/`, the escape for translating text that
/// starts with a command name (`//l` translates `/l`); other text unchanged.
pub fn unescape(text: &str) -> &str {
    if text.starts_with("//") {
        &text[1..]
    } else {
        text
    }
}

/// Parses `text` (already trimmed) as a command; `None` means "translate it". A lone
/// argument to `/p` that is both an axis word and a translation provider's name is the
/// provider when `is_translation_name` says so (see [`parse_provider_args`]).
pub fn parse(text: &str, is_translation_name: impl Fn(&str) -> bool) -> Option<Command<'_>> {
    let name_end = text.find(char::is_whitespace).unwrap_or(text.len());
    let (name, rest) = text.split_at(name_end);
    if !COMMAND_NAMES.contains(&name) {
        return None;
    }
    let rest = rest.trim();
    let args: Vec<&str> = rest.split_whitespace().collect();
    let no_args = |command: Command<'static>, usage: &'static str| {
        if args.is_empty() {
            command
        } else {
            Command::Usage(usage)
        }
    };
    Some(match name {
        "/l" | "/lang" => Command::Languages(args),
        "/p" | "/provider" => Command::Provider(parse_provider_args(&args, is_translation_name)),
        "/s" | "/speech" if rest.is_empty() => Command::Speak(SpeechCommand::LastPhrase),
        "/s" | "/speech" => Command::Speak(SpeechCommand::Text(rest)),
        "/ss" if rest.is_empty() => Command::Speak(SpeechCommand::LastTranslation),
        "/ss" => Command::Speak(SpeechCommand::Usage),
        "/clear" | "/cls" => no_args(Command::Clear, "Usage: /clear (takes no arguments)"),
        "/help" | "/h" | "/?" => no_args(Command::Help, "Usage: /help (takes no arguments)"),
        _ => no_args(Command::Version, "Usage: /version (takes no arguments)"),
    })
}

/// The axis an axis word names (`t`/`translation`, `d`/`dict`/`dictionary`,
/// `s`/`speech`, any case), if any.
pub fn parse_axis(word: &str) -> Option<ProviderAxis> {
    match word.to_ascii_lowercase().as_str() {
        "t" | "translation" => Some(ProviderAxis::Translation),
        "d" | "dict" | "dictionary" => Some(ProviderAxis::Dictionary),
        "s" | "speech" => Some(ProviderAxis::Speech),
        _ => None,
    }
}

/// The short axis word `/p <axis> <name>` takes for `axis`.
pub fn axis_word(axis: ProviderAxis) -> &'static str {
    match axis {
        ProviderAxis::Translation => "t",
        ProviderAxis::Dictionary => "d",
        ProviderAxis::Speech => "s",
    }
}

/// Parses `/p`'s arguments.
///
/// Profile names are `[a-z0-9_-]+`, so a lone argument could be a number, an axis word
/// and a profile name at once. Digits are always a number; a lone axis word is a
/// translation provider if `is_translation_name` says one has that name, else a usage
/// hint for that axis; the second of two words is always a name, so `/p t 2` reaches a
/// profile named `2`.
fn parse_provider_args<'a>(
    args: &[&'a str],
    is_translation_name: impl Fn(&str) -> bool,
) -> ProviderCommand<'a> {
    match *args {
        [] => ProviderCommand::List,
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
    }
}

/// The usage line for a malformed `/p`.
pub const PROVIDER_USAGE: &str = "Usage: /p [number | name | t|d|s name]";

/// The window's new language pair after `/l`, as codes, with what to tell the user about
/// adjustments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageChange {
    /// Source code, possibly `"auto"`.
    pub source: String,
    /// Target code, never `"auto"`.
    pub target: String,
    /// Notices to show, in order (an `auto` target replaced, a same-language pair).
    pub notices: Vec<String>,
}

/// The pair `/l` with `args` asks for, given the window's current pair (codes): no
/// argument swaps (an `auto` source becomes English as the target), one is the target
/// with an `auto` source, two are source and target. Names and codes in any case are
/// accepted; a language the window's dropdowns don't list is an error, and nothing
/// changes.
pub fn resolve_languages(
    args: &[&str],
    current_source: &str,
    current_target: &str,
) -> Result<LanguageChange, String> {
    let mut notices = Vec::new();
    let fallback_name = languages::code_to_name(AUTO_TARGET_FALLBACK);
    let (source, target) = match *args {
        [] => {
            let target = if is_auto(current_source) {
                notices.push(format!(
                    "Source was Auto; using {fallback_name} as the new target"
                ));
                AUTO_TARGET_FALLBACK.to_string()
            } else {
                current_source.to_string()
            };
            (current_target.to_string(), target)
        }
        [target] => ("auto".to_string(), code_of(target)?),
        [source, target] => (code_of(source)?, code_of(target)?),
        _ => return Err("Usage: /l [target | source target]".to_string()),
    };
    let target = if is_auto(&target) && !args.is_empty() {
        notices.push(format!(
            "Target can't be Auto; using {fallback_name} instead"
        ));
        AUTO_TARGET_FALLBACK.to_string()
    } else {
        target
    };
    if !is_auto(&source) && source.eq_ignore_ascii_case(&target) {
        notices.push("Note: source and target are the same language".to_string());
    }
    Ok(LanguageChange {
        source,
        target,
        notices,
    })
}

fn is_auto(code: &str) -> bool {
    code.eq_ignore_ascii_case("auto")
}

/// The code of a language name or code `tagent` lists (or `auto`), else the error to show.
fn code_of(input: &str) -> Result<String, String> {
    languages::language_code(input)
        .map(str::to_string)
        .ok_or_else(|| format!("Unknown language: {input} (languages unchanged)"))
}

/// `/p`'s list in the order it numbers entries: every entry of every section of the
/// provider menu, as `(axis, name)`.
pub fn provider_list(sections: &[MenuSection]) -> Vec<(ProviderAxis, String)> {
    sections
        .iter()
        .flat_map(|section| {
            section
                .entries
                .iter()
                .map(move |entry| (section.axis, entry.name.clone()))
        })
        .collect()
}

/// `/p`'s reply: one heading per axis (the menu's, so a turned-off axis says so), then
/// its entries numbered continuously across axes, the one in effect marked `*`.
pub fn format_provider_list(sections: &[MenuSection]) -> String {
    let count: usize = sections.iter().map(|s| s.entries.len()).sum();
    let number_width = count.to_string().len();
    let name_width = sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    let mut number = 0;
    for section in sections {
        lines.push(section.heading.clone());
        for entry in &section.entries {
            number += 1;
            let marker = if entry.checked { '*' } else { ' ' };
            lines.push(format!(
                "{marker} {number:>number_width$}  {:name_width$}  {}",
                entry.name, entry.title
            ));
        }
    }
    lines.push("Switch with /p <number>, /p <name> (translation) or /p t|d|s <name>".to_string());
    lines.join("\n")
}

/// The `(axis, name)` of entry `number` (1-based) of a list `/p` showed, or the message
/// to show when there is no such entry.
pub fn resolve_provider_number(
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

/// `/help`'s reply.
pub fn help_text() -> &'static str {
    "/l, /lang                swap the languages\n\
     /l <to>, /l <from> <to>  set the languages (names or codes)\n\
     /p, /provider            list the providers\n\
     /p <number>              use that provider for this session\n\
     /p <name>                use that translation provider for this session\n\
     /p t|d|s <name>          the same for translation, dictionary or speech\n\
     /s, /speech              speak the last phrase\n\
     /s <text>                speak the text\n\
     /ss                      speak the last translation\n\
     /clear, /cls             empty the transcript\n\
     /help, /h, /?            this list\n\
     /version, /v             the version\n\
     //text                   translate text that starts with /"
}

/// `/version`'s reply: `tagent-gui 0.15.0+018 (tagent 0.19.0+007)`, a `+000` build left
/// out as elsewhere.
pub fn version_text(gui_version: &str, tagent_version: &str) -> String {
    let display = |version: &str| version.trim_end_matches("+000").to_string();
    format!(
        "tagent-gui {} (tagent {})",
        display(gui_version),
        display(tagent_version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_menu::MenuEntry;

    fn no_names(_: &str) -> bool {
        false
    }

    fn parse_plain(text: &str) -> Option<Command<'_>> {
        parse(text, no_names)
    }

    #[test]
    fn every_command_and_alias_parses() {
        for name in ["/l", "/lang"] {
            assert_eq!(parse_plain(name), Some(Command::Languages(vec![])));
        }
        for name in ["/p", "/provider"] {
            assert_eq!(
                parse_plain(name),
                Some(Command::Provider(ProviderCommand::List))
            );
        }
        for name in ["/s", "/speech"] {
            assert_eq!(
                parse_plain(name),
                Some(Command::Speak(SpeechCommand::LastPhrase))
            );
        }
        assert_eq!(
            parse_plain("/ss"),
            Some(Command::Speak(SpeechCommand::LastTranslation))
        );
        for name in ["/clear", "/cls"] {
            assert_eq!(parse_plain(name), Some(Command::Clear));
        }
        for name in ["/help", "/h", "/?"] {
            assert_eq!(parse_plain(name), Some(Command::Help));
        }
        for name in ["/version", "/v"] {
            assert_eq!(parse_plain(name), Some(Command::Version));
        }
    }

    #[test]
    fn unknown_or_uppercase_names_are_text() {
        for text in [
            "/usr/bin/env",
            "/xyz",
            "1/2",
            "/ l",
            "/LANG",
            "/L de",
            "hello",
            "/ss2",
            "/",
        ] {
            assert_eq!(parse_plain(text), None, "{text}");
        }
    }

    #[test]
    fn a_double_slash_escapes_a_command() {
        assert_eq!(unescape("//l"), "/l");
        assert_eq!(
            parse_plain(unescape("//l")),
            Some(Command::Languages(vec![]))
        );
        assert_eq!(parse_plain("//l"), None);
        assert_eq!(unescape("/l de"), "/l de");
        assert_eq!(unescape("hello"), "hello");
    }

    #[test]
    fn arguments_are_split_on_any_whitespace() {
        assert_eq!(
            parse_plain("/l\ten  ru"),
            Some(Command::Languages(vec!["en", "ru"]))
        );
        assert_eq!(
            parse_plain("/s\nhello there"),
            Some(Command::Speak(SpeechCommand::Text("hello there")))
        );
    }

    #[test]
    fn speech_commands() {
        assert_eq!(
            parse_plain("/s  hello world "),
            Some(Command::Speak(SpeechCommand::Text("hello world")))
        );
        assert_eq!(
            parse_plain("/speech hi"),
            Some(Command::Speak(SpeechCommand::Text("hi")))
        );
        assert_eq!(
            parse_plain("/ss now"),
            Some(Command::Speak(SpeechCommand::Usage))
        );
    }

    #[test]
    fn argument_free_commands_reject_arguments() {
        assert!(matches!(parse_plain("/clear all"), Some(Command::Usage(_))));
        assert!(matches!(parse_plain("/v x"), Some(Command::Usage(_))));
        assert!(matches!(parse_plain("/? me"), Some(Command::Usage(_))));
    }

    fn provider(text: &str, names: &[&str]) -> ProviderCommand<'static> {
        let text: &'static str = Box::leak(text.to_string().into_boxed_str());
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        match parse(text, |name| names.iter().any(|n| n == name)) {
            Some(Command::Provider(command)) => command,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn provider_commands() {
        assert_eq!(provider("/p 3", &[]), ProviderCommand::Number(3));
        assert_eq!(
            provider("/p 99999999999999999999999", &[]),
            ProviderCommand::Number(0)
        );
        assert_eq!(
            provider("/p deepl", &[]),
            ProviderCommand::Switch {
                axis: ProviderAxis::Translation,
                name: "deepl"
            }
        );
        assert_eq!(
            provider("/provider d ollama", &[]),
            ProviderCommand::Switch {
                axis: ProviderAxis::Dictionary,
                name: "ollama"
            }
        );
        assert_eq!(
            provider("/p Speech google", &[]),
            ProviderCommand::Switch {
                axis: ProviderAxis::Speech,
                name: "google"
            }
        );
        // The second of two words is always a name.
        assert_eq!(
            provider("/p t 2", &[]),
            ProviderCommand::Switch {
                axis: ProviderAxis::Translation,
                name: "2"
            }
        );
        assert_eq!(
            provider("/p d", &[]),
            ProviderCommand::AxisUsage(ProviderAxis::Dictionary)
        );
        // A translation profile named like an axis word wins.
        assert_eq!(
            provider("/p d", &["d"]),
            ProviderCommand::Switch {
                axis: ProviderAxis::Translation,
                name: "d"
            }
        );
        assert_eq!(provider("/p x deepl", &[]), ProviderCommand::Usage);
        assert_eq!(provider("/p t deepl more", &[]), ProviderCommand::Usage);
    }

    fn pair(change: &LanguageChange) -> (&str, &str) {
        (change.source.as_str(), change.target.as_str())
    }

    #[test]
    fn one_argument_is_the_target_with_an_auto_source() {
        for arg in ["German", "de", "DE"] {
            let change = resolve_languages(&[arg], "en", "ru").unwrap();
            assert_eq!(pair(&change), ("auto", "de"));
            assert!(change.notices.is_empty());
        }
    }

    #[test]
    fn two_arguments_are_the_pair() {
        let change = resolve_languages(&["English", "ru"], "auto", "de").unwrap();
        assert_eq!(pair(&change), ("en", "ru"));
        let change = resolve_languages(&["auto", "CHINESE"], "en", "de").unwrap();
        assert_eq!(pair(&change), ("auto", "zh"));
        assert!(resolve_languages(&["zh-TW"], "en", "de").is_err());
    }

    #[test]
    fn an_unknown_language_changes_nothing() {
        let error = resolve_languages(&["klingon"], "en", "ru").unwrap_err();
        assert_eq!(error, "Unknown language: klingon (languages unchanged)");
        assert!(resolve_languages(&["xx", "ru"], "en", "ru").is_err());
        assert!(resolve_languages(&["en", "ru", "de"], "en", "ru").is_err());
    }

    #[test]
    fn an_auto_target_becomes_english_with_a_notice() {
        let change = resolve_languages(&["ru", "auto"], "en", "de").unwrap();
        assert_eq!(pair(&change), ("ru", "en"));
        assert_eq!(
            change.notices,
            ["Target can't be Auto; using English instead"]
        );
        let change = resolve_languages(&["Auto"], "en", "de").unwrap();
        assert_eq!(pair(&change), ("auto", "en"));
    }

    #[test]
    fn swapping_an_auto_source_uses_english() {
        let change = resolve_languages(&[], "auto", "ru").unwrap();
        assert_eq!(pair(&change), ("ru", "en"));
        assert_eq!(
            change.notices,
            ["Source was Auto; using English as the new target"]
        );
        let change = resolve_languages(&[], "en", "ru").unwrap();
        assert_eq!(pair(&change), ("ru", "en"));
        assert!(change.notices.is_empty());
    }

    #[test]
    fn a_same_language_pair_gets_a_note() {
        let change = resolve_languages(&["en", "EN"], "auto", "ru").unwrap();
        assert_eq!(pair(&change), ("en", "en"));
        assert_eq!(
            change.notices,
            ["Note: source and target are the same language"]
        );
    }

    fn sections() -> Vec<MenuSection> {
        let entry = |name: &str, title: &str, checked: bool| MenuEntry {
            name: name.to_string(),
            title: title.to_string(),
            checked,
        };
        vec![
            MenuSection {
                axis: ProviderAxis::Translation,
                heading: "Translation".to_string(),
                entries: vec![
                    entry("google", "Google Translate", false),
                    entry("deepl", "DeepL  ⚠ api_key", false),
                    entry("ollama", "OpenAI-compatible (ollama)", true),
                ],
            },
            MenuSection {
                axis: ProviderAxis::Dictionary,
                heading: "Dictionary (off)".to_string(),
                entries: vec![entry("google", "Google Dictionary", true)],
            },
            MenuSection {
                axis: ProviderAxis::Speech,
                heading: "Speech".to_string(),
                entries: vec![entry("google", "Google TTS", true)],
            },
        ]
    }

    #[test]
    fn the_provider_list_numbers_across_axes_and_marks_the_one_in_effect() {
        assert_eq!(
            format_provider_list(&sections()),
            "Translation\n\
             \x20 1  google  Google Translate\n\
             \x20 2  deepl   DeepL  ⚠ api_key\n\
             * 3  ollama  OpenAI-compatible (ollama)\n\
             Dictionary (off)\n\
             * 4  google  Google Dictionary\n\
             Speech\n\
             * 5  google  Google TTS\n\
             Switch with /p <number>, /p <name> (translation) or /p t|d|s <name>"
        );
        assert_eq!(
            provider_list(&sections()),
            [
                (ProviderAxis::Translation, "google".to_string()),
                (ProviderAxis::Translation, "deepl".to_string()),
                (ProviderAxis::Translation, "ollama".to_string()),
                (ProviderAxis::Dictionary, "google".to_string()),
                (ProviderAxis::Speech, "google".to_string()),
            ]
        );
    }

    #[test]
    fn provider_numbers_out_of_range_are_errors() {
        let list = provider_list(&sections());
        assert_eq!(
            resolve_provider_number(&list, 4),
            Ok((ProviderAxis::Dictionary, "google".to_string()))
        );
        assert_eq!(
            resolve_provider_number(&list, 0),
            Err("No provider number 0: choose 1-5 (/p shows the list)".to_string())
        );
        assert!(resolve_provider_number(&list, 6).is_err());
        assert_eq!(
            resolve_provider_number(&[], 1),
            Err("No providers to choose from".to_string())
        );
    }

    #[test]
    fn help_lists_every_command_name() {
        let help = help_text();
        for name in COMMAND_NAMES {
            assert!(help.contains(name), "{name}");
        }
    }

    #[test]
    fn version_text_leaves_out_a_zero_build() {
        assert_eq!(
            version_text("0.15.0+018", "0.19.0+000"),
            "tagent-gui 0.15.0+018 (tagent 0.19.0)"
        );
    }
}
