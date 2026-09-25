//! Dictionary lookup formatting for single-word input.
//!
//! The article layout itself (header, part-of-speech labels, definitions,
//! synonyms) comes from [`tagent::article`], shared with `tagent-cli`; this
//! module adds the GUI's own Markdown-template rendering on top of it. The
//! small helpers below are still duplicated from `tagent-cli`, since
//! `tagent-gui` never depends on `tagent-cli`.

use crate::styled::{self, Role};
use tagent::providers::DictionaryEntry;

/// Check if text is a single word (no spaces, punctuation at edges allowed).
pub fn is_single_word(text: &str) -> bool {
    let cleaned = text.trim_matches(|c: char| !c.is_alphabetic());
    !cleaned.is_empty()
        && !cleaned.contains(' ')
        && cleaned
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

/// Returns a localized notice to show when a spelling correction was applied.
pub fn correction_notice(corrected_word: &str, target_lang: &str) -> String {
    let phrase = match target_lang {
        "ru" => "Показан перевод слова",
        "es" => "Mostrando traducción de la palabra",
        "fr" => "Traduction affichée pour le mot",
        "de" => "Übersetzung angezeigt für das Wort",
        "it" => "Traduzione mostrata per la parola",
        "pt" => "Tradução mostrada para a palavra",
        "zh" => "显示单词翻译",
        _ => "Showing translation for word",
    };
    format!("{} {}", phrase, corrected_word)
}

pub use tagent::article::primary_line;
use tagent::article::{self, Line};

/// Maps a [`tagent::article::Role`] onto this app's own highlighting [`Role`].
fn style_role(role: article::Role) -> Role {
    match role {
        article::Role::Header => Role::Header,
        article::Role::PartOfSpeech => Role::PartOfSpeech,
        article::Role::Synonym => Role::Synonym,
        _ => Role::Plain,
    }
}

/// Lays `entry` out as role-tagged lines -- see [`tagent::article::article_lines`].
/// Both [`format_dictionary_entry`] and the Stage 13 styled renderer
/// ([`to_template`]) are derived from this one traversal, so they can never drift
/// apart.
pub fn article_lines(
    entry: &DictionaryEntry,
    target_lang: &str,
    primary_translation: Option<&str>,
) -> Vec<Line> {
    article::article_lines(entry, target_lang, primary_translation)
}

/// Renders [`article_lines`] as a Stage 13 template: two-NBSP indent (a
/// literal two-space indent would be stripped by Markdown), each span
/// escaped and wrapped in its own role's `<font color="@role">` (or left
/// plain for [`Role::Header`]/[`Role::Plain`]), lines joined with `\n` --
/// a single line break within one paragraph, not a blank-paragraph gap.
pub fn to_template(lines: &[Line]) -> String {
    article::render_with(lines, "\u{a0}\u{a0}", |role, text| {
        styled::span(style_role(role), &styled::escape_markdown(text))
    })
}

/// Formats a dictionary entry for display.
///
/// Unlike `tagent-cli`'s CLI-mode formatting, this never repeats the looked-up
/// word as its own line -- `tagent-gui`'s two-pane phrase/translation layout
/// already shows it on the phrase line above, so restating it here would be
/// redundant. `primary_translation` (the plain-translation result fetched
/// concurrently alongside the dictionary lookup) is used as the header line
/// when available, falling back to the first part-of-speech's first
/// definition text otherwise -- see [`primary_line`].
///
/// Feeds the popup and `translation_raw` (Stage 6/9), so this stays
/// byte-identical to what it returned before Stage 13 introduced
/// [`to_template`] alongside it -- see `format_dictionary_entry_golden_output`.
pub fn format_dictionary_entry(
    entry: &DictionaryEntry,
    target_lang: &str,
    primary_translation: Option<&str>,
) -> String {
    article::to_plain(&article_lines(entry, target_lang, primary_translation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tagent::providers::{Definition, PartOfSpeechEntry};

    #[test]
    fn single_word_accepts_plain_word() {
        assert!(is_single_word("violent"));
    }

    #[test]
    fn single_word_accepts_hyphenated_word() {
        assert!(is_single_word("well-known"));
    }

    #[test]
    fn single_word_accepts_contraction() {
        assert!(is_single_word("don't"));
    }

    #[test]
    fn single_word_accepts_edge_punctuation() {
        assert!(is_single_word("\"violent\","));
    }

    #[test]
    fn single_word_rejects_phrase() {
        assert!(!is_single_word("hello world"));
    }

    #[test]
    fn single_word_rejects_empty_or_punctuation_only() {
        assert!(!is_single_word(""));
        assert!(!is_single_word("---"));
    }

    #[test]
    fn correction_notice_russian() {
        assert_eq!(
            correction_notice("violent", "ru"),
            "Показан перевод слова violent"
        );
    }

    #[test]
    fn correction_notice_english_fallback() {
        assert_eq!(
            correction_notice("violent", "en"),
            "Showing translation for word violent"
        );
    }

    fn sample_entry() -> DictionaryEntry {
        DictionaryEntry::new(
            "violent",
            vec![PartOfSpeechEntry::new(
                "adjective",
                vec![
                    Definition::new(
                        "using or involving physical force",
                        vec!["fierce".to_string(), "brutal".to_string()],
                    ),
                    Definition::new("extremely strong", vec![]),
                ],
            )],
        )
    }

    #[test]
    fn primary_line_prefers_primary_translation() {
        assert_eq!(
            primary_line(&sample_entry(), Some("furious")),
            Some("furious".to_string())
        );
    }

    #[test]
    fn primary_line_falls_back_to_first_definition() {
        assert_eq!(
            primary_line(&sample_entry(), None),
            Some("using or involving physical force".to_string())
        );
    }

    #[test]
    fn primary_line_returns_none_when_no_definitions_and_no_primary_translation() {
        let empty_entry = DictionaryEntry::new("violent", vec![]);
        assert_eq!(primary_line(&empty_entry, None), None);
    }

    #[test]
    fn format_dictionary_entry_uses_primary_translation_as_header() {
        let formatted = format_dictionary_entry(&sample_entry(), "en", Some("furious"));
        let lines: Vec<&str> = formatted.lines().collect();
        assert_eq!(lines[0], "furious");
        assert_eq!(lines[1], "Adjective");
    }

    #[test]
    fn format_dictionary_entry_falls_back_to_first_definition_when_no_primary_translation() {
        let formatted = format_dictionary_entry(&sample_entry(), "en", None);
        let lines: Vec<&str> = formatted.lines().collect();
        assert_eq!(lines[0], "using or involving physical force");
    }

    /// Pins `format_dictionary_entry`'s exact output (Stage 13 step 0): the plain
    /// formatter feeds the popup and `translation_raw`, so it must stay byte-identical
    /// while a separate styled renderer is introduced next to it.
    #[test]
    fn format_dictionary_entry_golden_output() {
        assert_eq!(
            format_dictionary_entry(&sample_entry(), "en", Some("furious")),
            "furious\nAdjective\n  using or involving physical force [fierce, brutal]\n  extremely strong"
        );
        assert_eq!(
            format_dictionary_entry(&sample_entry(), "ru", None),
            "using or involving physical force\nПрилагательное\n  using or involving physical force [fierce, brutal]\n  extremely strong"
        );
    }

    #[test]
    fn format_dictionary_entry_formats_synonyms_and_plain_definitions() {
        let formatted = format_dictionary_entry(&sample_entry(), "en", Some("furious"));
        assert!(formatted.contains("  using or involving physical force [fierce, brutal]"));
        assert!(formatted.contains("  extremely strong"));
        assert!(!formatted.contains("extremely strong ["));
    }

    /// `to_template`'s output, once its `<font>` tags are stripped and its
    /// backslash-escapes and NBSP indentation are undone, must equal `to_plain`'s --
    /// the two are derived from the same `article_lines` traversal (Stage 13, decision
    /// 2) and must never drift apart.
    fn assert_template_matches_plain(
        entry: &DictionaryEntry,
        target_lang: &str,
        primary_translation: Option<&str>,
    ) {
        let lines = article_lines(entry, target_lang, primary_translation);
        let plain = article::to_plain(&lines);
        let template = to_template(&lines);
        let normalized = styled::strip_template(&template).replace('\u{a0}', " ");
        assert_eq!(
            normalized, plain,
            "to_template/to_plain diverged for target_lang {target_lang:?}"
        );
    }

    #[test]
    fn to_template_matches_to_plain_after_stripping_and_unescaping() {
        assert_template_matches_plain(&sample_entry(), "en", Some("furious"));
        assert_template_matches_plain(&sample_entry(), "ru", None);

        let no_synonyms = DictionaryEntry::new(
            "quick",
            vec![PartOfSpeechEntry::new(
                "adjective",
                vec![Definition::new("fast", vec![])],
            )],
        );
        assert_template_matches_plain(&no_synonyms, "en", None);

        let hostile = DictionaryEntry::new(
            "test",
            vec![PartOfSpeechEntry::new(
                "noun",
                vec![Definition::new(
                    "a *test* <thing> [bracket]",
                    vec!["exam*ple".to_string()],
                )],
            )],
        );
        assert_template_matches_plain(&hostile, "en", Some("*primary*"));

        let empty = DictionaryEntry::new("x", vec![]);
        assert_template_matches_plain(&empty, "en", None);
    }
}
