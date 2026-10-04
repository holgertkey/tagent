use crate::config::{self, ConfigManager};
use crate::platform::{ClipboardManager, WindowHandle, WindowManager};
use rustyline::ExternalPrinter;
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tagent::article;
use tagent::providers::{DictionaryProvider, TranslationProvider};

/// Shared slot for the rustyline external printer used to route hotkey-triggered
/// translation output safely while the interactive prompt may be mid-read on another
/// thread. `None` until [`InteractiveMode::start`](crate::interactive::InteractiveMode::start)
/// installs one; always `None` in CLI mode, which never calls [`Translator::translate_clipboard`].
type SharedPrinter = Arc<Mutex<Option<Box<dyn ExternalPrinter + Send>>>>;

/// The most recent successful translation, kept so `/s` and `/ss` can replay it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastTranslation {
    /// The text that was translated.
    pub phrase: String,
    /// The plain translation; for a dictionary hit, the primary translation (never the
    /// full article). `None` when a dictionary lookup succeeded but its plain translation
    /// failed.
    pub translation: Option<String>,
    /// Source language code at translation time (may be `"auto"`).
    pub source_code: String,
    /// Target language code at translation time.
    pub target_code: String,
}

/// A successful translation and the provider that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// The translated text.
    pub text: String,
    /// The `translate_provider` value (a kind or profile name, lowercased) of the provider
    /// that translated, which is what the answer's label shows.
    pub provider: String,
}

/// Result of [`Translator::get_dictionary_entry`].
#[derive(Debug)]
pub struct DictionaryLookup {
    /// The dictionary article as plain text, for the clipboard and the history file.
    pub formatted: String,
    /// The same article as role-tagged lines, for highlighted display with
    /// [`config::render_article`].
    pub lines: Vec<article::Line>,
    /// Set when the provider looked up a spelling-corrected word instead.
    pub corrected_word: Option<String>,
    /// The plain translation of the word, if the translate request succeeded.
    pub primary_translation: Option<String>,
}

/// The translation provider in use, shared by every clone of a [`Translator`] so a
/// switch (`/p`, or a hot reload of `translate_provider`) reaches the hotkey path too.
struct ActiveTranslation {
    /// The `translate_provider` value `provider` was built from.
    name: String,
    provider: Arc<dyn TranslationProvider>,
    /// A `translate_provider` value that failed to build, so the error is reported once
    /// rather than on every translation until the value changes.
    failed: Option<String>,
}

impl ActiveTranslation {
    fn new(name: &str, provider: Arc<dyn TranslationProvider>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            name: name.to_string(),
            provider,
            failed: None,
        }))
    }
}

/// The dictionary provider in use, shared by every clone of a [`Translator`] like
/// [`ActiveTranslation`], so a switch (`/p d`, or a hot reload of `dictionary_provider`)
/// reaches the hotkey path too.
struct ActiveDictionary {
    /// The `dictionary_provider` value `provider` was built from.
    name: String,
    /// The provider, or why it couldn't be built (non-fatal, unlike translation: lookups
    /// then fail at once and callers fall back to plain translation).
    provider: Result<Arc<dyn DictionaryProvider>, String>,
    /// A `dictionary_provider` value that failed to build while a working provider was
    /// kept, so the error is reported once rather than on every lookup.
    failed: Option<String>,
}

impl ActiveDictionary {
    fn new(name: &str, provider: Result<Arc<dyn DictionaryProvider>, String>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            name: name.to_string(),
            provider,
            failed: None,
        }))
    }
}

/// High-level translation orchestrator.
///
/// `Translator` ties together a [`TranslationProvider`] and a [`DictionaryProvider`] (from
/// the [`tagent`] library crate), the system clipboard, and optional window management to provide the full
/// Tagent translation experience. Use [`Translator::new_cli`] when window management
/// is not needed (e.g. one-off CLI translations).
#[derive(Clone)]
pub struct Translator {
    translation: Arc<Mutex<ActiveTranslation>>,
    dictionary: Arc<Mutex<ActiveDictionary>>,
    clipboard: ClipboardManager,
    config_manager: Arc<ConfigManager>,
    window_manager: Option<Arc<WindowManager>>,
    stored_foreground_window: Arc<std::sync::Mutex<Option<WindowHandle>>>,
    printer: SharedPrinter,
    /// Shared between the hotkey path and interactive mode (both hold clones of this
    /// `Translator`), so `/s`/`/ss` replay whichever translation happened last.
    last_translation: Arc<Mutex<Option<LastTranslation>>>,
}

impl Translator {
    /// Create a full translator for unified mode, including window management for
    /// showing/hiding the terminal. If window management fails to initialize (e.g. no
    /// display server), falls back to a translator without it rather than erroring out —
    /// use [`Translator::new_cli`] instead if window management is never needed.
    pub fn new_with_config(
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let window_manager = match WindowManager::new() {
            Ok(wm) => Some(Arc::new(wm)),
            Err(e) => {
                // Only worth a word when the user asked for it; the hotkeys work either way.
                if config_manager.get_config().show_terminal_on_translate {
                    eprintln!(
                        "Note: show_terminal_on_translate has no effect: {e} \
                         (the translation still appears at the prompt)."
                    );
                }
                None
            }
        };

        Self::build(config_manager, window_manager)
    }

    /// Create Translator without window management (for CLI mode)
    pub fn new_cli(
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::build(config_manager, None)
    }

