# Changelog

All notable changes to Tagent Text Translator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) with build numbers.

## [Unreleased]

### Added
- **`tagent-gui` transcript pane is now selectable/copyable** (`tagent-gui/ui/app.slint`): the transcript display was a plain, non-interactive `Text` element; it is now a read-only, multi-line `TextInput` so translated text can be selected with the mouse and copied.
- **`tagent-gui` transcript auto-scrolls to the newest entry** (`tagent-gui/src/main.rs`, `tagent-gui/ui/app.slint`): appending a translation (or the "Auto" target error) now calls `scroll_transcript_to_bottom()`, which reads the new `transcript-viewport-height`/`transcript-visible-height` properties and sets `transcript-viewport-y` to the overflow, so long transcripts no longer require manually scrolling down after each translation.
- **`tagent-gui` input box is resizable and multi-line** (`tagent-gui/ui/app.slint`): replaced the single-line `LineEdit` with a multi-line `TextInput` in its own `ScrollView`, plus a drag handle above it that lets the user resize the box between 32px and 220px (`input-user-height`); the box also grows automatically to fit wrapped text up to that cap. Enter submits the text (same as before); Shift+Enter now inserts a newline instead of submitting.
- **`tagent-gui` grabs input focus on launch** (`tagent-gui/ui/app.slint`): added `forward-focus: input-field` to the window root, so typing or pasting works immediately without first clicking into the input box.

### Removed
- **Dead Tauri-era version-sync code in the root `build.rs`**: `sync_version_in_gui()` and `update_gui_cargo_version()` synced the version into `tagent-gui/src-tauri/Cargo.toml`, `tagent-gui/ui/package.json`, and `tagent-gui/src-tauri/tauri.conf.json` — none of which exist since `tagent-gui` moved to Slint. Every build silently no-op'd on missing files; removed the dead functions and their call site.

### Fixed
- **`tagent-gui` ignored `[Provider] TranslateProvider` in `tagent.conf`** (`tagent-gui/src/main.rs`): the translate button always called `providers::create_provider("google")` regardless of the user's configured provider. Now reads the provider from `ConfigManager` at startup, same as the main `tagent` binary.
- **`tagent-gui` allowed "Auto" as the target language** (`tagent-gui/src/main.rs`): selecting "Auto" as the target (directly via the combo box, or via the ⇄ swap button when the source was "Auto") sent an invalid `to=auto` translation request. Swapping now falls back to "English" instead of carrying "Auto" into the target slot, and `on_translate_requested` rejects `to == "auto"` with an in-transcript error as a second guard.

