//! Dictionary lookup formatting for single-word input.
//!
//! Duplicated independently from `tagent-cli`'s `translator.rs`/`config.rs`
//! (Open Question 3, `.debug/tagent-gui development plan.md`) rather than
//! shared, since `tagent-gui` never depends on `tagent-cli`.

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

/// Picks the single line that best represents `entry` on its own -- the plain
/// translation fetched alongside the dictionary lookup when available, or the
/// first part-of-speech's first definition text otherwise.
///
/// Used both as [`format_dictionary_entry`]'s header line and, unmodified, as
/// the text a dictionary-hit transcript entry's translation speaker button
/// (Stage 10) reads aloud -- never the full formatted block (part-of-speech
/// headers, synonym lists), which would read strangely out loud.
pub fn primary_line(entry: &DictionaryEntry, primary_translation: Option<&str>) -> Option<String> {
    primary_translation.map(|s| s.to_string()).or_else(|| {
        entry
            .definitions
            .first()
            .and_then(|pos| pos.definitions.first())
            .map(|def| def.text.clone())
    })
}

/// One span of text within an [`article_lines`] [`Line`], tagged with the
/// [`Role`] it should be highlighted with (Stage 13).
pub struct Span {
    /// The span's semantic role -- see [`crate::styled::Role`].
    pub role: Role,
    /// The span's raw, unescaped text.
    pub text: String,
}

impl Span {
    fn new(role: Role, text: impl Into<String>) -> Span {
        Span {
            role,
            text: text.into(),
        }
    }
}

/// One line of a dictionary article: a header, a part-of-speech label, or an
/// indented definition -- see [`article_lines`].
pub struct Line {
    /// Whether this line gets the two-space (or, in a template, two-NBSP)
    /// definition indent.
    pub indent: bool,
    /// The line's spans, concatenated in order with no separator (any spacing
    /// between them is baked into a span's own text, e.g. a trailing space
    /// before a synonym bracket).
    pub spans: Vec<Span>,
}

/// Builds `entry`'s single traversal: a header line (the primary
/// translation, or the first definition text -- see [`primary_line`] -- when
/// there isn't one), then for each part of speech a label line followed by
/// one indented line per definition, with `[synonyms]` as its own
/// [`Role::Synonym`] span. Both [`format_dictionary_entry`] (`to_plain`) and
/// the Stage 13 styled renderer (`to_template`) are derived from this one
/// traversal, so they can never drift apart.
pub fn article_lines(
    entry: &DictionaryEntry,
    target_lang: &str,
    primary_translation: Option<&str>,
) -> Vec<Line> {
    let mut lines = Vec::new();

    if let Some(h) = primary_line(entry, primary_translation) {
        lines.push(Line {
            indent: false,
            spans: vec![Span::new(Role::Header, h)],
        });
    }

    for pos_entry in &entry.definitions {
        let pos_full = get_full_part_of_speech(&pos_entry.part_of_speech, target_lang);
        lines.push(Line {
            indent: false,
            spans: vec![Span::new(Role::PartOfSpeech, pos_full.to_string())],
        });

        for def in &pos_entry.definitions {
            let spans = if def.synonyms.is_empty() {
                vec![Span::new(Role::Plain, def.text.clone())]
            } else {
                vec![
                    Span::new(Role::Plain, format!("{} ", def.text)),
                    Span::new(Role::Synonym, format!("[{}]", def.synonyms.join(", "))),
                ]
            };
            lines.push(Line {
                indent: true,
                spans,
            });
        }
    }

    lines
}

