//! Translation and speech provider traits, their factories, and the built-in Google
//! implementations.
//!
//! There are two independent provider axes, and a backend implements only the one it
//! provides:
//!
//! - [`TranslationProvider`] — translate text, look up a dictionary entry, detect a language.
//!   Create one by name with [`create_provider`].
//! - [`SpeechProvider`] — turn text into audio. Create one by name with
//!   [`create_speech_provider`].
//!
//! Speech is deliberately not part of [`TranslationProvider`]: a translator and a speaker
//! are separate choices, and the only place the two meet is [`resolve_source_language`],
//! which turns an `"auto"` source language into a concrete code before speaking.
//!
//! # Contracts shared by all providers
//!
//! - **Language codes** are BCP-47 (`"en"`, `"ru"`). `"auto"` is a valid *source* language
//!   for translation and dictionary calls, and is never valid for speech — see
//!   [`resolve_source_language`].
//! - **Errors** are always [`Error`]; a provider maps its own failures onto these variants
//!   rather than adding new ones:
//!
//!   | Variant                     | Meaning                                                    |
//!   |-----------------------------|------------------------------------------------------------|
//!   | [`Error::Network`]          | Transport failure or timeout (the Google providers time out after 10 seconds). |
//!   | [`Error::Api`]              | The service answered with an error status.                 |
//!   | [`Error::Decode`]           | The response body could not be parsed into the expected shape. |
//!   | [`Error::EmptyText`]        | Empty input where non-empty text is required (Google's `speak_chunk`). |
//!   | [`Error::TextTooLong`]      | A chunk longer than the provider accepts (Google's `speak_chunk`). |
//!   | [`Error::UnknownProvider`]  | A factory was given a name it does not know.               |
//!
//!   [`Error::NotFound`] exists but no built-in provider returns it: a dictionary miss is
//!   `Ok(None)` from [`TranslationProvider::get_dictionary_entry`], not an error.
//! - **`Send + Sync`**. Both traits require it, so one provider can be shared across tasks.
//!   The factories build a *new* provider (with a fresh HTTP client) on every call, so
//!   create one and reuse it, e.g. behind an [`Arc`](std::sync::Arc), rather than calling a
//!   factory per request.
//!
//! # Writing a translation provider
//!
//! Implement [`TranslationProvider`] and use the value directly as a
//! `Box<dyn TranslationProvider>` — nothing needs to be registered for that to work. The
//! factories are a closed `match` inside this crate, so to make a new backend selectable by
//! name (`create_provider("yours")`, and through it a config file), add a branch to
//! [`create_provider`] here. The complete, offline example below implements all four
//! methods; a real backend does the same with an HTTP call in `translate_text`,
//! `get_dictionary_entry` and `detect_language`.
//!
//! ```
//! use async_trait::async_trait;
//! use tagent::error::Error;
//! use tagent::providers::{DictionaryEntry, TranslationProvider};
//!
//! /// A toy backend that "translates" by upper-casing.
//! struct Shout;
//!
//! #[async_trait]
//! impl TranslationProvider for Shout {
//!     async fn translate_text(&self, text: &str, _from: &str, _to: &str) -> Result<String, Error> {
//!         if text.trim().is_empty() {
//!             return Err(Error::EmptyText);
//!         }
//!         Ok(text.to_uppercase())
//!     }
//!
//!     // No dictionary support: `None` tells callers to fall back to `translate_text`.
//!     async fn get_dictionary_entry(
//!         &self,
//!         _word: &str,
//!         _from: &str,
//!         _to: &str,
//!     ) -> Result<Option<DictionaryEntry>, Error> {
//!         Ok(None)
//!     }
//!
//!     async fn detect_language(&self, _text: &str) -> Result<String, Error> {
//!         Ok("en".to_string())
//!     }
//!
//!     fn name(&self) -> &str {
//!         "Shout"
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     let provider: Box<dyn TranslationProvider> = Box::new(Shout);
//!     assert_eq!(provider.translate_text("hello", "auto", "en").await?, "HELLO");
//!     Ok(())
//! }
//! ```
//!
//! # Writing a speech provider
//!
//! Implement [`SpeechProvider`]; its trait documentation has a compilable example. Two
//! things are easy to miss:
//!
//! - **Chunk size is the provider's own limit.** [`split_for_speech`](SpeechProvider::split_for_speech)
//!   is pure and synchronous, and a backend without a per-request limit just returns the
//!   whole text as one chunk. Callers fetch and play chunk by chunk, so audio starts after
//!   the first chunk's round trip rather than after the whole text has been synthesized.
//! - **Audio must be decodable by the caller.** [`speak_chunk`](SpeechProvider::speak_chunk)
//!   returns bytes for one independently decodable clip. The bundled applications decode
//!   MP3 with `rodio`, so a backend returning another codec also needs that codec enabled
//!   in the application.
//!
//! An example of each provider kind lives in the crate's `examples/` directory
//! (`custom_provider`).

