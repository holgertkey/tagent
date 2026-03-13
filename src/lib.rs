//! # Tagent
//!
//! Cross-platform text translation library and tool.
//!
//! Tagent supports three translation modes:
//! - **GUI hotkeys** — system-wide configurable hotkey translates selected text (default: `Alt+Q`)
//! - **Interactive terminal** — interactive prompt for typing translations
//! - **CLI mode** — single one-off translation from the command line
//!
//! Translation is powered by the Google Translate API, with optional dictionary
//! lookup and text-to-speech playback.
//!
//! ## Library usage
//!
//! The crate exposes its core subsystems as library modules, allowing embedding
//! Tagent's translation capabilities in other applications:
//!
//! ```no_run
//! use std::sync::Arc;
//! use tagent::config::ConfigManager;
//! use tagent::translator::Translator;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let config_path = ConfigManager::get_default_config_path()?;
//!     let config_manager = Arc::new(ConfigManager::new(config_path.to_str().unwrap())?);
//!     let translator = Translator::new_cli(config_manager)?;
//!     translator.translate_text_public("Hello world", "auto", "en").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Modules
//!
//! - [`config`] — Configuration management, hotkey parsing, history logging helpers
//! - [`providers`] — Translation provider trait and factory; currently ships Google Translate
//! - [`translator`] — High-level translation orchestrator that ties provider, clipboard, and UI together
//! - [`speech`] — Text-to-speech via Google TTS
//! - [`platform`] — Platform-specific implementations (clipboard, keyboard hook, window management)

pub mod config;
pub mod platform;
pub mod providers;
pub mod speech;
pub mod translator;