### Changed
- **Documentation overhaul**: `CLAUDE.md`'s Architecture section described a stale, Windows-only module layout (flat `keyboard.rs`/`clipboard.rs`/`window.rs`) that no longer matched the codebase after the `src/platform/{linux,macos,windows}/` split and the addition of the `tagent-gui` crate. Rewrote the Core Modules, Platform Abstraction, Provider Trait, Configuration System, and Development Notes sections to reflect current reality, including the `[Colors]`/`[Speech]` config sections, the `detect_language` provider method, and cross-platform config file paths. Added `docs/ARCHITECTURE.md` for implementation-level detail that doesn't belong in `CLAUDE.md`'s day-to-day guidance: the platform abstraction mechanism, per-platform feature parity (macOS is a near-complete stub), `tagent-gui` internals and its gaps (hardcoded provider, no config integration), and a stale Tauri-era version-sync leftover in the root `build.rs`.
- **Added `#![warn(missing_docs)]` to `src/lib.rs`** and documented all previously-undocumented public items across `config.rs`, `platform/{linux,macos,windows}/*`, `providers/*`, `speech.rs`, and `translator.rs` (51 warnings on the Linux build, all resolved). `CLAUDE.md` already claimed this lint was enabled; it wasn't — the claim is now accurate.
- **Documentation follow-up pass** (`CLAUDE.md`, `docs/ARCHITECTURE.md`): both files still described `tagent-gui` as hardcoding `providers::create_provider("google")`, which was fixed in the entries above — updated to describe the actual startup-only config read (and called out that it's not live-reloaded, unlike the main `tagent` binary). Documented the `tagent-gui` UI changes added above (selectable transcript, auto-scroll, resizable multi-line input, focus-on-launch). Rewrote `docs/ARCHITECTURE.md`'s `build.rs` section, which still described a `sync_version_in_gui()` step targeting a Tauri-era `tagent-gui/src-tauri/` layout that was already removed from `build.rs` (see "Removed" above) — the section now describes only the doc-version-sync and Windows-resource steps that actually run. Corrected the claim that `TranslationProvider::detect_language` has no call site: it's used by `speech.rs`'s `detect_speech_language()` for TTS auto-language-detection, just not from `translator.rs`.
- **Split the single `tagent` crate into a three-crate workspace: `tagent` (library), `tagent-cli` (application, formerly the app half of `tagent`), and `tagent-gui`.** The library now owns `providers` (`TranslationProvider` trait + Google Translate implementation, including TTS), a new `languages` module (the name ↔ BCP-47 code table, moved out of `ConfigManager`), and a new unified `thiserror`-based `error::Error` type replacing `Box<dyn Error + Send + Sync>` at the provider boundary. `tagent-cli` keeps the application (hotkeys, interactive terminal, CLI, config, history, clipboard, platform integration) and its existing `MAJOR.MINOR.PATCH+BUILD` version convention; its built executable is still named `tagent`, so `target/release/tagent` and existing install instructions are unaffected — only the package name and source layout changed (`tagent-cli/src/...`). `tagent-gui` now depends on the `tagent` library directly instead of on `tagent-cli`'s (now-removed) `[lib]` target, using a small self-contained `TranslateProvider` reader in place of `ConfigManager` to avoid pulling in `tagent-cli`'s platform-integration dependencies. The `TranslationProvider` trait's TTS methods are `split_for_speech`/`speak_chunk` (one chunk at a time) rather than a single `speak()` returning every chunk up front, preserving `tagent-cli`'s existing fetch-then-play-per-chunk behavior instead of waiting on the whole utterance before any audio starts. See `docs/ARCHITECTURE.md`'s "Workspace layout" section for the full breakdown.

### Fixed
- **`build.rs`'s version sync silently deleted the CHANGELOG's `## [Unreleased]` section** (`build.rs`): `update_version_in_file`'s pattern search for `"## [0.13.0+002] - "` matched the `## [Unreleased]` header first, then scanned forward for the next `"] - "`, which only occurs at the *next real* version header — replacing everything in between (the entire Unreleased section) with just the version string. Found by hitting it firsthand: a freshly-written Unreleased entry vanished after the next `cargo build`. The scan now recognizes and skips `"## [Unreleased]"` headers instead of treating them as a version to replace.

## [0.13.0+003] - 2026-08-05

### Fixed
- **Hotkey-triggered translation split the `[Lang]:` label and its text onto separate lines** (`translator.rs`): `perform_translation` (and the single-word dictionary path) called `self.emit(&label)` followed by a separate `self.emit_line(&text)`, which became two independent `ExternalPrinter::print()` calls when the interactive prompt was active. rustyline's `State::external_print` unconditionally appends a newline to any message that doesn't already end with one, so the bare label — having no trailing `\n` — was always forced onto its own line, ahead of the text. Label and content are now built into a single string and emitted through one `emit_line` call, so they always travel through the same `print()` invocation. Added a regression test (`translator::tests::hotkey_translation_emits_label_and_text_in_one_printer_call`) using a mock `ExternalPrinter` that fails if a label is ever emitted as its own call.

## [0.13.0+001] - 2026-08-05

### Added
- **`rustyline`-based line editing for interactive mode** (`interactive.rs`): replaced the bare `io::stdin().read_line()` loop with a `rustyline` `Editor`, adding arrow-key/Emacs-style line editing, Ctrl+A/E, Ctrl+R history search, and Tab-completion for slash-commands (`/help`, `/config`, `/lang`, `/save`, `/speech`, `/clear`, `/quit`, `/version`, etc.). Input history now persists across sessions in a dedicated `interactive_history.txt` (separate from the translation-results history file), capped at 1000 entries with consecutive duplicates ignored.
- Ctrl+C at the interactive prompt now behaves like a real shell: it clears the current line and reprints the prompt without exiting. Only Ctrl+D on an empty line (or `/quit`/`/exit`/`/q`) exits.

### Changed
- **Hotkey-triggered translation output no longer corrupts the interactive prompt** (`translator.rs`, `interactive.rs`): output from `Translator::translate_clipboard()` (invoked from the keyboard-hook thread) is now routed through a rustyline `ExternalPrinter` when the interactive prompt is active, so it can no longer interleave with and corrupt the prompt/typed-so-far text if the hotkey fires while the user is mid-typing.

### Fixed
- **Double Ctrl+C left the shell in raw mode** (`platform/linux/signals.rs`): the Ctrl+C signal handler's second-press `std::process::exit(1)` skipped `Drop`, so rustyline's terminal-mode guard never ran and the shell was left in raw mode after exit. The handler no longer force-exits; Ctrl+C is now handled in-band via `ReadlineError::Interrupted` in `InteractiveMode::start()`, which always goes through rustyline's normal (`Drop`-respecting) exit path.

## [0.13.0+001] - 2026-08-04

### Fixed
- **No timeout on the Google Translate HTTP client** (`providers/google.rs`): `Client::new()` had no request timeout, so a hung connection or unresponsive endpoint blocked the calling task forever. Added a 10-second timeout (matching `speech.rs`) and a clear "Translation request timed out" message applied at every `.send()`/`.text()` call site (a stall during body read raises the same timeout error as a stalled connect, just on the second call).
- **`parse_ini` discarded keys from a repeated INI section** (`config.rs`): a config file with the same section header appearing twice (e.g. after a manual hand-edit) silently dropped every key parsed under the first occurrence, since each `[Section]` line unconditionally inserted a fresh, empty `HashMap`. Now uses `.entry(section_name).or_default()` so keys from repeated sections merge instead of overwriting.
- **Interactive prompt did not reappear after an error** (`translator.rs`): `perform_translation` only redrew the `[lang]:` prompt on the success path; a language mismatch or a translation error left the prompt missing, making the interactive session look stuck. Both branches now call `print_source_prompt` too.
- **Retry request errors and language-detection fallback were silently masked as "not found"** (`providers/google.rs`): `get_dictionary_entry`'s spell-correction retry fell through to `Ok(None)` on a non-2xx HTTP status, indistinguishable from a genuinely unknown word; the retry branch now returns an explicit HTTP error. `detect_language`'s fallback to `"en"` on an unexpected response shape now logs a diagnostic instead of failing silently.
- **`WindowManager` was declared `Send + Sync` without `XInitThreads()`** (`platform/linux/signals.rs`, `platform/linux/window.rs`): `WindowManager` and `XGrabManager` each open independent X11 `Display` connections from different threads (main thread vs. the keyboard-hook task) with no prior `XInitThreads()` call anywhere in the process — a real concurrency hazard, not just a documentation gap. `XInitThreads()` is now called as the first Xlib action in `platform::linux::signals::setup()`, which already runs before either manager is constructed.
- **AltGr/right-Alt combos were grabbed only under `Mod1Mask`, leaking the keystroke to the focused app** (`platform/linux/xgrab.rs`): on keyboard layouts where the physical AltGr key produces `Mod5` (`ISO_Level3_Shift`) rather than `Mod1`, translation still fired correctly (Linux hotkey detection goes through `rdev`, not `XGrabKey`), but `XGrabKey` never suppressed the `Mod5` variant, so the keystroke also leaked through to whatever application had focus. Alt-containing combos are now grabbed under both `Mod1Mask` and `Mod5Mask`. Also added a process-wide `XSetErrorHandler` that logs and continues instead of aborting the process on a conflicting `XGrabKey` (`BadAccess`), a pre-existing crash risk that doubling the grab count made more likely to hit.

## [0.13.0+001] - 2026-08-03

### Fixed
- **`EnableTextToSpeech` defaulted to off when loading a config file missing the `[Speech]` section** (`config.rs`): `Config::default()` set `enable_text_to_speech` to `true`, but `load_config()`'s fallback for a missing/absent key was `.unwrap_or(false)` — the only boolean setting with this divergence. A config file predating the `[Speech]` section, or a hand-edited one omitting `EnableTextToSpeech`, silently loaded TTS as disabled, and `/save` would then persist that `false` permanently to disk. The fallback now matches `Default` (`.unwrap_or(true)`).

## [0.13.0+001] - 2026-08-03

### Fixed
- **A modifier in a combo hotkey was blocked system-wide on Windows** (`platform/windows/keyboard.rs`): `HotkeyState::handle`'s `ModifierCombo` arm blocked (returned `true` in the low-level keyboard hook) every press/release of a configured modifier unconditionally, not just at the moment the combo actually completed. With e.g. `TranslateHotkey = Alt+Q` configured, this meant Alt stopped working as a modifier system-wide for as long as tagent was running — breaking Alt-Tab, the system menu, Alt+F4, and any other app's Alt-based shortcuts. The modifier-tracking branch now only records state without blocking; blocking still happens only for the combo's target key, and only once all modifiers are confirmed pressed.

## [0.13.0+001] - 2026-08-03

### Fixed
- **Clipboard write warning on Linux/X11** (`platform/linux/clipboard.rs`): `set_text`/`get_text` created a new `arboard::Clipboard` per call and dropped it immediately afterward. On X11, clipboard ownership is process-based — the background thread that serves other apps' paste requests is spawned in `Clipboard::new()` and dies when the value is dropped, so clipboard managers polling a moment later could miss the contents (arboard's own "Clipboard was dropped very quickly after writing" warning). A single `arboard::Clipboard` is now kept alive for the process lifetime (guarded by a `Mutex`, following the same pattern already used for X11 key state in `keycodes.rs`) and reused across calls.