    fn build(
        config_manager: Arc<ConfigManager>,
        window_manager: Option<Arc<WindowManager>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Create translation provider based on config
        let config = config_manager.get_config();
        let provider = config.create_translate_provider()?;

        // A bad dictionary provider must never break translation: warn once and disable
        // dictionary lookups instead of failing to start (unlike the translate provider).
        let dictionary_provider = config
            .create_dictionary_provider()
            .map(Arc::from)
            .inspect_err(|message| {
                eprintln!(
                    "Dictionary provider unavailable: {message}; dictionary lookups disabled"
                );
            });

        Ok(Self {
            translation: ActiveTranslation::new(&config.translate_provider, Arc::from(provider)),
            dictionary: ActiveDictionary::new(&config.dictionary_provider, dictionary_provider),
            clipboard: ClipboardManager::new(),
            config_manager,
            window_manager,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
        })
    }

    /// The providers for the banner: translation and dictionary for `config`'s
    /// `translate_provider` and `dictionary_provider` (see [`Self::translation_provider`]
    /// and [`Self::dictionary_provider`]), speech from `config`, since it is built again
    /// for every playback.
    pub fn active_providers(&self, config: &config::Config) -> config::ActiveProviders {
        config::ActiveProviders {
            translation: Ok(self.translation_provider(config).name().to_string()),
            dictionary: self
                .dictionary_provider(config)
                .map(|dictionary| dictionary.name().to_string()),
            speech: config
                .create_speech_provider()
                .map(|speech| speech.name().to_string()),
        }
    }

    /// Install the external printer used to route [`translate_clipboard`](Self::translate_clipboard)
    /// output safely while the interactive prompt may be reading a line in raw mode.
    /// Called once, from `InteractiveMode::start()`, right after the rustyline `Editor` is built.
    pub fn set_external_printer(&self, printer: impl ExternalPrinter + Send + 'static) {
        *self.printer.lock().unwrap() = Some(Box::new(printer));
    }

    /// Remember a successful translation for `/s` and `/ss`.
    pub fn record_last_translation(
        &self,
        phrase: &str,
        translation: Option<&str>,
        source_code: &str,
        target_code: &str,
    ) {
        *self.last_translation.lock().unwrap() = Some(LastTranslation {
            phrase: phrase.to_string(),
            translation: translation.map(str::to_string),
            source_code: source_code.to_string(),
            target_code: target_code.to_string(),
        });
    }

    /// The most recent successful translation (hotkey or interactive), if any.
    pub fn last_translation(&self) -> Option<LastTranslation> {
        self.last_translation.lock().unwrap().clone()
    }

    /// True once an external printer has been installed via [`set_external_printer`](Self::set_external_printer).
    fn has_external_printer(&self) -> bool {
        self.printer.lock().unwrap().is_some()
    }

    /// Emit hotkey-triggered translation output, routed through the external printer when
    /// one is installed (so it can't corrupt an interactive prompt mid-read on another
    /// thread), or printed directly to stdout otherwise.
    fn emit(&self, msg: &str) {
        let mut guard = self.printer.lock().unwrap();
        if let Some(printer) = guard.as_mut() {
            if printer.print(msg.to_string()).is_ok() {
                return;
            }
        }
        drop(guard);
        print!("{}", msg);
        io::stdout().flush().ok();
    }

    /// Like [`emit`](Self::emit) but appends a trailing newline, mirroring `println!`.
    pub(crate) fn emit_line(&self, msg: impl AsRef<str>) {
        self.emit(&format!("{}\n", msg.as_ref()));
    }

    /// Copy text to clipboard if enabled in config
    fn copy_to_clipboard_if_enabled(
        &self,
        text: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if config.copy_to_clipboard {
            self.clipboard
                .set_text(text)
                .map_err(|e| -> Box<dyn Error + Send + Sync> { e.to_string().into() })
        } else {
            Ok(())
        }
    }

    /// Reprint the source language prompt after hotkey-triggered output, but only on the
    /// no-printer fallback path — once an external printer is installed, rustyline redraws
    /// the real (possibly non-empty) prompt itself, and a plain `print!` here would bypass
    /// the printer and corrupt it.
    fn maybe_print_source_prompt(&self, config: &crate::config::Config) {
        if self.has_external_printer() {
            return;
        }
        let (source_code, target_code) = self.config_manager.get_language_codes();
        let source_prompt = format!(
            "[{}]: ",
            config::language_pair_label(&source_code, &target_code)
        );
        config::print_colored(&source_prompt, &config.source_prompt_color);
        io::stdout().flush().ok();
    }

