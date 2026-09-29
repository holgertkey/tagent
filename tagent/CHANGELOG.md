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

## [0.19.0] - 2026-09-26

### Changed
- **Shared HTTP transport with retries and status mapping; the Google providers use it.**
  Every call has a time budget (Google: 10 seconds, as before) that now covers one retry:
  a connection failure or an HTTP 502/503/504 is retried once after a short pause, if
  there is time left. Timeouts, HTTP 500 and other 4xx are never retried, and neither is a
  429 from Google (insisting risks a block). An HTTP 429 now surfaces as
  `Error::RateLimited` instead of `Error::Api`, and error messages read
  `HTTP 503 Service Unavailable` (plus a short excerpt of a non-HTML error body) instead of
  `HTTP error: 503 Service Unavailable` / `Google TTS API returned status: …`. Network
  error messages no longer contain the request URL, which carried the text being
  translated. The generic options `timeout_secs` and `max_retries` (`0` disables
  retries) change the budget and retry count of any HTTP provider; the Google providers
  declare them and gain `with_options` constructors (`new()` is unchanged).
- **Breaking: `Error` is now `#[non_exhaustive]`.** A `match` on it outside this crate needs a
  wildcard arm. From now on, a new variant is a compatible change rather than a breaking one.

### Added
- **`languages::language_code` and `languages::language_for_locales`**, shared by both
  applications. `language_code` gives the table's code for a listed code or name in any
  case (`"Russian"`, `"RU"` → `"ru"`; `"Auto"` → `"auto"`) and `None` for anything
  unlisted, so a caller can tell a known language from a code it passes through.
  `language_for_locales` gives the code of the first locale in a preference list
  (`"ru-UA"`, `"de_DE.UTF-8"`, as `sys-locale` returns them) whose language is listed;
  the fallback is the caller's. Pure functions: the crate reads no environment and gains
  no dependency.