## [0.13.0+001] - 2026-08-03

### Fixed
- **Left/right-specific modifiers never triggered the hotkey** (`config.rs`): a hotkey configured with a side-specific modifier (e.g. `LAlt+Q`, `RCtrl+T`) or a side-specific double-press (e.g. `LCtrl+LCtrl`) passed validation but never fired. `HotkeyParser::parse` stored the specific left/right virtual-key code, while both the Windows and Linux keyboard hooks always normalize the *observed* key event to its generic form before comparing, so the two could never match. `HotkeyParser::parse` now normalizes the configured modifier codes (and the double-press target) the same way, so side-specific hotkeys work like their generic counterparts. The trigger key itself (the last part of a modifier combo) is left untouched, since it was already matched against the raw observed code and side-specific trigger keys already worked.

## [0.13.0+001] - 2026-03-11

### Fixed
- **Panic on UTF-8 slice boundary in TTS** (`speech.rs`): text chunking for text-to-speech computed byte-slice boundaries as `start + MAX_TEXT_LENGTH` without checking for UTF-8 character boundaries, causing a process-crashing panic (`panic = "abort"` in release builds) when a long non-ASCII "word" or fallback text was split mid-character. Added a `floor_char_boundary` helper that rounds the slice index down to the nearest valid character boundary before slicing.

