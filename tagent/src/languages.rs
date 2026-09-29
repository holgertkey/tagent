//! Human-readable language name ↔ BCP-47 code mapping.
//!
//! [`LANGUAGES`] is the single table behind [`name_to_code`] and [`code_to_name`], so an
//! application can also build a language picker from it.

/// A language known to [`name_to_code`] and [`code_to_name`].
///
/// # Examples
///
/// ```
/// use tagent::languages::LANGUAGES;
///
/// let russian = LANGUAGES.iter().find(|l| l.code == "ru").unwrap();
/// assert_eq!(russian.name, "Russian");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Language {
    /// The lowercase BCP-47 code, e.g. `"ru"`.
    pub code: &'static str,
    /// The English name, capitalized, e.g. `"Russian"`.
    pub name: &'static str,
}

/// Builds a [`Language`] table entry (keeps [`LANGUAGES`] one line per language).
const fn lang(code: &'static str, name: &'static str) -> Language {
    Language { code, name }
}

/// Every language [`name_to_code`] and [`code_to_name`] know, in display order.
///
/// `"auto"` (auto-detect) is not listed: it is a source-only choice, not a language.
/// Both functions still accept it (`"Auto"` ↔ `"auto"`), so a source-language picker
/// usually offers `"Auto"` followed by these names.
///
/// # Examples
///
/// ```
/// use tagent::languages::{self, LANGUAGES};
///
/// let targets: Vec<&str> = LANGUAGES.iter().map(|l| l.name).collect();
/// assert_eq!(targets[0], "English");
/// assert!(!targets.contains(&"Auto"));
///
/// for language in LANGUAGES {
///     assert_eq!(languages::name_to_code(language.name), language.code);
/// }
/// ```
pub const LANGUAGES: &[Language] = &[
    lang("en", "English"),
    lang("ru", "Russian"),
    lang("es", "Spanish"),
    lang("fr", "French"),
    lang("de", "German"),
    lang("zh", "Chinese"),
    lang("ja", "Japanese"),
    lang("ko", "Korean"),
    lang("it", "Italian"),
    lang("pt", "Portuguese"),
    lang("nl", "Dutch"),
    lang("pl", "Polish"),
    lang("tr", "Turkish"),
    lang("ar", "Arabic"),
    lang("hi", "Hindi"),
];

/// Maps a human-readable language name (e.g. `"Russian"`) to its BCP-47 code
/// (e.g. `"ru"`), ignoring case.
///
/// Returns `"auto"` for `"Auto"` and the input unchanged for unknown names (which may
/// already be a code).
///
/// # Examples
///
/// ```
/// assert_eq!(tagent::languages::name_to_code("Russian"), "ru");
/// assert_eq!(tagent::languages::name_to_code("klingon"), "klingon");
/// ```
pub fn name_to_code(language: &str) -> &str {
    if language.eq_ignore_ascii_case("auto") {
        return "auto";
    }
    LANGUAGES
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(language))
        .map_or(language, |l| l.code)
}

/// Reverse of [`name_to_code`]: maps a BCP-47 code (e.g. `"ru"`) to its
/// human-readable name (e.g. `"Russian"`), ignoring case.
///
/// Returns the code as-is if no matching name is found.
///
/// # Examples
///
/// ```
/// assert_eq!(tagent::languages::code_to_name("ru"), "Russian");
/// ```
pub fn code_to_name(code: &str) -> &str {
    if code.eq_ignore_ascii_case("auto") {
        return "Auto";
    }
    LANGUAGES
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(code))
        .map_or(code, |l| l.name)
}

/// The code [`LANGUAGES`] lists for `input`, a language code or name in any case
/// (`"Russian"`, `"ru"` and `"RU"` all give `"ru"`); `"auto"` (any case) gives `"auto"`.
///
/// Returns `None` for anything the table doesn't list, so a caller can tell a known
/// language from a code it passes through as it is. A caller that needs a concrete
/// language (a target) checks for `"auto"` itself.
///
/// # Examples
///
/// ```
/// use tagent::languages::language_code;
///
/// assert_eq!(language_code("Russian"), Some("ru"));
/// assert_eq!(language_code("DE"), Some("de"));
/// assert_eq!(language_code("Auto"), Some("auto"));
/// assert_eq!(language_code("uk"), None);
/// ```
pub fn language_code(input: &str) -> Option<&'static str> {
    if input.eq_ignore_ascii_case("auto") {
        return Some("auto");
    }
    LANGUAGES
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(input) || l.name.eq_ignore_ascii_case(input))
        .map(|l| l.code)
}