- **Cargo features per provider**: `google` and `deepl`, both in `default` (neither
  brings dependencies of its own). A library user can leave one out with
  `default-features = false`; a kind that is compiled out is
  missing from `TRANSLATION_PROVIDERS` etc. and the registry, and the factories return
  `Error::UnknownProvider` for it (also as a profile's `type`). With
  `default-features = false` and no provider feature the crate still builds (traits,
  options, profiles, registry types).
- **DeepL translation provider** (default feature `deepl`; `providers::deepl::DeepLTranslateProvider`,
  name `"deepl"`, display name `"DeepL"`), over DeepL's official API. It needs an `api_key`
  (a Free key, ending in `:fx`, uses `api-free.deepl.com`, any other key `api.deepl.com`;
  the `endpoint` option overrides the base URL), so it is built through
  `create_provider_with` / a profile, e.g. `[provider_options.deepl]` with `api_key`, or
  the `TAGENT_DEEPL_API_KEY` environment variable; `create_provider("deepl")` returns
  `Error::InvalidOptions`. Language codes are mapped at the edge (`en-US` → source `EN`,
  `pt-BR` → target `PT-BR`, `zh-TW` → `ZH-HANT`; `"auto"` lets DeepL detect the source).
  HTTP 403 is `Error::Auth`, 456 `Error::QuotaExceeded`, and both 429 and DeepL's 529 are
  `Error::RateLimited`, retried when `Retry-After` is at most 2 seconds; the transport
  defaults are a 10-second budget and two retries. `detect_language` works by translating
  the first 100 characters into English (DeepL has no detection endpoint), so it bills
  those characters. `TRANSLATION_PROVIDERS` and `translation_providers()` list `"deepl"` after
  `"google"`.
- **`languages::LANGUAGES`**: the public table of every language `name_to_code` /
  `code_to_name` know (15, as `Language { code, name }`, in display order; `"auto"` is
  accepted by both functions but not listed), so an application can build its language
  pickers from it. Both functions now look up this table instead of two separate `match`
  blocks, with unchanged results.
- **`Error` variants for keyed and paid services**, groundwork for providers beyond Google
  (no built-in provider returns them yet): `Auth` (credentials missing, invalid or expired;
  HTTP 401/403), `RateLimited { retry_after }` (HTTP 429, with the service's `Retry-After`
  if it sent one), `QuotaExceeded` (quota or character allowance used up), `Unsupported`
  (an operation or language pair the provider can't handle) and `InvalidOptions` (a
  missing or invalid provider option). The error table in the `providers` module docs
  lists them. DeepL (below) returns all of them except `Unsupported`.
- **Provider options and profiles**: `create_provider_with`, `create_dictionary_provider_with`
  and `create_speech_provider_with` take a `ProviderOptions` (a case-insensitive string map
  for API keys, endpoints, models, ...) next to the name. The name is a *profile* name: the
  reserved option `type` picks the provider kind and defaults to the name, so several
  configured instances of one kind can coexist (`"work"` with `type = google` shows up as
  `"Google Translate (work)"`). The name-only factories are now wrappers around these, with
  empty options, and behave exactly as before. `ProviderOptions::with_env_overrides` and
  `env_var_name` give every application the same `TAGENT_<PROFILE>_<KEY>` environment
  variables (e.g. `TAGENT_DEEPL_API_KEY`); `Debug` output of `ProviderOptions` redacts
  secret-looking values. The Google providers take no options yet.
- **Provider registry**: `translation_providers()`, `dictionary_providers()` and
  `speech_providers()` describe each axis's built-in providers as `ProviderDescriptor`s:
  canonical name, display name, the options it accepts (`OptionSpec`: key, required,
  secret, description) and its `TransportDefaults` (timeout, retries, whether a rate-limit
  answer is retried), so an application can build pickers and settings forms without
  hardcoding them. Same names and order as `TRANSLATION_PROVIDERS` etc., which stay.
  `ProviderOptions::with_env_overrides` now also looks up every option the profile's
  provider kind declares. For applications: `is_secret_option(kind, key)` says whether a
  value should be masked, and `ProviderOptions::with_env_overrides_using` takes the
  environment as a lookup function (for testing how config and environment combine).
- **`ProviderProfiles`**: an application's configured provider profiles (profile → key →
  value, case-insensitive, stored lowercase). `options(profile)` gives the
  `ProviderOptions` for a `*_with` factory with environment overrides applied,
  `profiles_of_kinds(kinds)` lists the profiles a picker for one axis can offer, `Debug`
  masks secret values, and with serde it is a plain `{"<profile>": {"<key>": "<value>"}}`
  object. `insert`/`remove` edit it (a profile left empty is dropped). Both applications
  keep their profiles in it.
- **`TranslationProvider::capabilities()`** returns a `TranslationCapabilities`
  (`detects_language`, `max_text_len` in characters, `languages`), so an application can
  adapt up front, e.g. offer `"auto"` only when the provider detects languages. It has a
  default implementation that claims nothing, so existing implementors keep compiling
  unchanged. `GoogleTranslateProvider` reports language detection. A provider without
  detection now documents `Error::Unsupported` from `detect_language`
  (`resolve_source_language` already falls back to `"en"` on any error).

## [0.18.3] - 2026-09-25

### Added
- **`article` module: display layout of a `DictionaryEntry`**, moved here from the two
  applications, which had identical copies. `article_lines` lays an entry out as lines of
  role-tagged spans (`Role::Header`, `PartOfSpeech`, `Plain`, `Synonym`), `to_plain` renders
  them as the plain text both apps have always shown, and `render_with` lets an app wrap each
  span in its own highlighting (ANSI colors in `tagent-cli`, rich-text markup in `tagent-gui`).
  Also `primary_line` and `part_of_speech_label` (the part-of-speech localization table).
  `Role`, `Span` and `Line` are `#[non_exhaustive]`.

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