### Added
- **Spell checking for single words**: when a misspelled word is looked up, the correct word is found automatically and a correction notice is shown in the target language (e.g. "Показан перевод слова violent")
  - Two detection scenarios: silent auto-correction by Google (`json[0][0][1]`) and explicit suggestion via `dt=qca` field (`json[7][1]`) with a retry request
  - Notice is localized: Russian, English, Spanish, French, German, Italian, Portuguese, Chinese
  - `DictionaryEntry` now carries `corrected_word: Option<String>`
  - `Translator::correction_notice(word, lang)` public helper
- **`SpellCheck` config option** in `[Dictionary]` section (default: `true`)
  - `SpellCheck = false` disables the correction notice; misspelled words fall back to simple translation

### Fixed
- `cargo test` linking error (`LNK1123: duplicate resource`): changed `cargo:rustc-link-arg` to `cargo:rustc-link-arg-bins` in `build.rs` so `resource.lib` is only linked into binary targets, not test binaries

### Changed
- Dictionary entry display: `[Word]:` now shows the primary translation (from the translate API) instead of the first dictionary definition, matching the result shown when dictionary is disabled
- Dictionary entry layout: primary translation appears on the `[Word]:` line, part-of-speech sections start on the next line

### Added
- Wayland compatibility: application now starts and works on Wayland sessions
  - Interactive mode and CLI mode fully functional on Wayland
  - Window management gracefully disabled with informative message
  - CLI mode skips window management entirely (no warnings)
- `Translator::new_cli()` constructor for CLI mode without window management overhead

### Fixed
- **Double free crash** in `get_active_window()` (`platform/linux/window.rs`): `XFree(prop)` was called twice when `_NET_ACTIVE_WINDOW` returned window=0 (common on XWayland), causing `free(): double free detected in tcache 2` abort
- Eliminated duplicate `WindowManager` initialization: `Translator` is now created once and shared between `KeyboardHook` and `InteractiveMode` via `InteractiveMode::with_translator()`
- Removed duplicate "Window management disabled" warning that appeared twice on startup

### Changed
- `WindowManager` is now optional in `Translator` (`Option<Arc<WindowManager>>`): failure to initialize window management no longer crashes the application
- `InteractiveMode::new_with_config()` replaced by `InteractiveMode::with_translator()` to accept a shared `Translator` instance

---

