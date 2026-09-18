//! Dictionary lookup formatting for single-word input.
//!
//! Duplicated independently from `tagent-cli`'s `translator.rs`/`config.rs`
//! (Open Question 3, `.debug/tagent-gui development plan.md`) rather than
//! shared, since `tagent-gui` never depends on `tagent-cli`.

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

/// Formats a dictionary entry for display.
///
/// Unlike `tagent-cli`'s CLI-mode formatting, this never repeats the looked-up
/// word as its own line -- `tagent-gui`'s two-pane phrase/translation layout
/// already shows it on the phrase line above, so restating it here would be
/// redundant. `primary_translation` (the plain-translation result fetched
/// concurrently alongside the dictionary lookup) is used as the header line
/// when available, falling back to the first part-of-speech's first
/// definition text otherwise.
pub fn format_dictionary_entry(
    entry: &DictionaryEntry,
    target_lang: &str,
    primary_translation: Option<&str>,
) -> String {
    let mut result = Vec::new();

    let header = primary_translation.map(|s| s.to_string()).or_else(|| {
        entry
            .definitions
            .first()
            .and_then(|pos| pos.definitions.first())
            .map(|def| def.text.clone())
    });
    if let Some(h) = header {
        result.push(h);
    }

    for pos_entry in &entry.definitions {
        let pos_full = get_full_part_of_speech(&pos_entry.part_of_speech, target_lang);
        result.push(pos_full.to_string());

        for def in &pos_entry.definitions {
            if !def.synonyms.is_empty() {
                result.push(format!("  {} [{}]", def.text, def.synonyms.join(", ")));
            } else {
                result.push(format!("  {}", def.text));
            }
        }
    }

    result.join("\n")
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
        DictionaryEntry {
            word: "violent".to_string(),
            corrected_word: None,
            definitions: vec![PartOfSpeechEntry {
                part_of_speech: "adjective".to_string(),
                definitions: vec![
                    Definition {
                        text: "using or involving physical force".to_string(),
                        synonyms: vec!["fierce".to_string(), "brutal".to_string()],
                    },
                    Definition {
                        text: "extremely strong".to_string(),
                        synonyms: vec![],
                    },
                ],
            }],
        }
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

    #[test]
    fn format_dictionary_entry_formats_synonyms_and_plain_definitions() {
        let formatted = format_dictionary_entry(&sample_entry(), "en", Some("furious"));
        assert!(formatted.contains("  using or involving physical force [fierce, brutal]"));
        assert!(formatted.contains("  extremely strong"));
        assert!(!formatted.contains("extremely strong ["));
    }
}