    /// Main function for translating text from clipboard
    pub async fn translate_clipboard(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check if config file was modified and reload if necessary
        if let Err(e) = self.config_manager.check_and_reload() {
            self.emit_line(format!("Config reload error: {}", e));
        }

        let config = self.config_manager.get_config();

        // Store the current foreground window before any operations
        if config.show_terminal_on_translate {
            if let Some(wm) = &self.window_manager {
                if let Some(fg_window) = wm.get_foreground_window() {
                    if let Ok(mut stored) = self.stored_foreground_window.lock() {
                        *stored = Some(fg_window);
                    }
                }
            }
        }

        let original_text = match self.clipboard.get_selected_text() {
            Ok(text) => {
                if text.trim().is_empty() {
                    self.emit_line("No selected text or clipboard is empty");
                    return Ok(());
                }
                text.trim().to_string()
            }
            Err(e) => {
                self.emit_line(format!("Copy or clipboard read error: {}", e));
                return Err(e.to_string().into());
            }
        };

        // Show terminal window if configured
        if config.show_terminal_on_translate {
            if let Some(wm) = &self.window_manager {
                if let Err(e) = wm.show_terminal() {
                    self.emit_line(format!("Failed to show terminal: {}", e));
                }
            }
        }

        let (source_code, target_code) = self.config_manager.get_language_codes();

        // Check if it's a single word and dictionary feature is enabled
        if config.show_dictionary && config::is_single_word(&original_text) {
            match self
                .get_dictionary_entry(&original_text, &source_code, &target_code)
                .await
            {
                Ok(DictionaryLookup {
                    formatted: dictionary_info,
                    lines,
                    corrected_word,
                    primary_translation,
                }) => {
                    self.record_last_translation(
                        &original_text,
                        primary_translation.as_deref(),
                        &source_code,
                        &target_code,
                    );

                    // Clear any existing prompt and print on new line (no-printer fallback only;
                    // with a printer installed, rustyline handles redrawing on its own).
                    if !self.has_external_printer() {
                        print!("\r");
                        io::stdout().flush().ok();
                    }

                    // Show the original text (source word)
                    let source_label = format!(
                        "[{}]: ",
                        config::language_pair_label(&source_code, &target_code)
                    );
                    self.emit_line(format!(
                        "{}{}",
                        config::colorize(&source_label, &config.source_prompt_color),
                        original_text
                    ));

                    // If a spelling correction was applied, notify the user
                    if config.spell_check {
                        if let Some(ref corrected) = corrected_word {
                            if corrected.to_lowercase() != original_text.to_lowercase() {
                                self.emit_line(Self::correction_notice(
                                    corrected,
                                    &target_code,
                                    &config,
                                ));
                            }
                        }
                    }

                    // Print colored dictionary label
                    self.emit_line(format!(
                        "{}{}\n",
                        config::colorize("[Word]: ", &config.dictionary_prompt_color),
                        config::render_article(&lines, &config)
                    ));

                    if let Err(e) = self.copy_to_clipboard_if_enabled(&dictionary_info, &config) {
                        self.emit_line(config::colorize(
                            &format!("Dictionary clipboard write error: {}", e),
                            &config.error_color,
                        ));
                    }

                    // Save dictionary entry to history
                    if let Err(e) = config::save_translation_history(
                        &original_text,
                        &dictionary_info,
                        &source_code,
                        &target_code,
                        &config,
                    ) {
                        self.emit_line(config::colorize(
                            &format!("History save error: {}", e),
                            &config.error_color,
                        ));
                    }

                    // Show source language prompt after hotkey translation
                    self.maybe_print_source_prompt(&config);
                }
                Err(_) => {
                    // Fall back to regular translation
                    self.perform_translation(&original_text, &source_code, &target_code, &config)
                        .await?;
                }
            }
        } else {
            // Regular translation for phrases or when dictionary is disabled
            self.perform_translation(&original_text, &source_code, &target_code, &config)
                .await?;
        }

        // Hide terminal and restore previous window after delay if configured
        if config.show_terminal_on_translate
            && config.auto_hide_terminal_seconds > 0
            && self.window_manager.is_some()
        {
            self.hide_terminal_and_restore(config.auto_hide_terminal_seconds)
                .await;
        }

        Ok(())
    }

    /// Perform regular translation
    async fn perform_translation(
        &self,
        text: &str,
        source_code: &str,
        target_code: &str,
        config: &crate::config::Config,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Clear any existing prompt and move to new line (no-printer fallback only).
        if !self.has_external_printer() {
            print!("\r");
            io::stdout().flush().ok();
        }

        // Show the language pair with colored prompt, like the interactive prompt
        let source_label = format!(
            "[{}]: ",
            config::language_pair_label(source_code, target_code)
        );
        self.emit_line(format!(
            "{}{}",
            config::colorize(&source_label, &config.source_prompt_color),
            text
        ));

        // If source language is not Auto, check if text matches expected language
        if source_code != "auto" && !self.is_expected_language(text, source_code) {
            self.emit_line(format!(
                "Text does not appear to be in {} language",
                config.source_language_name()
            ));
            self.maybe_print_source_prompt(config);
            return Ok(());
        }

        match self
            .translate_text_internal(text, source_code, target_code)
            .await
        {
            Ok(Translation {
                text: translated_text,
                provider,
            }) => {
                self.record_last_translation(
                    text,
                    Some(&translated_text),
                    source_code,
                    target_code,
                );

                // Print colored translation label: the provider that translated
                let trans_label = format!("[{}]: ", provider);
                self.emit_line(format!(
                    "{}{}\n",
                    config::colorize(&trans_label, &config.target_prompt_color),
                    translated_text
                ));

                if let Err(e) = self.copy_to_clipboard_if_enabled(&translated_text, config) {
                    self.emit_line(config::colorize(
                        &format!("Translation clipboard write error: {}", e),
                        &config.error_color,
                    ));
                }

                // Save translation to history
                if let Err(e) = config::save_translation_history(
                    text,
                    &translated_text,
                    source_code,
                    target_code,
                    config,
                ) {
                    self.emit_line(config::colorize(
                        &format!("History save error: {}", e),
                        &config.error_color,
                    ));
                }

                // Show source language prompt after hotkey translation
                self.maybe_print_source_prompt(config);
            }
            Err(e) => {
                self.emit_line(config::colorize(
                    &format!("Translation error: {}", e),
                    &config.error_color,
                ));
                self.maybe_print_source_prompt(config);
            }
        }

        Ok(())
    }

    /// Public method to get dictionary entry.
    /// See [`DictionaryLookup`] for what is returned; `corrected_word` is `Some` when
    /// the provider detected a spelling error and used a corrected word for the lookup.
    pub async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<DictionaryLookup, Box<dyn Error + Send + Sync>> {
        // No dictionary provider: fail before any network call so the caller's fallback to
        // plain translation runs exactly once.
        let dictionary_provider = self
            .dictionary_provider(&self.config_manager.get_config())
            .map_err(|_| "Dictionary provider unavailable")?;

        // Run regular translation and dictionary lookup concurrently
        let (translation_result, dict_result) = tokio::join!(
            self.translate_text_internal(word, from, to),
            dictionary_provider.lookup(word, from, to)
        );

        let primary_translation = translation_result.ok().map(|translation| translation.text);

        match dict_result? {
            Some(entry) => {
                let corrected_word = entry.corrected_word.clone();
                let lines = article::article_lines(&entry, to, primary_translation.as_deref());
                Ok(DictionaryLookup {
                    formatted: article::to_plain(&lines),
                    lines,
                    corrected_word,
                    primary_translation,
                })
            }
            None => Err("Limited dictionary information available".into()),
        }
    }

