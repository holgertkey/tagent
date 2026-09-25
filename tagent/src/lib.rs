//! # Tagent
//!
//! Translation, dictionary lookup, and text-to-speech library, powered by the
//! Google Translate API.
//!
//! The crate is provider-agnostic. It has three independent provider axes, so the
//! backend that translates, the one that looks words up, and the one that speaks are
//! separate choices, usable in any combination:
//!
//! | Axis        | Trait                              | Factory                                 | Built-in                                        |
//! |-------------|------------------------------------|-----------------------------------------|-------------------------------------------------|
//! | Translation | [`providers::TranslationProvider`] | [`providers::create_provider`]          | [`providers::google::GoogleTranslateProvider`]  |
//! | Dictionary  | [`providers::DictionaryProvider`]  | [`providers::create_dictionary_provider`] | [`providers::google::GoogleDictionaryProvider`] |
//! | Speech      | [`providers::SpeechProvider`]      | [`providers::create_speech_provider`]   | [`providers::google::GoogleSpeechProvider`]     |
//!
//! The crate has no knowledge of configuration files, clipboards, hotkeys, or any
//! other application concern — those live in the `tagent-cli` and `tagent-gui`
//! applications built on top of it.
//!
//! ## Quick start
//!
//! Translate a phrase, letting the provider detect the source language:
//!
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), tagent::error::Error> {
//!     let provider = tagent::providers::create_provider("google")?;
//!     let translated = provider.translate_text("Hello world", "auto", "ru").await?;
//!     println!("{translated}");
//!     Ok(())
//! }
//! ```
//!
//! Look up a single word in the dictionary, with spelling correction:
//!
//! ```no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), tagent::error::Error> {
//! let dictionary = tagent::providers::create_dictionary_provider("google")?;
//! match dictionary.lookup("vialent", "en", "ru").await? {
//!     Some(entry) => {
//!         // `corrected_word` is the word the provider actually looked up; compare it
//!         // with your input to decide whether to show a "did you mean" notice.
//!         println!("looked up {:?}", entry.corrected_word);
//!         for pos in &entry.definitions {
//!             println!("{}: {} definition(s)", pos.part_of_speech, pos.definitions.len());
//!         }
//!     }
//!     None => println!("no dictionary entry"),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Synthesize speech — split first, then fetch one chunk at a time:
//!
//! ```no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), tagent::error::Error> {
//! let speech = tagent::providers::create_speech_provider("google")?;
//! let mut audio = Vec::new();
//! for chunk in speech.split_for_speech("Hello world") {
//!     audio.extend(speech.speak_chunk(&chunk, "en").await?);
//! }
//! std::fs::write("hello.mp3", audio).expect("write hello.mp3");
//! # Ok(())
//! # }
//! ```
//!
//! Runnable versions of these, plus a fully offline custom provider, are in the
//! crate's `examples/` directory:
//!
//! ```text
//! cargo run -p tagent --example translate -- "Hello world" ru
//! cargo run -p tagent --example dictionary -- vialent
//! cargo run -p tagent --example speak -- "Hello world" en
//! cargo run -p tagent --example custom_provider
//! ```
//!
//! ## Concepts
//!
//! - **Language codes.** Providers take BCP-47 codes (`"en"`, `"ru"`, `"de"`). Use
//!   [`languages::name_to_code`] / [`languages::code_to_name`] to convert from and to
//!   human-readable names such as `"Russian"`.
//! - **`"auto"` source language.** [`providers::TranslationProvider::translate_text`] and
//!   [`providers::DictionaryProvider::lookup`] accept `"auto"` as the source and detect it
//!   themselves (a dictionary backend that cannot returns `Ok(None)`). [`providers::SpeechProvider::speak_chunk`] does *not*: it
//!   needs a concrete code, so resolve `"auto"` first with
//!   [`providers::resolve_source_language`]. That is the only place two provider axes
//!   meet, which is why a translation provider is only needed for speech when the source
//!   language is `"auto"`.
//! - **Errors.** Every fallible call returns [`error::Error`], a plain enum with no
//!   provider-specific types in it.
//! - **Async runtime.** Calls are `async` and the built-in providers use `reqwest`, so they
//!   need a Tokio runtime.
//! - **Network caveats.** The built-in Google providers talk to *unofficial* endpoints with
//!   no API key and no service guarantees — see [`providers::google`] before depending on
//!   them.
//!
//! ## Modules
//!
//! - [`providers`] — Translation, dictionary and speech provider traits and factories,
//!   plus the Google implementations. Start here to use, or to extend, the crate.
//! - [`languages`] — Human-readable language name ↔ BCP-47 code mapping.
//! - [`article`] — Role-tagged display layout of a dictionary entry, for plain or
//!   highlighted rendering.
//! - [`error`] — Unified error type used throughout this crate.
//!
//! ## Versioning
//!
//! The crate is pre-1.0 (see `CHANGELOG.md`): a breaking change to the public API bumps
//! the minor version, a compatible addition or fix bumps the patch.

#![warn(missing_docs)]

pub mod article;
/// Unified error type for the `tagent` library.
pub mod error;
/// Human-readable language name ↔ BCP-47 code mapping.
pub mod languages;
pub mod providers;
