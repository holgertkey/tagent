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

## [0.18.2] - 2026-09-22

### Fixed
- **`GoogleTranslateProvider`/`GoogleDictionaryProvider`: stripped stray U+200B (zero-width
  space) characters from translations, definitions, synonyms, part-of-speech labels and
  corrected words.** The unofficial endpoint occasionally embeds these around individual
  words it considers ambiguous -- apparently a leftover of the Google Translate web page's
  per-word "show alternate translations" click targets. Invisible in most renderers, but
  visible as literal escape notation in editors with a "reveal whitespace" mode, which is how
  this was caught in practice (translating "You've done such an admirable job,
  congratulations!" into Russian produced two U+200B characters directly before
  "замечательную"). New `strip_invisible_markers` helper in `providers::google`, applied at
  every plain-text extraction point in both providers.

## [0.18.1] - 2026-09-20

### Added
- **`TRANSLATION_PROVIDERS`, `DICTIONARY_PROVIDERS` and `SPEECH_PROVIDERS`**: the names
  each factory accepts (canonical lowercase spelling), so a picker or a message can list
  the choices without hardcoding them. A test checks that every listed name is accepted
  by its factory, so a list can't drift from the `match` it describes. The factories stay
  closed `match`es — this is a list of names, not a registration mechanism.

## [0.18.0] - 2026-09-20

Dictionary lookup becomes its own provider axis — `TranslationProvider` ×
`DictionaryProvider` × `SpeechProvider`, any combination — with no change to what the
built-in Google backend returns for a lookup, apart from the `DictionaryEntry::word`
fix below. Breaking, hence the minor bump.

### Added
- **`DictionaryProvider`** trait (`lookup(word, from, to)` + `name()`), its
  **`create_dictionary_provider(name)`** factory (case-insensitive, `"google"` only,
  `Error::UnknownProvider` otherwise) and **`google::GoogleDictionaryProvider`**, a
  separate provider with its own HTTP client: selecting it never instantiates anything
  translate-related. The trait documentation now spells out the contract every backend
  inherits (bilingual field semantics, lowercase-English part-of-speech labels, when to
  return `Ok(None)`, `"auto"` sources, `corrected_word`), and the `providers` module
  docs gained a "Writing a dictionary provider" section with a compiled, offline
  example.
- **Constructors** `Definition::new`, `PartOfSpeechEntry::new`, `DictionaryEntry::new`
  and `DictionaryEntry::with_corrected_word`.
- **`examples/`**: `translate`, `dictionary`, `speak` (built-in Google providers, need
  network) and `custom_provider` (all three provider traits on toy backends, fully
  offline). Built by `cargo test`, so they can't drift from the API.
- **Guide-level API documentation.** The crate docs now cover quick-start snippets and
  the shared concepts (language codes, the `"auto"` source language, errors). The
  `providers` module docs hold the error contract, guidance for writing a translation
  or a speech provider (with a compiled, offline example), and how factories relate to
  provider construction. The `google` module docs list the caveats of the unofficial
  endpoints, and `create_provider` / `resolve_source_language` gained examples.

### Changed
- **`DictionaryEntry`, `PartOfSpeechEntry` and `Definition` are `#[non_exhaustive]`**, so
  a richer backend can add fields (confidence, pronunciation, examples, …) later without
  breaking callers. Their fields stay `pub`; outside this crate a struct literal no
  longer compiles — use the new constructors.
- **`DictionaryEntry::word` is now the word exactly as the caller passed it to
  `lookup`** (also on the spell-suggestion retry path, where `corrected_word` holds the
  suggestion). It used to hold Google's *translation* of the input (`"жестокий"` for
  `violent`), contradicting its own documentation. The field docs for
  `PartOfSpeechEntry::part_of_speech` and `Definition::{text, synonyms}` now state what
  the bilingual data actually is (`text` is a translation into the target language,
  `synonyms` are source-language words).
- `Error::UnknownProvider` also documents `create_dictionary_provider`.

### Removed
- **`TranslationProvider::get_dictionary_entry`** (and its `GoogleTranslateProvider`
  implementation), with no compatibility shim: use
  `create_dictionary_provider(..)?.lookup(..)` /
  `GoogleDictionaryProvider::lookup`. `TranslationProvider` keeps `translate_text`,
  `detect_language` and `name`.

### Fixed
- **Documentation corrections.** `DictionaryEntry::corrected_word` is `Some` whenever
  the provider echoes the looked-up word, *including for correctly spelled input*, not
  only after a correction; the field docs said otherwise and now say to compare it with
  the input. The Google TTS request limit is 100 *bytes*, not characters (the docs and
  a constant's comment said characters). The "adding a provider" steps named a
  non-existent `tagent.conf` and now name `tagent-cli.conf` / `tagent-gui.json`.

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
