//! # Tagent
//!
//! Translation, dictionary lookup, and text-to-speech library, powered by the
//! Google Translate API.
//!
//! This crate is provider-agnostic: [`providers::TranslationProvider`] defines the
//! interface, and [`providers::google::GoogleTranslateProvider`] is the reference
//! implementation. It has no knowledge of configuration files, clipboards, hotkeys,
//! or any other application concern — those live in the `tagent-cli` and
//! `tagent-gui` binaries built on top of this crate.
//!
//! ## Example
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
//! ## Modules
//!
//! - [`providers`] — Translation provider trait and factory; currently ships Google Translate
//! - [`languages`] — Human-readable language name ↔ BCP-47 code mapping
//! - [`error`] — Unified error type used throughout this crate

#![warn(missing_docs)]

/// Unified error type for the `tagent` library.
pub mod error;
/// Human-readable language name ↔ BCP-47 code mapping.
pub mod languages;
/// Translation provider trait and factory; currently ships Google Translate.
pub mod providers;
