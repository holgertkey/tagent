# Changelog

All notable changes to the `tagent` library crate are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent` is versioned independently of its two applications: see
[`tagent-cli/CHANGELOG.md`](../tagent-cli/CHANGELOG.md) and
[`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md) for theirs. A change to this
crate that an application has to adapt to is also recorded in that application's own
changelog.

While the version is `0.y.z`, a breaking change to the public API bumps the minor
version (`0.17` → `0.18`) and a compatible addition or fix bumps the patch
(`0.17.0` → `0.17.1`). `1.0.0` waits until the API settles.

## [Unreleased]

## [0.17.0] - 2026-09-19

First entry in this changelog. It also resets the crate's version from `1.0.0` to a
pre-1.0 `0.17.0`: the API is still moving and the crate isn't published, so `1.0.0`
promised a stability it doesn't have. `0.17.0` still sorts above `0.12.0`, the last
version the old single-crate `tagent` application published to crates.io.

### Changed
- **Version reset from `1.0.0` to `0.17.0`** (`Cargo.toml`), and a changelog for the
  library (this file) alongside `tagent-cli`'s and `tagent-gui`'s.

### Added
The public API at this version, as the baseline for later entries:
- **`providers::TranslationProvider`**: `translate_text`, `get_dictionary_entry`,
  `detect_language`, `name`; created with `providers::create_provider()`. Its
  `DictionaryEntry` / `PartOfSpeechEntry` / `Definition` data structures carry the
  optional `corrected_word` set when the provider spell-corrected the input.
  `GoogleTranslateProvider` (`providers::google`) is the reference implementation.
- **`providers::SpeechProvider`**: `split_for_speech`, `speak_chunk`, `name`; created
  with `providers::create_speech_provider()`. A separate provider axis from
  translation, so the backend that translates and the backend that speaks are
  independent choices. `GoogleSpeechProvider` (`providers::google`) is the reference
  implementation. Introduced in Stage 11 of the `tagent-gui` development plan, which
  moved `split_for_speech` / `speak_chunk` off `TranslationProvider` (a source-breaking
  change for any implementor of that trait).
- **`providers::resolve_source_language()`**: resolves a `"auto"` source language to a
  concrete code through a translation provider's `detect_language`, for callers that
  need one before speaking.
- **`languages::name_to_code()` / `languages::code_to_name()`**: language name ↔ BCP-47
  code mapping.
- **`error::Error`**: `thiserror`-based error type shared across the provider boundary
  (`Network`, `Api`, `NotFound`, `EmptyText`, `TextTooLong`, `Decode`,
  `UnknownProvider`). `UnknownProvider` displays `unknown provider: {name}` since it
  covers speech providers as well as translation ones.