## [0.13.0+001] - 2026-02-14

### Added
- Linux X11 window management implementation for auto-hide feature
  - `show_terminal()`: Uses `XMapRaised` + `_NET_ACTIVE_WINDOW` client message
  - `hide_terminal()`: Uses `XIconifyWindow` to minimize the terminal
  - `get_foreground_window()` / `set_foreground_window()`: Read/write `_NET_ACTIVE_WINDOW` property
  - `is_mouse_over_terminal()`: Uses `XQueryPointer` + `XGetGeometry` for cursor hit-testing
  - Terminal window found via `_NET_WM_PID` property matching against process ID

### Changed
- Default translation hotkey changed from `Ctrl+Ctrl` to `Alt+Q`
- Major refactoring: eliminated code duplication across modules
  - `save_translation_history()` consolidated into shared function in `config.rs` (was duplicated 3x)
  - `is_single_word()` consolidated into shared function in `config.rs` (was duplicated 3x)
  - Added `print_colored()` shared helper to eliminate 9+ repeated color-printing patterns
  - Merged duplicate `get_dictionary_entry`/`get_dictionary_entry_cli`/`get_dictionary_entry_public` into single public `get_dictionary_entry` method
  - Fixed `Arc<Box<dyn TranslationProvider>>` double indirection to `Arc<dyn TranslationProvider>`
  - Deduplicated keyboard hotkey handlers (~280 lines to ~130 lines) via `HotkeyState` struct
  - Fixed `CliHandler` creating duplicate `ConfigManager` (now shares one instance with `Translator`)
  - Removed dead code: `InteractiveMode::new()`, `Translator::new()`, and 4 unused `WindowManager` methods
  - Translated all Russian comments to English

### Added
- `/save` command in interactive mode to save current configuration to file
- Language switching command `/l` (`/lang`) for interactive mode
  - `/l` or `/lang` without arguments swaps source and target languages (Auto becomes English before swapping)
  - `/l <target>` sets target language with source=Auto (e.g., `/l French`)
  - `/l <source> <target>` sets both languages (e.g., `/l English German`)
  - Accepts both language names (`English`, `German`) and codes (`en`, `de`)
- CLI flag `-l` (`--lang`) for setting languages before translation
  - `tagent -l German hello` translates "hello" to German
  - `tagent -l en de "Hello world"` translates from English to German
- `ConfigManager::set_languages()` method for in-memory language updates
- `ConfigManager::code_to_language()` reverse lookup (code to language name)
- `ConfigManager::normalize_language()` to accept both names and codes as input

## [0.13.0+001] - 2026-01-15

### Fixed
- Text-to-speech: suppressed "Dropping OutputStream, audio playing through this stream will stop" console message
  - Used rodio's `log_on_drop(false)` method to disable the warning on stream drop
- Dictionary mode: unified format between hotkey and interactive modes
  - Hotkey mode now matches interactive mode format: `[Auto]: word` then `[Word]: Part of Speech...`
  - Example: `[Auto]: auction` then `[Word]: Существительное` (not separate translation line)
  - Dictionary definitions now start immediately after `[Word]:` label

## [0.13.0+001] - 2026-01-13

### Fixed
- Dictionary mode: corrected display order - now shows original word in `[Auto]` line and translation in `[Word]` line
  - Example: `[Auto]: auction` (original) then `[Word]: аукцион` (translation)
  - Previously showed translation in `[Auto]` line which was incorrect

## [0.13.0+001] - 2026-01-13

### Fixed
- Dictionary mode: added simple translation prompt before dictionary entry
- Fixed missing word translation in dictionary mode when translating via hotkeys

## [0.13.0+001] - 2026-01-13

### Fixed
- Double-press hotkey pattern now prevents false triggering when using Copy/Paste shortcuts (Ctrl+C, Ctrl+V)
- Added sequence interruption detection: if another key is pressed between two target key presses, the double-press sequence is reset
- Added `LAST_KEY_INTERRUPTED` and `SPEECH_LAST_KEY_INTERRUPTED` state tracking to detect interrupted sequences
- Ensures Ctrl+C followed by Ctrl+V (within double-press time window) no longer triggers translation hotkey

## [0.13.0+001] - 2026-01-13

### Fixed
- Double-press hotkey pattern now ignores key auto-repeat when key is held down
- Prevents false triggering of translation/speech when holding down a configured double-press key
- Added `LAST_KEY_PRESSED` and `SPEECH_LAST_KEY_PRESSED` state tracking to detect first press vs auto-repeat