    /// Returns the localized spelling-correction notice, colored for the terminal: the
    /// phrase in `notice_color`, the corrected word itself in `source_prompt_color`.
    pub fn correction_notice(
        corrected_word: &str,
        target_lang: &str,
        config: &config::Config,
    ) -> String {
        format!(
            "{} {}",
            config::colorize(Self::correction_phrase(target_lang), &config.notice_color),
            config::colorize(corrected_word, &config.source_prompt_color)
        )
    }

    /// The localized phrase preceding the corrected word in a correction notice.
    fn correction_phrase(target_lang: &str) -> &'static str {
        match target_lang {
            "ru" => "Показан перевод слова",
            "es" => "Mostrando traducción de la palabra",
            "fr" => "Traduction affichée pour le mot",
            "de" => "Übersetzung angezeigt für das Wort",
            "it" => "Traduzione mostrata per la parola",
            "pt" => "Tradução mostrada para a palavra",
            "zh" => "显示单词翻译",
            _ => "Showing translation for word",
        }
    }

    /// Public method to translate text
    pub async fn translate_text_public(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<Translation, Box<dyn Error + Send + Sync>> {
        self.translate_text_internal(text, from, to).await
    }

    /// Hide terminal window and restore previously active window
    /// Delays hiding if mouse cursor is over the terminal
    async fn hide_terminal_and_restore(&self, delay_seconds: u64) {
        let Some(wm) = &self.window_manager else {
            return;
        };

        // Wait specified time to let user see the result
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

        // Check if mouse is over terminal, and wait until it moves away
        loop {
            if !wm.is_mouse_over_terminal() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        // Restore the previously active window
        if let Ok(stored) = self.stored_foreground_window.lock() {
            if let Some(prev_window) = *stored {
                if let Err(e) = wm.set_foreground_window(prev_window) {
                    self.emit_line(format!("Failed to restore previous window: {}", e));
                }
            }
        }

        // Hide the terminal
        if let Err(e) = wm.hide_terminal() {
            self.emit_line(format!("Failed to hide terminal: {}", e));
        }
    }

    /// Check if text appears to be in expected language
    fn is_expected_language(&self, text: &str, language_code: &str) -> bool {
        match language_code {
            "en" => self.is_english_text(text),
            "ru" => self.is_russian_text(text),
            _ => true,
        }
    }

    /// Check if text contains English characters
    fn is_english_text(&self, text: &str) -> bool {
        let english_chars = text.chars().filter(|c| c.is_alphabetic()).count();
        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();

        if total_chars == 0 {
            return false;
        }

        let english_ratio = english_chars as f64 / total_chars as f64;
        english_ratio > 0.7 && text.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Check if text contains Russian characters
    fn is_russian_text(&self, text: &str) -> bool {
        let russian_chars = text
            .chars()
            .filter(|c| c.is_alphabetic() && (*c as u32) >= 0x0400 && (*c as u32) <= 0x04FF)
            .count();

        let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();

        if total_chars == 0 {
            return false;
        }

        let russian_ratio = russian_chars as f64 / total_chars as f64;
        russian_ratio > 0.3
    }

    /// The translation provider for `config`'s `translate_provider`, rebuilt when that
    /// value has changed since the provider in use was built (`/p`, or a hot reload of the
    /// file). A value that fails to build keeps the previous provider; the error is shown
    /// once per value.
    fn translation_provider(&self, config: &config::Config) -> Arc<dyn TranslationProvider> {
        self.active_translation_provider(config).1
    }

    /// [`Self::translation_provider`] together with the `translate_provider` value it was
    /// built from, which differs from `config`'s when that value failed to build.
    fn active_translation_provider(
        &self,
        config: &config::Config,
    ) -> (String, Arc<dyn TranslationProvider>) {
        let wanted = &config.translate_provider;
        let (name, provider, error) = {
            let mut active = self.translation.lock().unwrap();
            let mut error = None;
            if active.name.eq_ignore_ascii_case(wanted) {
                active.failed = None;
            } else if !active
                .failed
                .as_ref()
                .is_some_and(|failed| failed.eq_ignore_ascii_case(wanted))
            {
                match config.create_translate_provider() {
                    Ok(provider) => {
                        active.name = wanted.clone();
                        active.provider = Arc::from(provider);
                        active.failed = None;
                    }
                    Err(message) => {
                        active.failed = Some(wanted.clone());
                        error = Some(format!(
                            "Translation provider unavailable: {message}\n(keeping {})",
                            active.provider.name()
                        ));
                    }
                }
            }
            (active.name.clone(), active.provider.clone(), error)
        };
        if let Some(error) = error {
            self.emit_line(error);
        }
        (name, provider)
    }

    /// The dictionary provider for `config`'s `dictionary_provider`, or why there is none,
    /// rebuilt when that value has changed since the slot was filled (`/p d`, or a hot
    /// reload of the file). A value that fails to build keeps a working provider (the error
    /// is shown once per value); with no working provider, the new error replaces the old
    /// one, so the banner shows the current reason.
    fn dictionary_provider(
        &self,
        config: &config::Config,
    ) -> Result<Arc<dyn DictionaryProvider>, String> {
        let wanted = &config.dictionary_provider;
        let (provider, error) = {
            let mut active = self.dictionary.lock().unwrap();
            let mut error = None;
            if active.name.eq_ignore_ascii_case(wanted) {
                active.failed = None;
            } else if !active
                .failed
                .as_ref()
                .is_some_and(|failed| failed.eq_ignore_ascii_case(wanted))
            {
                match config.create_dictionary_provider() {
                    Ok(provider) => {
                        active.name = wanted.clone();
                        active.provider = Ok(Arc::from(provider));
                        active.failed = None;
                    }
                    Err(message) => {
                        let kept = active.provider.as_ref().ok().map(|p| p.name().to_string());
                        error = Some(match kept {
                            Some(kept) => {
                                active.failed = Some(wanted.clone());
                                format!(
                                    "Dictionary provider unavailable: {message}\n(keeping {kept})"
                                )
                            }
                            None => {
                                active.name = wanted.clone();
                                active.provider = Err(message.clone());
                                format!(
                                    "Dictionary provider unavailable: {message}; \
                                     dictionary lookups disabled"
                                )
                            }
                        });
                    }
                }
            }
            (active.provider.clone(), error)
        };
        if let Some(error) = error {
            self.emit_line(error);
        }
        provider
    }

    /// Translate text using translation provider
    async fn translate_text_internal(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<Translation, Box<dyn Error + Send + Sync>> {
        let (name, provider) = self.active_translation_provider(&self.config_manager.get_config());
        Ok(Translation {
            text: provider.translate_text(text, from, to).await?,
            provider: name.to_ascii_lowercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tagent::providers::{Definition, DictionaryEntry, PartOfSpeechEntry};

    struct MockProvider {
        translation: String,
        /// Counts `translate_text` calls, to assert a code path never reached the network.
        translate_calls: Arc<AtomicUsize>,
    }

    impl MockProvider {
        fn new(translation: &str) -> Self {
            Self {
                translation: translation.to_string(),
                translate_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    /// Returns a canned entry, or a miss when `entry` is `None`.
    struct MockDictionary {
        entry: Option<DictionaryEntry>,
    }

    #[async_trait::async_trait]
    impl DictionaryProvider for MockDictionary {
        async fn lookup(
            &self,
            _word: &str,
            _from: &str,
            _to: &str,
        ) -> Result<Option<DictionaryEntry>, tagent::error::Error> {
            Ok(self.entry.clone())
        }

        fn name(&self) -> &str {
            "mock dictionary"
        }
    }

    #[async_trait::async_trait]
    impl TranslationProvider for MockProvider {
        async fn translate_text(
            &self,
            _text: &str,
            _from: &str,
            _to: &str,
        ) -> Result<String, tagent::error::Error> {
            self.translate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.translation.clone())
        }

        async fn detect_language(&self, _text: &str) -> Result<String, tagent::error::Error> {
            Ok("en".to_string())
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_correction_phrase_russian() {
        assert_eq!(Translator::correction_phrase("ru"), "Показан перевод слова");
    }

    #[test]
    fn test_correction_phrase_english_fallback() {
        assert_eq!(
            Translator::correction_phrase("en"),
            "Showing translation for word"
        );
    }

    #[test]
    fn test_correction_notice_colors_the_word_with_the_source_prompt_color() {
        let config = config::Config {
            notice_color: "Magenta".to_string(),
            source_prompt_color: "Cyan".to_string(),
            ..config::Config::default()
        };
        assert_eq!(
            Translator::correction_notice("violent", "ru", &config),
            format!(
                "{} {}",
                config::colorize("Показан перевод слова", "Magenta"),
                config::colorize("violent", "Cyan")
            )
        );
    }

    #[test]
    fn test_correction_notice_plain_without_colors() {
        let config = config::Config {
            notice_color: String::new(),
            source_prompt_color: String::new(),
            ..config::Config::default()
        };
        assert_eq!(
            Translator::correction_notice("violent", "ru", &config),
            "Показан перевод слова violent"
        );
    }

    #[derive(Clone, Default)]
    struct MockPrinter {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl ExternalPrinter for MockPrinter {
        fn print(&mut self, msg: String) -> rustyline::Result<()> {
            self.messages.lock().unwrap().push(msg);
            Ok(())
        }
    }

    fn test_config_manager(unique: &str) -> Arc<ConfigManager> {
        let path = std::env::temp_dir().join(format!(
            "tagent_test_translator_{}_{}.toml",
            unique,
            std::process::id()
        ));
        std::fs::write(
            &path,
            "[translation]\nsource_language = \"Auto\"\ntarget_language = \"Russian\"\n\
             [interface]\ncopy_to_clipboard = false\n\
             [history]\nsave_translation_history = false\n",
        )
        .unwrap();
        let manager = Arc::new(ConfigManager::new(path.to_str().unwrap()).unwrap());
        let _ = std::fs::remove_file(&path);
        manager
    }

    /// Regression test for a bug where hotkey-triggered translations showed the
    /// `[Auto]: ` (now `[auto → ru]: `) prompt label and the translated text on separate lines.
    ///
    /// Root cause: `perform_translation` used to call `self.emit(&label)` and
    /// `self.emit_line(&text)` as two separate calls, which became two separate
    /// `ExternalPrinter::print()` invocations. rustyline's `State::external_print`
    /// unconditionally appends a newline to any message that doesn't already end
    /// with one, so the bare label (no trailing `\n`) was always forced onto its
    /// own line. The fix combines the label and the text into a single
    /// `emit_line` call so they travel through exactly one `print()` invocation.
    #[tokio::test]
    async fn hotkey_translation_emits_label_and_text_in_one_printer_call() {
        let config_manager = test_config_manager("label_line");
        let provider: Arc<dyn TranslationProvider> = Arc::new(MockProvider::new(
            "Добавлена постоянная дедуплицированная история ввода",
        ));
        let translator = Translator {
            translation: ActiveTranslation::new("google", provider),
            dictionary: ActiveDictionary::new("google", Err("no dictionary provider".to_string())),
            clipboard: ClipboardManager::new(),
            config_manager: config_manager.clone(),
            window_manager: None,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
        };

        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);

        let config = config_manager.get_config();
        let source_text = "Added persistent, deduplicated input history";
        translator
            .perform_translation(source_text, "auto", "ru", &config)
            .await
            .unwrap();

        let messages = captured.lock().unwrap();

        let source_line = messages
            .iter()
            .find(|m| m.contains(source_text))
            .unwrap_or_else(|| panic!("no message contained the source text: {:?}", *messages));
        assert!(
            source_line.starts_with("[auto → ru]: "),
            "label and source text must be emitted together, got: {:?}",
            source_line
        );

        let target_line = messages
            .iter()
            .find(|m| m.contains("дедуплицированная"))
            .unwrap_or_else(|| panic!("no message contained the translated text: {:?}", *messages));
        // The answer is labeled with the provider that translated, not the target language.
        assert!(
            target_line.starts_with("[google]: "),
            "label and translated text must be emitted together, got: {:?}",
            target_line
        );

        // No message should ever be just a bare "[...]: " label with nothing after it.
        assert!(
            !messages.iter().any(|m| {
                let trimmed = m.trim_end_matches('\n');
                trimmed.ends_with(": ") && trimmed.len() <= "[auto → ru]: ".len()
            }),
            "a label was emitted as its own print() call, split from its content: {:?}",
            *messages
        );
    }

    fn translator_with(
        provider: MockProvider,
        dictionary: Option<MockDictionary>,
        unique: &str,
    ) -> Translator {
        Translator {
            translation: ActiveTranslation::new("google", Arc::new(provider)),
            // Named after the test config's `dictionary_provider`, so the mock is kept.
            dictionary: ActiveDictionary::new(
                "google",
                dictionary
                    .map(|d| Arc::new(d) as Arc<dyn DictionaryProvider>)
                    .ok_or_else(|| "no dictionary provider".to_string()),
            ),
            clipboard: ClipboardManager::new(),
            config_manager: test_config_manager(unique),
            window_manager: None,
            stored_foreground_window: Arc::new(std::sync::Mutex::new(None)),
            printer: Arc::new(Mutex::new(None)),
            last_translation: Arc::new(Mutex::new(None)),
        }
    }

    /// The `Translator`/`DictionaryProvider` seam had no coverage before the provider split:
    /// the old mock's `get_dictionary_entry` always returned `None`.
    #[tokio::test]
    async fn dictionary_entry_is_formatted_and_reports_corrected_word() {
        let entry = DictionaryEntry::new(
            "violnt",
            vec![PartOfSpeechEntry::new(
                "adjective",
                vec![Definition::new("жестокий", vec!["violent".to_string()])],
            )],
        )
        .with_corrected_word("violent");
        let translator = translator_with(
            MockProvider::new("насилие"),
            Some(MockDictionary { entry: Some(entry) }),
            "dict_entry",
        );

        let DictionaryLookup {
            formatted,
            lines,
            corrected_word: corrected,
            primary_translation,
        } = translator
            .get_dictionary_entry("violnt", "en", "ru")
            .await
            .unwrap();

        assert_eq!(corrected.as_deref(), Some("violent"));
        assert_eq!(primary_translation.as_deref(), Some("насилие"));
        // Terminal mode: the translate provider's result is the header line.
        assert!(formatted.starts_with("насилие"), "got: {formatted:?}");
        assert!(formatted.contains("жестокий"), "got: {formatted:?}");
        assert!(formatted.contains("[violent]"), "got: {formatted:?}");
        // The plain text (clipboard, history) and the highlighted display share one layout.
        assert_eq!(formatted, article::to_plain(&lines));
    }

    #[tokio::test]
    async fn dictionary_miss_is_an_error_so_callers_fall_back_to_translation() {
        let translator = translator_with(
            MockProvider::new("ксиззик"),
            Some(MockDictionary { entry: None }),
            "dict_miss",
        );

        let result = translator.get_dictionary_entry("xyzzyq", "en", "ru").await;

        assert!(result.is_err());
    }

    /// A bad `dictionary_provider` must never break translation: with none available the
    /// lookup fails immediately, without touching the translate provider.
    #[tokio::test]
    async fn missing_dictionary_provider_fails_without_any_network_call() {
        let provider = MockProvider::new("не должно вызываться");
        let calls = provider.translate_calls.clone();
        let translator = translator_with(provider, None, "dict_none");

        let result = translator.get_dictionary_entry("violent", "en", "ru").await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    /// The banner names the translator's own providers, and says why the dictionary
    /// provider is missing when it couldn't be built.
    #[test]
    fn active_providers_name_the_providers_in_use() {
        let translator = translator_with(
            MockProvider::new("x"),
            Some(MockDictionary { entry: None }),
            "active_ok",
        );
        let providers = translator.active_providers(&config::Config::default());
        assert_eq!(providers.translation.as_deref(), Ok("mock"));
        assert_eq!(providers.dictionary.as_deref(), Ok("mock dictionary"));
        assert_eq!(providers.speech.as_deref(), Ok("Google TTS"));

        let translator = translator_with(MockProvider::new("x"), None, "active_none");
        let providers = translator.active_providers(&config::Config::default());
        assert_eq!(
            providers.dictionary,
            Err("no dictionary provider".to_string())
        );
    }

    /// `/s` and `/ss` replay what `perform_translation` (the hotkey path) records.
    #[tokio::test]
    async fn successful_translation_is_recorded_for_speech_replay() {
        let translator = translator_with(MockProvider::new("Привет, мир"), None, "last_ok");
        translator.set_external_printer(MockPrinter::default());
        let config = translator.config_manager.get_config();

        translator
            .perform_translation("Hello, world", "auto", "ru", &config)
            .await
            .unwrap();

        assert_eq!(
            translator.last_translation(),
            Some(LastTranslation {
                phrase: "Hello, world".to_string(),
                translation: Some("Привет, мир".to_string()),
                source_code: "auto".to_string(),
                target_code: "ru".to_string(),
            })
        );
    }

    /// A rejected phrase ("Text does not appear to be in ...") must not replace the
    /// previously recorded translation.
    #[tokio::test]
    async fn rejected_translation_keeps_previous_last_translation() {
        let translator = translator_with(MockProvider::new("мир"), None, "last_rejected");
        translator.set_external_printer(MockPrinter::default());
        let config = translator.config_manager.get_config();
        translator.record_last_translation("world", Some("мир"), "en", "ru");

        translator
            .perform_translation("Привет всем", "en", "ru", &config)
            .await
            .unwrap();

        let last = translator.last_translation().unwrap();
        assert_eq!(last.phrase, "world");
        assert_eq!(last.translation.as_deref(), Some("мир"));
    }

    /// Clones share the slot, so a hotkey translation is visible to interactive mode.
    #[test]
    fn last_translation_is_shared_between_clones() {
        let translator = translator_with(MockProvider::new(""), None, "last_shared");
        let interactive_copy = translator.clone();

        translator.record_last_translation("word", None, "auto", "ru");

        let last = interactive_copy.last_translation().unwrap();
        assert_eq!(last.phrase, "word");
        assert_eq!(last.translation, None);
    }

    /// A config whose `translate_provider` is `name`, with a `deepl` profile that builds
    /// offline (a dummy key; nothing is translated, so no request goes out).
    fn config_with_provider(translator: &Translator, name: &str) -> config::Config {
        let mut config = translator.config_manager.get_config();
        config.translate_provider = name.to_string();
        config
            .provider_options
            .insert("deepl", "api_key", "dummy-key:fx");
        config
    }

    #[test]
    fn translation_provider_is_kept_while_translate_provider_is_unchanged() {
        let translator = translator_with(MockProvider::new(""), None, "provider_same");
        let config = config_with_provider(&translator, "google");

        let first = translator.translation_provider(&config);
        let second = translator.translation_provider(&config);

        assert_eq!(first.name(), "mock");
        assert!(Arc::ptr_eq(&first, &second));
    }

    /// Profile names are case-insensitive in the factories, so a different spelling of the
    /// same name is not a switch.
    #[test]
    fn translation_provider_ignores_case_of_the_name() {
        let translator = translator_with(MockProvider::new(""), None, "provider_case");
        let config = config_with_provider(&translator, "Google");

        assert_eq!(translator.translation_provider(&config).name(), "mock");
    }

    #[test]
    fn changed_translate_provider_rebuilds_the_provider() {
        let translator = translator_with(MockProvider::new(""), None, "provider_switch");
        let config = config_with_provider(&translator, "deepl");

        let first = translator.translation_provider(&config);
        let second = translator.translation_provider(&config);

        assert_eq!(first.name(), "DeepL");
        assert!(Arc::ptr_eq(&first, &second), "built once, then kept");
    }

    /// The hotkey path and interactive mode hold clones of one `Translator`; a switch made
    /// through either must reach both.
    #[test]
    fn provider_switch_is_shared_between_clones() {
        let translator = translator_with(MockProvider::new(""), None, "provider_clones");
        let hotkey_copy = translator.clone();
        let config = config_with_provider(&translator, "deepl");

        let switched = translator.translation_provider(&config);

        let active = hotkey_copy.translation.lock().unwrap();
        assert_eq!(active.name, "deepl");
        assert!(Arc::ptr_eq(&active.provider, &switched));
    }

    /// A hot reload that brings an unusable `translate_provider` keeps the previous provider
    /// and reports the error once, not on every translation.
    #[test]
    fn failed_rebuild_keeps_previous_provider_and_reports_once() {
        let translator = translator_with(MockProvider::new(""), None, "provider_failed");
        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);
        let bad = config_with_provider(&translator, "no-such-provider");

        assert_eq!(translator.translation_provider(&bad).name(), "mock");
        assert_eq!(translator.translation_provider(&bad).name(), "mock");

        let messages = captured.lock().unwrap().clone();
        assert_eq!(messages.len(), 1, "{messages:?}");
        assert!(messages[0].contains("no-such-provider"), "{messages:?}");
        assert!(messages[0].contains("keeping mock"), "{messages:?}");

        // Back to the working value and then to the bad one again: reported again.
        let good = config_with_provider(&translator, "google");
        translator.translation_provider(&good);
        translator.translation_provider(&bad);
        assert_eq!(captured.lock().unwrap().len(), 2);
    }

    /// The answer's label names the provider that translated: after a switch that failed to
    /// build, the kept provider, not the value in the config.
    #[tokio::test]
    async fn translation_label_names_the_provider_actually_used() {
        let translator = translator_with(MockProvider::new("Привет"), None, "label_failed");
        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);
        translator.config_manager.set_provider(
            tagent::providers::ProviderAxis::Translation,
            "no-such-provider",
        );
        let config = translator.config_manager.get_config();

        translator
            .perform_translation("Hello", "auto", "ru", &config)
            .await
            .unwrap();

        let messages = captured.lock().unwrap();
        assert!(
            messages.iter().any(|m| m.starts_with("[google]: Привет")),
            "{messages:?}"
        );
        assert!(
            !messages
                .iter()
                .any(|m| m.starts_with("[no-such-provider]: ")),
            "{messages:?}"
        );
    }

    /// A profile name is the label, lowercased (profile names are case-insensitive).
    #[tokio::test]
    async fn translation_label_is_the_lowercased_profile_name() {
        let translator = translator_with(MockProvider::new(""), None, "label_profile");
        // The mock stands in for a profile built from `translate_provider = "DeepL-Work"`.
        translator.translation.lock().unwrap().name = "DeepL-Work".to_string();
        translator
            .config_manager
            .set_provider(tagent::providers::ProviderAxis::Translation, "DeepL-Work");

        let translation = translator
            .translate_text_internal("Hello", "en", "ru")
            .await
            .unwrap();

        assert_eq!(translation.provider, "deepl-work");
    }

    #[test]
    fn active_providers_names_the_switched_provider() {
        let translator = translator_with(MockProvider::new(""), None, "provider_banner");
        let config = config_with_provider(&translator, "deepl");

        let providers = translator.active_providers(&config);

        assert_eq!(providers.translation, Ok("DeepL".to_string()));
    }

    /// A config whose `dictionary_provider` is `name`, with an `ollama` profile of kind
    /// `openai` that builds offline (nothing is looked up, so no request goes out).
    fn config_with_dictionary(translator: &Translator, name: &str) -> config::Config {
        let mut config = translator.config_manager.get_config();
        config.dictionary_provider = name.to_string();
        config.provider_options.insert("ollama", "type", "openai");
        config
            .provider_options
            .insert("ollama", "endpoint", "http://localhost:11434/v1");
        config.provider_options.insert("ollama", "model", "dummy");
        config
    }

    fn mock_dictionary() -> Option<MockDictionary> {
        Some(MockDictionary { entry: None })
    }

    #[test]
    fn dictionary_provider_is_kept_while_dictionary_provider_is_unchanged() {
        let translator = translator_with(MockProvider::new(""), mock_dictionary(), "dict_same");
        let config = config_with_dictionary(&translator, "GOOGLE");

        let first = translator.dictionary_provider(&config).unwrap();
        let second = translator.dictionary_provider(&config).unwrap();

        assert_eq!(first.name(), "mock dictionary", "case is ignored");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn changed_dictionary_provider_rebuilds_the_provider() {
        let translator = translator_with(MockProvider::new(""), mock_dictionary(), "dict_switch");
        let config = config_with_dictionary(&translator, "ollama");

        let first = translator.dictionary_provider(&config).unwrap();
        let second = translator.dictionary_provider(&config).unwrap();

        assert_eq!(first.name(), "OpenAI-compatible (ollama)");
        assert!(Arc::ptr_eq(&first, &second), "built once, then kept");
    }

    /// The hotkey path and interactive mode hold clones of one `Translator`.
    #[test]
    fn dictionary_switch_is_shared_between_clones() {
        let translator = translator_with(MockProvider::new(""), mock_dictionary(), "dict_clones");
        let hotkey_copy = translator.clone();
        let config = config_with_dictionary(&translator, "ollama");

        let switched = translator.dictionary_provider(&config).unwrap();

        let active = hotkey_copy.dictionary.lock().unwrap();
        assert_eq!(active.name, "ollama");
        assert!(Arc::ptr_eq(active.provider.as_ref().unwrap(), &switched));
    }

    #[test]
    fn failed_dictionary_rebuild_keeps_working_provider_and_reports_once() {
        let translator = translator_with(MockProvider::new(""), mock_dictionary(), "dict_failed");
        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);
        let bad = config_with_dictionary(&translator, "no-such-provider");

        assert_eq!(
            translator.dictionary_provider(&bad).unwrap().name(),
            "mock dictionary"
        );
        assert_eq!(
            translator.dictionary_provider(&bad).unwrap().name(),
            "mock dictionary"
        );

        let messages = captured.lock().unwrap().clone();
        assert_eq!(messages.len(), 1, "{messages:?}");
        assert!(messages[0].contains("no-such-provider"), "{messages:?}");
        assert!(
            messages[0].contains("keeping mock dictionary"),
            "{messages:?}"
        );

        // Back to the working value and then to the bad one again: reported again.
        let good = config_with_dictionary(&translator, "google");
        translator.dictionary_provider(&good).unwrap();
        translator.dictionary_provider(&bad).unwrap();
        assert_eq!(captured.lock().unwrap().len(), 2);
    }

    /// A startup failure is not permanent: a value that builds replaces it.
    #[test]
    fn unavailable_dictionary_recovers_when_a_good_value_arrives() {
        let translator = translator_with(MockProvider::new(""), None, "dict_recover");
        translator.dictionary.lock().unwrap().name = "broken".to_string();
        let config = config_with_dictionary(&translator, "google");

        let provider = translator.dictionary_provider(&config).unwrap();

        assert_eq!(provider.name(), "Google Dictionary");
        assert_eq!(
            translator.active_providers(&config).dictionary.as_deref(),
            Ok("Google Dictionary")
        );
    }

    /// With no working provider to keep, a new bad value replaces the stored error, so the
    /// banner names the current reason; it is reported once.
    #[test]
    fn unavailable_dictionary_stores_the_new_error() {
        let translator = translator_with(MockProvider::new(""), None, "dict_new_error");
        let printer = MockPrinter::default();
        let captured = printer.messages.clone();
        translator.set_external_printer(printer);
        let bad = config_with_dictionary(&translator, "no-such-provider");

        let error = translator.dictionary_provider(&bad).err().unwrap();
        assert!(error.contains("no-such-provider"), "{error}");
        assert_eq!(
            translator.dictionary_provider(&bad).err(),
            Some(error.clone())
        );
        assert_eq!(translator.active_providers(&bad).dictionary, Err(error));
        assert_eq!(
            translator.dictionary.lock().unwrap().name,
            "no-such-provider"
        );
        assert_eq!(captured.lock().unwrap().len(), 1);
    }
}
