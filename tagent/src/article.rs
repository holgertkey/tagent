//! Display layout for a [`DictionaryEntry`], shared by the bundled applications.
//!
//! [`article_lines`] turns an entry into a small, role-tagged intermediate
//! representation: a header line, then a label line per part of speech, each
//! followed by one indented line per definition. Every piece of text carries a
//! [`Role`], so an application can highlight it however its output medium
//! allows (ANSI colors in a terminal, rich-text markup in a GUI) with
//! [`render_with`], while [`to_plain`] gives the uncolored text suitable for a
//! clipboard or a log file. Both come from the same traversal, so a colored
//! rendering never drifts from the plain one.
//!
//! # Examples
//!
//! ```
//! use tagent::article::{article_lines, render_with, to_plain, Role};
//! use tagent::providers::{Definition, DictionaryEntry, PartOfSpeechEntry};
//!
//! let entry = DictionaryEntry::new(
//!     "violent",
//!     vec![PartOfSpeechEntry::new(
//!         "adjective",
//!         vec![Definition::new("жестокий", vec!["brutal".to_string()])],
//!     )],
//! );
//! let lines = article_lines(&entry, "ru", Some("насильственный"));
//!
//! assert_eq!(
//!     to_plain(&lines),
//!     "насильственный\nПрилагательное\n  жестокий [brutal]"
//! );
//!
//! // Upper-case part-of-speech labels, leave everything else alone.
//! let shouted = render_with(&lines, "  ", |role, text| match role {
//!     Role::PartOfSpeech => text.to_uppercase(),
//!     _ => text.to_string(),
//! });
//! assert_eq!(shouted, "насильственный\nПРИЛАГАТЕЛЬНОЕ\n  жестокий [brutal]");
//! ```

use crate::providers::DictionaryEntry;

/// The semantic role of a [`Span`] within a dictionary article.
///
/// `#[non_exhaustive]`: match it with a wildcard arm, since new roles may be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// Ordinary definition text.
    Plain,
    /// The article's header line: the primary translation of the word (see [`primary_line`]).
    Header,
    /// A localized part-of-speech label line, e.g. `"Noun"` or `"Существительное"`.
    PartOfSpeech,
    /// The `[synonym, synonym]` bracket after a definition.
    Synonym,
}

/// One span of text within a [`Line`], tagged with the [`Role`] it plays.
///
/// `#[non_exhaustive]`: spans are produced by [`article_lines`], not built by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Span {
    /// The span's semantic role.
    pub role: Role,
    /// The span's raw text, with no escaping or markup.
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

/// One line of a dictionary article: the header, a part-of-speech label, or an
/// indented definition.
///
/// `#[non_exhaustive]`: lines are produced by [`article_lines`], not built by callers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Line {
    /// Whether this is a definition line, rendered with an indent.
    pub indent: bool,
    /// The line's spans, concatenated in order with no separator: any spacing between
    /// them is part of a span's own text (e.g. the space before a synonym bracket).
    pub spans: Vec<Span>,
}

/// Picks the single line that best represents `entry` on its own.
///
/// That is `primary_translation` (a plain translation of the word, typically fetched
/// alongside the dictionary lookup) when available, or the first part of speech's
/// first definition otherwise; `None` when there is neither.
pub fn primary_line(entry: &DictionaryEntry, primary_translation: Option<&str>) -> Option<String> {
    primary_translation.map(|s| s.to_string()).or_else(|| {
        entry
            .definitions
            .first()
            .and_then(|pos| pos.definitions.first())
            .map(|def| def.text.clone())
    })
}