## [0.13.0+001] - 2026-01-08

### Changed
- Documentation: updated README with comprehensive text-to-speech documentation
- Documentation: added hotkey configuration examples and explanations
- Documentation: added terminal color customization guide

## [0.13.0+001] - 2026-01-08

### Changed
- Code cleanup: removed unused speech methods to eliminate compilation warnings
- Code quality improvements: fixed all Clippy warnings and lints
- Code formatting: applied rustfmt to entire codebase for consistent style

### Fixed
- Speech: fixed text splitting for very long words without spaces (improves TTS for edge cases)

## [0.13.0+001] - 2026-01-06

### Added
- Interactive mode speech commands: `/s <text>` and `/speech <text>`
- Text-to-speech support in interactive mode with Esc cancellation
- Speech language determined by `SourceLanguage` configuration (Auto → English)

## [0.13.0+001] - 2026-01-06

### Fixed
- Added Esc key cancellation support for CLI speech mode (`-s`/`--speech`)
- Speech can now be interrupted by pressing Esc in both CLI and hotkey modes

## [0.13.0+001] - 2026-01-06

### Changed
- Speech language now determined by `SourceLanguage` configuration setting
- When `SourceLanguage` is set to "Auto", English is used by default for speech

## [0.13.0+001] - 2026-01-06

### Fixed
- Speech language detection now based on text content (Cyrillic → Russian, Latin → English)

## [0.13.0+001] - 2026-01-06

### Added
- CLI speech command: `-s, --speech` for text-to-speech functionality
- Text-to-speech support using Google Translate TTS API
- Automatic language detection for speech

### Changed
- Help text updated with speech command examples

## [0.13.0+001] - 2025-12-24

### Added
- Automatic version synchronization system in build.rs
- Version now syncs from Cargo.toml to all documentation files automatically
- Detailed version sync reporting in `.debug/` folder

### Changed
- Interactive mode commands now use slash prefix (/) instead of dash (-/--) for better consistency
  - Commands: /h, /help, /?, /c, /config, /v, /version, /clear, /cls, /q, /quit, /exit
- Default alternative hotkey changed from F9 to Alt+Q for better ergonomics
- Configuration moved to AppData folder (`%APPDATA%\Tagent\`)
- Automatic directory creation for config and history files
- Better compliance with Windows application standards
- Cleaner project structure (no config files in program directory)

### Improved
- Documentation updated to reflect automatic version synchronization
- CLAUDE.md now includes detailed version sync mechanism description

### Fixed
- Speech error messages now display correctly with prompt on a new line

## [0.13.0+001] - 2025-XX-XX

### Added
- Translation history logging with timestamps
- Configurable history file path
- Multi-line format for better readability
- History works in all modes (GUI, CLI, Interactive)

### Changed
- History file now defaults to AppData folder location

## [0.13.0+001] - 2024-XX-XX

### Added
- Unified interface: GUI hotkeys + Interactive terminal
- Interactive commands with smart recognition
- Simultaneous operation of all translation modes
- Enhanced command-line interface

### Changed
- Application now runs in unified mode by default (GUI + Interactive)
- Improved terminal interaction experience

## [0.13.0+001] - 2024-XX-XX

### Added
- Basic GUI hotkey functionality (Ctrl+Ctrl double-press)
- CLI translation support
- Dictionary lookup feature
- Configuration management with INI format
- Multi-language support
- Google Translate API integration
- Clipboard operations
- Terminal window management

### Features
- Double-Ctrl hotkey for system-wide translation
- Dictionary definitions with part of speech
- Auto-detection of source language
- Configurable target languages
- Real-time configuration reload

## Version Format

Versions follow the format: `MAJOR.MINOR.PATCH+BUILD`

- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backward compatible)
- **PATCH**: Bug fixes (backward compatible)
- **BUILD**: Incremental build number (resets on version change)

Example: `0.8.0+022` = Version 0.8.0, Build 22

[Unreleased]: https://github.com/holgertkey/tagent-win/compare/v0.8.0+022...HEAD
[0.8.0+022]: https://github.com/holgertkey/tagent-win/compare/v0.8.0...v0.8.0+022
[0.8.0]: https://github.com/holgertkey/tagent-win/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/holgertkey/tagent-win/compare/v0.6.0...v0.7.0
[0.6.0 and Earlier]: https://github.com/holgertkey/tagent-win/releases/tag/v0.6.0
