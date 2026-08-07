//! Human-readable language name ↔ BCP-47 code mapping.

/// Maps a human-readable language name (e.g. `"Russian"`) to its BCP-47 code
/// (e.g. `"ru"`).
///
/// Returns `"auto"` for `"Auto"` and falls back to the input lowercased for
/// unknown names (which may already be a code).
///
/// # Examples
///
/// ```
/// assert_eq!(tagent::languages::name_to_code("Russian"), "ru");
/// assert_eq!(tagent::languages::name_to_code("klingon"), "klingon");
/// ```
pub fn name_to_code(language: &str) -> &str {
    match language.to_lowercase().as_str() {
        "auto" => "auto",
        "english" => "en",
        "russian" => "ru",
        "spanish" => "es",
        "french" => "fr",
        "german" => "de",
        "chinese" => "zh",
        "japanese" => "ja",
        "korean" => "ko",
        "italian" => "it",
        "portuguese" => "pt",
        "dutch" => "nl",
        "polish" => "pl",
        "turkish" => "tr",
        "arabic" => "ar",
        "hindi" => "hi",
        _ => language, // Return as-is if not found (might be a code already)
    }
}

/// Reverse of [`name_to_code`]: maps a BCP-47 code (e.g. `"ru"`) to its
/// human-readable name (e.g. `"Russian"`).
///
/// Returns the code as-is if no matching name is found.
///
/// # Examples
///
/// ```
/// assert_eq!(tagent::languages::code_to_name("ru"), "Russian");
/// ```
pub fn code_to_name(code: &str) -> &str {
    match code.to_lowercase().as_str() {
        "auto" => "Auto",
        "en" => "English",
        "ru" => "Russian",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "tr" => "Turkish",
        "ar" => "Arabic",
        "hi" => "Hindi",
        _ => code,
    }
}
