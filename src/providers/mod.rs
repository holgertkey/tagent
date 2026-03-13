use async_trait::async_trait;
use std::error::Error;

pub mod google;

/// Dictionary lookup result returned by a translation provider.
///
/// Contains all definitions grouped by part of speech. When the provider
/// silently corrected a spelling error, `corrected_word` holds the word that
/// was actually looked up, allowing callers to display a correction notice.
#[derive(Debug, Clone)]
pub struct DictionaryEntry {
    /// The word that was looked up (as supplied by the caller).
    pub word: String,
    /// Corrected source word when the input had a spelling error (e.g. `"violent"` for input `"vialent"`).
    pub corrected_word: Option<String>,
    /// Definitions grouped by part of speech.
    pub definitions: Vec<PartOfSpeechEntry>,
}

/// A set of definitions that all share the same part of speech (e.g. *noun*, *verb*).
#[derive(Debug, Clone)]
pub struct PartOfSpeechEntry {
    /// Part-of-speech label, e.g. `"noun"`, `"verb"`, `"adjective"`.
    pub part_of_speech: String,
    /// Individual definitions for this part of speech.
    pub definitions: Vec<Definition>,
}

/// A single definition with optional synonyms.
#[derive(Debug, Clone)]
pub struct Definition {
    /// Definition text.
    pub text: String,
    /// Synonyms listed alongside the definition, if any.
    pub synonyms: Vec<String>,
}

/// Abstraction over a translation backend.
///
/// Implement this trait to add a new translation service (DeepL, Yandex, etc.).
/// See [`google::GoogleTranslateProvider`] for a reference implementation.
///
/// # Adding a new provider
///
/// 1. Create `src/providers/yourprovider.rs` and implement this trait.
/// 2. Register it in [`create_provider`] with a matching name string.
/// 3. Users set `TranslateProvider = yourprovider` in `tagent.conf`.
#[async_trait]
pub trait TranslationProvider: Send + Sync {
    /// Translate `text` from language `from` to language `to`.
    ///
    /// `from` and `to` are BCP-47 language codes (e.g. `"en"`, `"ru"`).
    /// Pass `"auto"` for `from` to request automatic language detection.
    async fn translate_text(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;

    /// Look up a single word in the provider's dictionary.
    ///
    /// Returns `None` when dictionary lookup is not supported by the provider
    /// or the word was not found. For multi-word input the caller should fall
    /// back to [`translate_text`](TranslationProvider::translate_text).
    async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Box<dyn Error + Send + Sync>>;

    /// Detect the language of `text`.
    ///
    /// Returns a BCP-47 language code, e.g. `"en"`, `"ru"`, `"de"`.
    async fn detect_language(&self, text: &str) -> Result<String, Box<dyn Error + Send + Sync>>;

    /// Human-readable provider name for display purposes (e.g. `"Google Translate"`).
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

/// Instantiate a translation provider by name.
///
/// # Supported names
///
/// | Name       | Provider              |
/// |------------|-----------------------|
/// | `"google"` | Google Translate API  |
///
/// # Errors
///
/// Returns an error if `provider_name` does not match any known provider.
pub fn create_provider(provider_name: &str) -> Result<Box<dyn TranslationProvider>, Box<dyn Error + Send + Sync>> {
    match provider_name.to_lowercase().as_str() {
        "google" => Ok(Box::new(google::GoogleTranslateProvider::new())),
        _ => Err(format!("Unknown translation provider: {}", provider_name).into()),
    }
}