use crate::error::Error;
use async_trait::async_trait;

pub mod google;

/// Dictionary lookup result returned by a translation provider.
///
/// Contains all definitions grouped by part of speech. `corrected_word` holds the word the
/// provider actually looked up, which differs from the input when it corrected a spelling
/// error; compare the two to decide whether to show a correction notice.
#[derive(Debug, Clone)]
pub struct DictionaryEntry {
    /// The word that was looked up (as supplied by the caller).
    pub word: String,
    /// The source word the provider actually looked up, when it reported one (e.g.
    /// `"violent"` for input `"vialent"`).
    ///
    /// This is `Some` whenever the provider echoes the word back, **including when the
    /// input was already spelled correctly**, so it is not itself a "was corrected" flag.
    /// Compare it with the original input (case-insensitively, as the bundled applications
    /// do) before showing a notice.
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
/// See [`google::GoogleTranslateProvider`] for a reference implementation, and the
/// [module documentation](self) for the error contract and a complete offline example.
///
/// # Adding a new provider
///
/// 1. Create `src/providers/yourprovider.rs` and implement this trait.
/// 2. Add `pub mod yourprovider;` here and register it in [`create_provider`] with a
///    matching name string.
/// 3. Users select it with `TranslateProvider = yourprovider` in `tagent-cli.conf`, or
///    `translate_provider` in `tagent-gui.json`.
#[async_trait]
pub trait TranslationProvider: Send + Sync {
    /// Translate `text` from language `from` to language `to`.
    ///
    /// `from` and `to` are BCP-47 language codes (e.g. `"en"`, `"ru"`).
    /// Pass `"auto"` for `from` to request automatic language detection; `to` must be a
    /// concrete code.
    ///
    /// # Errors
    ///
    /// [`Error::Network`] on a transport failure or timeout, [`Error::Api`] on an error
    /// status, and [`Error::Decode`] when the response cannot be parsed.
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Error>;

    /// Look up a single word in the provider's dictionary.
    ///
    /// Returns `None` when dictionary lookup is not supported by the provider
    /// or the word was not found — a miss is not an error. For multi-word input the caller
    /// should fall back to [`translate_text`](TranslationProvider::translate_text).
    ///
    /// A provider that spell-checks reports the word it actually looked up in
    /// [`DictionaryEntry::corrected_word`].
    ///
    /// # Errors
    ///
    /// The same transport, status and decode errors as
    /// [`translate_text`](TranslationProvider::translate_text).
    async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Error>;

    /// Detect the language of `text`.
    ///
    /// Returns a BCP-47 language code, e.g. `"en"`, `"ru"`, `"de"`.
    ///
    /// Most callers should go through [`resolve_source_language`], which also turns a
    /// detection failure into a `"en"` fallback instead of an error.
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
    ///
    /// The returned bytes must be decodable on their own, in whatever codec the provider
    /// produces (the bundled applications decode MP3).
    ///
    /// # Errors
    ///
    /// [`Error::Network`] / [`Error::Api`] on a transport or status failure. Providers with
    /// a per-request limit also reject an empty chunk ([`Error::EmptyText`]) and an
    /// oversized one ([`Error::TextTooLong`]) instead of truncating it.
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
///
/// # Examples
///
/// Names are matched case-insensitively. Each call builds a new provider with its own HTTP
/// client, so create it once and reuse it:
///
/// ```
/// use tagent::providers::create_provider;
///
/// let provider = create_provider("Google").expect("google is registered");
/// assert_eq!(provider.name(), "Google Translate");
/// assert!(create_provider("no-such-provider").is_err());
/// ```
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
///
/// A concrete `from` is returned unchanged without touching the network, so this is safe to
/// call unconditionally before [`SpeechProvider::speak_chunk`].
///
/// # Examples
///
/// ```
/// use tagent::providers::{google::GoogleTranslateProvider, resolve_source_language};
///
/// # #[tokio::main]
/// # async fn main() {
/// let provider = GoogleTranslateProvider::new();
/// // A concrete source language is passed through; only "auto" triggers detection.
/// assert_eq!(resolve_source_language(&provider, "Hallo", "de").await, "de");
/// # }
/// ```
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