/// Lays `entry` out as role-tagged lines.
///
/// The result is a header line (see [`primary_line`]; omitted when there is none),
/// then for each part of speech a label line (localized into `target_lang` by
/// [`part_of_speech_label`]) followed by one indented line per definition, whose
/// synonyms, if any, form a separate [`Role::Synonym`] span.
///
/// See the [module documentation](self) for an example.
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
        let pos_full = part_of_speech_label(&pos_entry.part_of_speech, target_lang);
        lines.push(Line {
            indent: false,
            spans: vec![Span::new(Role::PartOfSpeech, pos_full)],
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

/// Renders `lines`, passing every span through `paint`.
///
/// Indented lines are prefixed with `indent`, spans are concatenated with no
/// separator, and lines are joined with `\n`. `paint` receives each span's role and
/// raw text and returns what to emit for it — the text wrapped in color codes or
/// markup, escaped, or unchanged.
///
/// See the [module documentation](self) for an example.
pub fn render_with<F>(lines: &[Line], indent: &str, mut paint: F) -> String
where
    F: FnMut(Role, &str) -> String,
{
    lines
        .iter()
        .map(|line| {
            let mut out = String::new();
            if line.indent {
                out.push_str(indent);
            }
            for span in &line.spans {
                out.push_str(&paint(span.role, &span.text));
            }
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders `lines` as plain text: a two-space indent and no highlighting.
///
/// See the [module documentation](self) for an example.
pub fn to_plain(lines: &[Line]) -> String {
    render_with(lines, "  ", |_, text| text.to_string())
}

/// Returns the full name of part of speech `pos` in language `target_lang`.
///
/// `pos` is a provider's lowercase English label (`"noun"`, `"verb"`, …, see
/// [`PartOfSpeechEntry::part_of_speech`](crate::providers::PartOfSpeechEntry::part_of_speech)),
/// matched case-insensitively. Supported languages are `ru`, `es`, `fr`, `de`, `it`,
/// `pt` and `zh`; any other code gets English. An unknown label becomes that
/// language's word for "other".
///
/// # Examples
///
/// ```
/// use tagent::article::part_of_speech_label;
///
/// assert_eq!(part_of_speech_label("noun", "de"), "Substantiv");
/// assert_eq!(part_of_speech_label("Verb", "en"), "Verb");
/// assert_eq!(part_of_speech_label("gerund", "ru"), "Прочее");
/// ```
pub fn part_of_speech_label(pos: &str, target_lang: &str) -> &'static str {
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
    use crate::providers::{Definition, PartOfSpeechEntry};

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

    /// Pins the plain layout both applications have always produced: it feeds their
    /// clipboard, history and popup text, so it must stay byte-identical.
    #[test]
    fn to_plain_golden_output() {
        assert_eq!(
            to_plain(&article_lines(&sample_entry(), "en", Some("furious"))),
            "furious\nAdjective\n  using or involving physical force [fierce, brutal]\n  extremely strong"
        );
        assert_eq!(
            to_plain(&article_lines(&sample_entry(), "ru", None)),
            "using or involving physical force\nПрилагательное\n  using or involving physical force [fierce, brutal]\n  extremely strong"
        );
    }

    #[test]
    fn to_plain_of_empty_entry_is_empty() {
        let empty_entry = DictionaryEntry::new("x", vec![]);
        assert_eq!(to_plain(&article_lines(&empty_entry, "en", None)), "");
    }

    #[test]
    fn article_lines_tags_every_span_with_its_role() {
        let lines = article_lines(&sample_entry(), "en", Some("furious"));
        let roles: Vec<Vec<Role>> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.role).collect())
            .collect();
        assert_eq!(
            roles,
            vec![
                vec![Role::Header],
                vec![Role::PartOfSpeech],
                vec![Role::Plain, Role::Synonym],
                vec![Role::Plain],
            ]
        );
        let indents: Vec<bool> = lines.iter().map(|l| l.indent).collect();
        assert_eq!(indents, vec![false, false, true, true]);
    }

    #[test]
    fn render_with_passes_each_span_to_paint_and_uses_the_given_indent() {
        let lines = article_lines(&sample_entry(), "en", Some("furious"));
        let rendered = render_with(&lines, ">>", |role, text| format!("<{role:?}>{text}"));
        assert_eq!(
            rendered,
            "<Header>furious\n<PartOfSpeech>Adjective\n>><Plain>using or involving physical force <Synonym>[fierce, brutal]\n>><Plain>extremely strong"
        );
    }

    #[test]
    fn part_of_speech_label_is_case_insensitive_and_falls_back() {
        assert_eq!(part_of_speech_label("NOUN", "ru"), "Существительное");
        assert_eq!(part_of_speech_label("noun", "xx"), "Noun");
        assert_eq!(part_of_speech_label("gerund", "fr"), "Autre");
    }
}
