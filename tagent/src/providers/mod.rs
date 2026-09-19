use crate::error::Error;
use async_trait::async_trait;

/// Google Translate and Google text-to-speech provider implementations.
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
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Error>;

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
    ) -> Result<Option<DictionaryEntry>, Error>;

    /// Detect the language of `text`.
    ///
    /// Returns a BCP-47 language code, e.g. `"en"`, `"ru"`, `"de"`.
    async fn detect_language(&self, text: &str) -> Result<String, Error>;

    /// Human-readable provider name for display purposes (e.g. `"Google Translate"`).
    fn name(&self) -> &str;
}

/// Abstraction over a text-to-speech backend.
///
/// Independent of [`TranslationProvider`]: which backend translates and which one
/// speaks are separate choices (`TranslateProvider` × `SpeechProvider` in `tagent-cli`'s
/// config, `translate_provider` × `speech_provider` in `tagent-gui`'s), so an
/// implementation of this trait never needs to know anything about translation.
/// See [`google::GoogleSpeechProvider`] for a reference implementation.
///
/// Playback is deliberately split into [`split_for_speech`](Self::split_for_speech) +
/// [`speak_chunk`](Self::speak_chunk) rather than one `speak()` call, so callers can fetch
/// and play one chunk at a time (audio starts after the first chunk's round-trip, and a
/// cancellation flag can be checked between chunks).
///
/// # Adding a new speech provider
///
/// 1. Implement this trait (in a new `src/providers/yourprovider.rs`, or alongside an
///    existing backend in the same file).
/// 2. Register it in [`create_speech_provider`] with a matching name string.
/// 3. Users set `SpeechProvider = yourprovider` in `tagent-cli.conf` (or `speech_provider`
///    in `tagent-gui.json`).
///
/// # Examples
///
/// A backend with no per-request length limit returns the whole text as one chunk:
///
/// ```
/// use async_trait::async_trait;
/// use tagent::error::Error;
/// use tagent::providers::SpeechProvider;
///
/// struct SilentProvider;
///
/// #[async_trait]
/// impl SpeechProvider for SilentProvider {
///     fn split_for_speech(&self, text: &str) -> Vec<String> {
///         vec![text.to_string()]
///     }
///
///     async fn speak_chunk(&self, _text: &str, _lang: &str) -> Result<Vec<u8>, Error> {
///         Ok(Vec::new())
///     }
///
///     fn name(&self) -> &str {
///         "Silent"
///     }
/// }
///
/// assert_eq!(SilentProvider.split_for_speech("Hello"), vec!["Hello".to_string()]);
/// ```
#[async_trait]
pub trait SpeechProvider: Send + Sync {
    /// Split `text` into chunks small enough for a single [`speak_chunk`](Self::speak_chunk)
    /// request, in playback order.
    ///
    /// Chunk sizing is the provider's own constraint, not a general one: a backend with
    /// no per-request length limit returns the whole text as a single chunk.
    ///
    /// Splitting is a pure, non-network operation so callers can fetch and play chunks
    /// one at a time (e.g. to start audio playback before later chunks are fetched, or
    /// to check a cancellation flag between chunks) rather than waiting on every chunk
    /// up front.
    fn split_for_speech(&self, text: &str) -> Vec<String>;

    /// Synthesizes speech for a single chunk of `text` (as produced by
    /// [`split_for_speech`](Self::split_for_speech)) in language `lang`, returning one
    /// independently decodable audio clip.
    ///
    /// `lang` is a BCP-47 language code (e.g. `"en"`, `"ru"`); it must already be
    /// resolved — see [`resolve_source_language`] for turning `"auto"` into a concrete code.
    async fn speak_chunk(&self, text: &str, lang: &str) -> Result<Vec<u8>, Error>;

    /// Human-readable provider name for display purposes (e.g. `"Google TTS"`).
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
/// Returns [`Error::UnknownProvider`] if `provider_name` does not match any known provider.
pub fn create_provider(provider_name: &str) -> Result<Box<dyn TranslationProvider>, Error> {
    match provider_name.to_lowercase().as_str() {
        "google" => Ok(Box::new(google::GoogleTranslateProvider::new())),
        _ => Err(Error::UnknownProvider(provider_name.to_string())),
    }
}

/// Instantiate a speech provider by name.
///
/// Independent of [`create_provider`]: the speech backend is chosen separately from the
/// translation backend.
///
/// # Supported names
///
/// | Name       | Provider                                  |
/// |------------|-------------------------------------------|
/// | `"google"` | Google's `translate_tts` text-to-speech   |
///
/// # Errors
///
/// Returns [`Error::UnknownProvider`] if `provider_name` does not match any known provider.
///
/// # Examples
///
/// ```
/// use tagent::providers::create_speech_provider;
///
/// assert!(create_speech_provider("google").is_ok());
/// assert!(create_speech_provider("no-such-provider").is_err());
/// ```
pub fn create_speech_provider(provider_name: &str) -> Result<Box<dyn SpeechProvider>, Error> {
    match provider_name.to_lowercase().as_str() {
        "google" => Ok(Box::new(google::GoogleSpeechProvider::new())),
        _ => Err(Error::UnknownProvider(provider_name.to_string())),
    }
}

/// Resolves `from` to a concrete BCP-47 language code, calling
/// [`TranslationProvider::detect_language`] when `from == "auto"`.
///
/// On detection failure, logs a warning to stderr and falls back to `"en"` rather
/// than propagating the error, matching the best-effort behavior expected of
/// language auto-detection in interactive contexts (e.g. before TTS playback).
pub async fn resolve_source_language(
    provider: &dyn TranslationProvider,
    text: &str,
    from: &str,
) -> String {
    if from == "auto" {
        match provider.detect_language(text).await {
            Ok(detected) => detected,
            Err(e) => {
                eprintln!("Language detection failed: {}, using 'en'", e);
                "en".to_string()
            }
        }
    } else {
        from.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_speech_provider_google_succeeds() {
        let provider = create_speech_provider("google").expect("google is registered");
        assert_eq!(provider.name(), "Google TTS");
    }

    #[test]
    fn create_speech_provider_is_case_insensitive() {
        assert!(create_speech_provider("GoOgLe").is_ok());
    }

    #[test]
    fn create_speech_provider_unknown_name_errors() {
        match create_speech_provider("no-such-provider") {
            Err(Error::UnknownProvider(name)) => assert_eq!(name, "no-such-provider"),
            Err(other) => panic!("expected UnknownProvider, got {other:?}"),
            Ok(_) => panic!("expected an error for an unknown provider name"),
        }
    }
}