/// The code of the first of `locales` (most preferred first) whose language
/// [`LANGUAGES`] lists; `None` when none is.
///
/// A locale is a BCP 47 tag (`"ru-UA"`, `"zh-Hans-CN"`, as the `sys-locale` crate
/// returns them) or a POSIX locale (`"de_DE.UTF-8"`, `"sr_RS@latin"`); only its primary
/// language subtag counts. The fallback for `None` is the caller's choice.
///
/// # Examples
///
/// ```
/// use tagent::languages::language_for_locales;
///
/// // Esperanto isn't listed, so the next preference wins.
/// assert_eq!(language_for_locales(["eo", "de-AT", "en-US"]), Some("de"));
/// assert_eq!(language_for_locales(["pt_BR.UTF-8"]), Some("pt"));
/// assert_eq!(language_for_locales(Vec::<String>::new()), None);
/// ```
pub fn language_for_locales(
    locales: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<&'static str> {
    locales.into_iter().find_map(|locale| {
        let primary = locale
            .as_ref()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default();
        LANGUAGES
            .iter()
            .find(|l| l.code.eq_ignore_ascii_case(primary))
            .map(|l| l.code)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_code_accepts_codes_and_names_in_any_case() {
        for input in ["ru", "RU", "Russian", "rUsSiAn"] {
            assert_eq!(language_code(input), Some("ru"), "{input}");
        }
        for language in LANGUAGES {
            assert_eq!(language_code(language.code), Some(language.code));
            assert_eq!(language_code(language.name), Some(language.code));
        }
    }

    #[test]
    fn language_code_maps_auto_and_rejects_unlisted_values() {
        assert_eq!(language_code("Auto"), Some("auto"));
        assert_eq!(language_code("AUTO"), Some("auto"));
        for input in ["uk", "zh-TW", "Klingon", "", "English "] {
            assert_eq!(language_code(input), None, "{input:?}");
        }
    }

    #[test]
    fn language_for_locales_takes_the_first_listed_language() {
        assert_eq!(language_for_locales(["ru-UA"]), Some("ru"));
        assert_eq!(language_for_locales(["en-US", "ru"]), Some("en"));
        // An unlisted language is skipped in favor of the next preference.
        assert_eq!(language_for_locales(["eo", "de-AT"]), Some("de"));
        assert_eq!(language_for_locales(["C", "C.UTF-8", "fr"]), Some("fr"));
        assert_eq!(language_for_locales(["eo"]), None);
        assert_eq!(language_for_locales(Vec::<String>::new()), None);
    }

    #[test]
    fn language_for_locales_ignores_region_script_encoding_and_modifier() {
        assert_eq!(language_for_locales(["zh-Hans-CN"]), Some("zh"));
        assert_eq!(language_for_locales(["pt_BR"]), Some("pt"));
        assert_eq!(language_for_locales(["de_DE.UTF-8"]), Some("de"));
        assert_eq!(language_for_locales(["nl_NL.UTF-8@euro"]), Some("nl"));
        assert_eq!(language_for_locales(["it@euro"]), Some("it"));
        assert_eq!(language_for_locales(["ES"]), Some("es"));
        assert_eq!(language_for_locales([String::from("ja-JP")]), Some("ja"));
    }

    #[test]
    fn table_round_trips_and_has_no_duplicates() {
        for (i, language) in LANGUAGES.iter().enumerate() {
            assert_eq!(name_to_code(language.name), language.code);
            assert_eq!(code_to_name(language.code), language.name);
            assert_eq!(language.code, language.code.to_lowercase());
            for other in &LANGUAGES[i + 1..] {
                assert_ne!(language.code, other.code);
                assert!(!language.name.eq_ignore_ascii_case(other.name));
            }
        }
    }

    #[test]
    fn auto_is_accepted_but_not_listed() {
        assert_eq!(name_to_code("Auto"), "auto");
        assert_eq!(name_to_code("AUTO"), "auto");
        assert_eq!(code_to_name("auto"), "Auto");
        assert!(LANGUAGES.iter().all(|l| l.code != "auto"));
    }

    #[test]
    fn lookups_ignore_case() {
        assert_eq!(name_to_code("rUsSiAn"), "ru");
        assert_eq!(code_to_name("RU"), "Russian");
    }

    #[test]
    fn unknown_values_pass_through_unchanged() {
        assert_eq!(name_to_code("Klingon"), "Klingon");
        assert_eq!(name_to_code("pt-BR"), "pt-BR");
        assert_eq!(code_to_name("tlh"), "tlh");
    }
}