/// Renders [`article_lines`] as plain text: two-space indent, spans
/// concatenated with no highlighting, lines joined with `\n`.
fn to_plain(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|line| {
            let indent = if line.indent { "  " } else { "" };
            let body: String = line.spans.iter().map(|s| s.text.as_str()).collect();
            format!("{indent}{body}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders [`article_lines`] as a Stage 13 template: two-NBSP indent (a
/// literal two-space indent would be stripped by Markdown), each span
/// escaped and wrapped in its own role's `<font color="@role">` (or left
/// plain for [`Role::Header`]/[`Role::Plain`]), lines joined with `\n` --
/// a single line break within one paragraph, not a blank-paragraph gap.
pub fn to_template(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|line| {
            let indent = if line.indent { "\u{a0}\u{a0}" } else { "" };
            let body: String = line
                .spans
                .iter()
                .map(|s| styled::span(s.role, &styled::escape_markdown(&s.text)))
                .collect();
            format!("{indent}{body}")
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    to_plain(&article_lines(entry, target_lang, primary_translation))
}

/// Get full part of speech name in target language.
fn get_full_part_of_speech(pos: &str, target_lang: &str) -> &'static str {
    let pos_lower = pos.to_lowercase();

    match target_lang {
        "ru" => match pos_lower.as_str() {
            "noun" | "существительное" => "Существительное",
            "verb" | "глагол" => "Глагол",
            "adjective" | "прилагательное" => "Прилагательное",
            "adverb" | "наречие" => "Наречие",
            "preposition" | "предлог" => "Предлог",
            "conjunction" | "союз" => "Союз",
            "pronoun" | "местоимение" => "Местоимение",
            "interjection" | "междометие" => "Междометие",
            "article" | "артикль" => "Артикль",
            "determiner" | "определитель" => "Определитель",
            "participle" | "причастие" => "Причастие",
            _ => "Прочее",
        },
        "es" => match pos_lower.as_str() {
            "noun" => "Sustantivo",
            "verb" => "Verbo",
            "adjective" => "Adjetivo",
            "adverb" => "Adverbio",
            "preposition" => "Preposición",
            "conjunction" => "Conjunción",
            "pronoun" => "Pronombre",
            "interjection" => "Interjección",
            "article" => "Artículo",
            "determiner" => "Determinante",
            "participle" => "Participio",
            _ => "Otro",
        },
        "fr" => match pos_lower.as_str() {
            "noun" => "Nom",
            "verb" => "Verbe",
            "adjective" => "Adjectif",
            "adverb" => "Adverbe",
            "preposition" => "Préposition",
            "conjunction" => "Conjonction",
            "pronoun" => "Pronom",
            "interjection" => "Interjection",
            "article" => "Article",
            "determiner" => "Déterminant",
            "participle" => "Participe",
            _ => "Autre",
        },
        "de" => match pos_lower.as_str() {
            "noun" => "Substantiv",
            "verb" => "Verb",
            "adjective" => "Adjektiv",
            "adverb" => "Adverb",
            "preposition" => "Präposition",
            "conjunction" => "Konjunktion",
            "pronoun" => "Pronomen",
            "interjection" => "Interjektion",
            "article" => "Artikel",
            "determiner" => "Bestimmungswort",
            "participle" => "Partizip",
            _ => "Andere",
        },
        "it" => match pos_lower.as_str() {
            "noun" => "Sostantivo",
            "verb" => "Verbo",
            "adjective" => "Aggettivo",
            "adverb" => "Avverbio",
            "preposition" => "Preposizione",
            "conjunction" => "Congiunzione",
            "pronoun" => "Pronome",
            "interjection" => "Interiezione",
            "article" => "Articolo",
            "determiner" => "Determinante",
            "participle" => "Participio",
            _ => "Altro",
        },
        "pt" => match pos_lower.as_str() {
            "noun" => "Substantivo",
            "verb" => "Verbo",
            "adjective" => "Adjetivo",
            "adverb" => "Advérbio",
            "preposition" => "Preposição",
            "conjunction" => "Conjunção",
            "pronoun" => "Pronome",
            "interjection" => "Interjeição",
            "article" => "Artigo",
            "determiner" => "Determinante",
            "participle" => "Particípio",
            _ => "Outro",
        },
        "zh" => match pos_lower.as_str() {
            "noun" => "名词",
            "verb" => "动词",
            "adjective" => "形容词",
            "adverb" => "副词",
            "preposition" => "介词",
            "conjunction" => "连词",
            "pronoun" => "代词",
            "interjection" => "感叹词",
            "article" => "冠词",
            "determiner" => "限定词",
            "participle" => "分词",
            _ => "其他",
        },
        // English fallback (default)
        _ => match pos_lower.as_str() {
            "noun" | "существительное" => "Noun",
            "verb" | "глагол" => "Verb",
            "adjective" | "прилагательное" => "Adjective",
            "adverb" | "наречие" => "Adverb",
            "preposition" | "предлог" => "Preposition",
            "conjunction" | "союз" => "Conjunction",
            "pronoun" | "местоимение" => "Pronoun",
            "interjection" | "междометие" => "Interjection",
            "article" | "артикль" => "Article",
            "determiner" | "определитель" => "Determiner",
            "participle" | "причастие" => "Participle",
            _ => "Other",
        },
    }
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
        let plain = to_plain(&lines);
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
