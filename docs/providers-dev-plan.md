# Providers Development Plan

Living document: the provider concept for the `tagent` library and the staged plan for
building it out. Update it as work lands, and add a new stage (see "Stage template" at the
end) for every new provider. This document *is* the current state of the plan; there is no
separate decision log.

Started 2026-09-26.

---

## Part I — Concept

### Goal

The `tagent` library should be as provider-agnostic as possible: any service that accepts
text and answers with a usable result should be able to become a provider, ideally without
touching either application (`tagent-cli`, `tagent-gui`).

"Text in, text out" is the right *spirit*, but the wrong *abstraction level*. It describes
the transport and says nothing about what the returned text means or how to parse it. So
universality is built at the level of **capabilities**, and "any text server" is served by
**generic adapters** that implement those capabilities, not by weakening the contracts.

### Why "text in, text out" alone is not enough

- **Translation** is nearly pure text → text, but providers still differ in language-code
  spelling (`zh-CN` vs `ZH`, `en` vs `EN-US`), per-request length limits, and whether they
  can auto-detect the source language.
- **Dictionary** is structured: parts of speech, definitions, synonyms, spelling
  correction. Google returns a positional JSON array, and its correction logic (a second
  request for the suggested word) can't be expressed as "take field X".
- **Speech** returns audio bytes, not text, and each backend has its own chunk-size limit.

### Architecture: three layers

```
┌─ Capability (public trait) ─ WHAT we do:   TranslationProvider / DictionaryProvider / SpeechProvider
├─ Adapter ─────────────────── HOW to build a request and parse a response for one concrete API
└─ Transport ───────────────── HTTP client, credentials, timeouts, retries, rate limits, status → Error
```

- **Capability traits** stay the public contract and stay separate. The three independent
  axes (translation × dictionary × speech, any combination) are the core and are kept.
  Merging them into one generic `Provider<Req, Resp>` would lose type safety and leave no
  place for semantic contracts such as "`Definition::synonyms` are words in `from`".
- **Adapters** come in two tiers:
  - **Native adapters**: Rust code for rich or quirky APIs (Google today; DeepL, etc.).
  - **Generic adapters**: one implementation covering many services through configuration:
    - a **declarative HTTP adapter** (URL/body template, headers, JSON path to the result),
      translation only at first;
    - an **OpenAI-compatible chat adapter** (`/chat/completions`), which literally is
      "text in, text out" and covers OpenAI, Ollama, LM Studio, OpenRouter, vLLM, and other
      compatible endpoints; it can serve dictionary lookups too, via structured JSON output.
- **Transport** is shared, so adapters don't each re-implement client setup, retries and
  status-code mapping.

### Principles

1. **Contracts over transport.** Every trait's doc comment is the contract all backends
   inherit (language codes, `Ok(None)` for a dictionary miss, error meanings).
2. **BCP-47 inside, provider codes at the edge.** Apps and the library talk BCP-47 plus
   `"auto"`; each adapter maps to its own spelling internally.
3. **The library never stores configuration or secrets.** Apps own their config files
   (`tagent-cli.conf`, `tagent-gui.json`) and pass options in; the library only consumes them.
4. **A broken provider never breaks translation.** A dictionary or speech provider that
   fails to build degrades to "disabled + warning" (already the rule; it applies to every
   new provider too).
5. **Additive API growth.** New factories, trait methods with default impls, and
   `#[non_exhaustive]` types, so adding a provider is never a breaking change. Breaking
   changes are batched into as few `tagent` minor bumps as possible.
6. **Secrets are handled carefully.** Keys go in headers, never in query strings (because
   `Error::Network(e.to_string())` can include the URL). They are never included in error
   messages and are redacted in `Debug` output.
7. **Pay only for what you use.** Heavier providers live behind Cargo features.

### Current state (verified 2026-09-26, `tagent` 0.18.3)

- Three independent traits in `tagent/src/providers/mod.rs`: `TranslationProvider`
  (`translate_text`, `detect_language`, `name`), `DictionaryProvider` (`lookup`, `name`),
  `SpeechProvider` (`split_for_speech`, `speak_chunk`, `name`).
- Factories take a name only: `create_provider(name)`, `create_dictionary_provider(name)`,
  `create_speech_provider(name)`, each a `match` on the lowercased name. There is no way to
  pass an API key, endpoint, model or timeout.
- Name lists `TRANSLATION_PROVIDERS` / `DICTIONARY_PROVIDERS` / `SPEECH_PROVIDERS`
  (`["google"]` each), kept in step with the factories by a unit test. `tagent-gui`'s
  Settings dropdowns are built from them.
- One backend family, `tagent/src/providers/google.rs`: `GoogleTranslateProvider`,
  `GoogleDictionaryProvider`, `GoogleSpeechProvider`. Each builds its own `reqwest` 0.11
  `Client` with a 10 s timeout and a hardcoded browser `User-Agent`. There is no shared
  transport, no retries, and no status-code classification.
- `tagent::error::Error` has `Network`, `Api`, `NotFound`, `EmptyText`, `TextTooLong`,
  `Decode`, `UnknownProvider`. It is **not** `#[non_exhaustive]`, so adding a variant is
  breaking. `tagent-cli/src/config.rs:214` (`provider_error_message`) already matches with a
  `_` arm, so the app side is safe.
- `DictionaryEntry` / `PartOfSpeechEntry` / `Definition` are already `#[non_exhaustive]`.
- Factory call sites in the apps (the scope of any factory change):
  - `tagent-cli/src/translator.rs:104` (translation), `:115` (dictionary)
  - `tagent-cli/src/speech.rs:113` (translation, for `"auto"` resolution), `:210` (speech)
  - `tagent-cli/src/platform/linux/keyboard.rs:599`, `tagent-cli/src/platform/windows/keyboard.rs:838` (speech)
  - `tagent-gui/src/main.rs:1310` (speech), `:1318` and `:1406` (translation), `:1413` (dictionary)
- Examples: `tagent/examples/{custom_provider,dictionary,speak,translate}.rs`.
  `custom_provider.rs` is an **external implementor**, so any new required trait method is a
  breaking change.

---

## Part II — Development plan

### Conventions for every stage

- **Semver:** `tagent` is pre-1.0. A breaking change bumps the minor (0.18 → 0.19); an
  additive change or fix bumps the patch. **One `tagent` version per release cycle**
  (Q4, recorded in CLAUDE.md "Version Management"): the first change after a release
  picks the version, and every later change until the next release goes into that same
  version and changelog section, escalating patch → minor only if a breaking change
  arrives. The per-stage "Semver" lines below say which *kind* of change a stage is; the
  actual number follows this rule. `tagent-cli` / `tagent-gui` get a `+BUILD` bump
  when their code changes. If an app starts using a newer `tagent` API, raise the `version`
  in its `tagent = { path = ..., version = ... }` dependency.
- **Changelogs:** add an entry to each affected crate's own `CHANGELOG.md`
  (`tagent/`, `tagent-cli/`, `tagent-gui/`).
- **Docs:** `cargo doc -p tagent` has to stay free of `missing_docs` and broken-link
  warnings. Update the "Adding a New … Provider" sections in `CLAUDE.md` and
  `docs/ARCHITECTURE.md` when the procedure changes.
- **Tests:** unit tests for every pure function (request building, response parsing, code
  mapping). HTTP adapters are tested against a **local mock HTTP server** (dev-dependency,
  e.g. `wiremock` — pick after reading its docs), so CI never needs keys or network. Live
  tests against real services are `#[ignore]` and run only when an env var such as
  `TAGENT_LIVE_TESTS=1` plus the needed key are set.
- **Third-party APIs:** endpoint URLs, auth header formats, request/response shapes and
  limits are taken from the **official documentation**, read at the start of the stage,
  never from memory. Link the docs in the stage notes.

Stage 0 is a release-process prerequisite. Stages A–F are the **foundation** (library
infrastructure, done once); the former Stage G (Cargo features per provider) moved into
P1, where the second provider makes it useful (decided 2026-09-26). Stages P1, P2, … are **provider stages**, appended as
providers are added. They are numbered separately so a new provider never renumbers the
foundation.

---

### Stage 0 — Release notes: collect every unpublished `tagent` section (prerequisite)

**Status:** done (2026-09-26, no crate version change)
**Why:** `.github/scripts/release-notes.sh` includes only the changelog sections whose
version equals the crate's *current* version. That works for the apps, since within a cycle
only their `+BUILD` changes. It breaks for `tagent`, whose plain version can move several
times between releases. As of 2026-09-26, the last release (`v0.16.0`, 2026-09-20)
shipped `tagent` **0.18.1**, and 0.18.2 and 0.18.3 exist unpublished. At the next release,
the 0.18.2 entries would be missing from the GitHub Release notes, and if `tagent` is
0.19.0 by then (Stage A), both 0.18.2 and 0.18.3 would be missing. The "one version per
cycle" rule (Q4) prevents this going forward; this stage fixes the existing case and acts
as a safety net.
**Scope:** in `release-notes.sh`, for `tagent`, include every section whose version is
**greater than** the `tagent` version in the latest `v*` tag
(`git show <tag>:tagent/Cargo.toml`) and **≤** the current one, merged under one
heading per kind of change, as the script already does. Apps keep the current behavior.
Handle "no previous tag" (take all sections) and a shallow CI clone (the release workflow
may need `fetch-depth: 0` or an explicit tag fetch; check `.github/workflows/release.yml`).
It still fails when the crate has no entries.
**Tests:** this is a bug fix, so it needs a regression test: a small script-level test
that runs `release-notes.sh` against fixture changelogs and Cargo.toml files (sections
0.18.1 / 0.18.2 / 0.18.3 / 0.19.0 with the last tag at 0.18.1 → output contains the
0.18.2–0.19.0 entries but not 0.18.1's), wired into `ci.yml`.
**Semver:** no crate code changes → no version bumps; mention it in the next release's notes
only if useful.
**Done when:** a dry run on the real repo prints both 0.18.2 and 0.18.3 entries for `tagent`,
and the test runs in CI.
**Notes after landing:**
- The previous release is the highest `v*` tag by version (`--sort=-v:refname`, not by
  ancestry, so it works from `dev`), **skipping tags that point at HEAD**: during the release
  run, the pushed tag itself is the latest one and would otherwise make the range empty.
- A tag that predates the `tagent` crate (no `tagent/Cargo.toml`, e.g. `v0.9.0`) counts as
  "no previous release" → every section up to the current version.
- If `tagent`'s version didn't change since the previous release, the script falls back to the
  old behavior (sections equal to the current version) rather than failing on an empty range;
  `publish-crates.sh` skips the already-published crate anyway.
- The "no entries" check stays tied to the **current** version: collecting 0.18.2/0.18.3 must
  not hide a missing 0.19.0 section.
- A shallow clone is an error; `release.yml`'s `verify` and `release` jobs now check out with
  `fetch-depth: 0`.
- Test: `.github/scripts/test-release-notes.sh` (range with a tag at HEAD, no tag, tag before
  the crate, unchanged version, numeric `0.10.0 > 0.9.0` ordering, missing current section,
  shallow clone), run on Linux in `ci.yml`. It fails against the old script. On the real repo
  (last tag `v0.16.0`, `tagent` 0.18.1) the notes now carry both the 0.18.2 and 0.18.3 entries.

---

### Stage A — Error model for real-world APIs

**Status:** done (2026-09-26, tagent 0.19.0)
**Goal:** give keyed and paid services meaningful error variants, and make `Error` open for
future growth, all in **one** breaking bump.

**Changes** (`tagent/src/error.rs`, `tagent/src/providers/mod.rs` module docs table):
- Mark `Error` as `#[non_exhaustive]`.
- Add variants (final names decided at implementation time):
  - `Auth(String)`: missing, invalid or expired credentials (HTTP 401/403).
  - `RateLimited { retry_after: Option<Duration> }`: HTTP 429.
  - `QuotaExceeded(String)`: the account's quota or character limit is used up (e.g.
    DeepL's 456).
  - `Unsupported(String)`: the provider doesn't support this operation or language pair
    (e.g. `detect_language` on a backend without detection).
  - `InvalidOptions(String)`: a required option is missing, or an option value is invalid.
    Returned by the `*_with` factories in Stage B.
- Update the "Errors" contract table in the `providers` module docs.

**Apps:** `tagent-cli` `provider_error_message` already has a `_` arm, so no functional
change. Optionally, it could add friendly texts for `Auth` / `RateLimited`. Check `tagent-gui` for
exhaustive matches on `Error` (none found in the 2026-09-26 grep).

**Semver:** breaking. `tagent` → **0.19.0** (from 0.18.3; the cycle's existing 0.18.3
section is kept as is, since Stage 0 makes the release notes pick it up). Everything
else that lands before the next release (Stage B onward) goes into this same 0.19.0 (Q4).
Apps: bump the dependency version plus `+BUILD`.
**Commits:** A is its own commit, separate from B (small, mechanical, easy to review and bisect).
**Tests:** `Display` output of the new variants; the existing `From<reqwest::Error>` still maps timeouts.
**Done when:** `cargo test`, `cargo clippy -D warnings` and `cargo doc -p tagent` are clean,
and all three changelogs have entries.
**Notes after landing:**
- Variant names shipped as sketched: `Auth(String)`, `RateLimited { retry_after: Option<Duration> }`,
  `QuotaExceeded(String)`, `Unsupported(String)`, `InvalidOptions(String)`. `RateLimited`'s
  message appends `" (retry after N s)"` when a wait is known, rounded **up** so it never
  suggests retrying too early.
- No built-in provider returns the new variants yet; the transport (Stage E) and the `*_with`
  factories (Stage B) will.
- Apps: no code change. `tagent-cli`'s `provider_error_message` already had a `_` arm, and
  `tagent-gui` has no `match` on `Error`. The optional friendly texts for `Auth` /
  `RateLimited` were left for Stage F, when a keyed provider can actually produce them.
  Dependency `version` raised to `0.19.0` in both apps (required: `^0.18.3` doesn't match
  0.19.0); `tagent-cli` 0.16.0+007, `tagent-gui` 0.14.0+020.
- Tests: `Display` of every new variant; a `reqwest` timeout (a local listener that never
  answers) still maps to `Network("request timed out")`; a doctest shows matching with a
  wildcard arm.

---

### Stage B — Provider options and `*_with` factories

**Status:** done (2026-09-26, tagent 0.19.0)
**Goal:** allow passing API keys, endpoints, models and timeouts into providers.

**API sketch** (`tagent/src/providers/options.rs`, re-exported from `providers`):
```rust
/// Options for constructing a provider (credentials, endpoint, model, ...).
#[non_exhaustive]
#[derive(Clone, Default)]
pub struct ProviderOptions {
    values: BTreeMap<String, String>, // keys are lowercase: "api_key", "endpoint", "model", "timeout_secs", ...
}

impl ProviderOptions {
    pub fn new() -> Self;
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self;
    pub fn get(&self, key: &str) -> Option<&str>;
    pub fn require(&self, key: &str) -> Result<&str, Error>; // Error::InvalidOptions
    /// Overrides values from `TAGENT_<PROVIDER>_<KEY>` environment variables (see Q1).
    pub fn with_env_overrides(self, provider: &str) -> Self;
}

/// `("deepl", "api_key")` → `"TAGENT_DEEPL_API_KEY"`; `("work-deepl", "api_key")`
/// → `"TAGENT_WORK_DEEPL_API_KEY"` (uppercase, every non-alphanumeric char → `_`).
/// `provider` is the *profile name* (Q3), not its type.
pub fn env_var_name(provider: &str, key: &str) -> String;

impl fmt::Debug for ProviderOptions { /* values of secret-looking keys are redacted */ }

pub fn create_provider_with(name: &str, options: &ProviderOptions)
    -> Result<Box<dyn TranslationProvider>, Error>;
pub fn create_dictionary_provider_with(name: &str, options: &ProviderOptions)
    -> Result<Box<dyn DictionaryProvider>, Error>;
pub fn create_speech_provider_with(name: &str, options: &ProviderOptions)
    -> Result<Box<dyn SpeechProvider>, Error>;
```
- The existing name-only factories become thin wrappers:
  `create_provider(name) == create_provider_with(name, &ProviderOptions::default())`.
- A string map (not a typed struct per provider) keeps the app-side config generic: an app
  can forward any `key = value` pairs from its config file without knowing the provider.
- Google ignores options for now (it could honor `timeout_secs` later).
- `env_var_name` / `with_env_overrides` live in the library so both apps share **one**
  naming rule (Q1). Reading environment variables is not app configuration, so this doesn't
  break the principle that the library never stores config. `with_env_overrides` only
  overrides keys that the provider's descriptor declares (Stage C), or, if C hasn't landed
  yet, keys already present plus `api_key`. Decide which at implementation time.
- **Profiles (Q3 decision, 2026-09-26).** `name` in the `*_with` factories is a **profile
  name**, and `type` is a **reserved option key** handled by the library:
  - The provider kind is `options.get("type")`, falling back to `name`. So
    `create_provider_with("deepl", &opts)` without `type` is the built-in DeepL, and
    `create_provider_with("ollama", &opts_with_type_openai_compat)` is an OpenAI-compatible
    instance named `ollama`. The apps need no profile-resolution logic: they pass the
    profile name and the contents of its config section.
  - Profile names are case-insensitive and must match `[a-z0-9_-]+` (**no colon**, so a
    `[Provider:<name>]` section parses unambiguously); otherwise → `InvalidOptions`.
  - Built-in kind names (`google`, `deepl`, `openai-compat`, `http`, …) are reserved: a
    profile named after one must have no `type`, or a `type` equal to its name.
    `name = "google", type = "deepl"` → `InvalidOptions`.
  - Unknown `type` → `UnknownProvider(<type>)`.
  - A kind that doesn't implement the requested axis (e.g. an `http` profile passed to
    `create_speech_provider_with`) → `UnknownProvider` as well. The existing "a broken
    dictionary/speech provider only disables that axis" rule applies unchanged.
  - The profile name goes into the provider's display name, e.g. `"OpenAI-compatible (ollama)"`,
    and into the env var names (`TAGENT_OLLAMA_API_KEY`).
  - `type` itself is never forwarded to the adapter as an ordinary option.

**Semver:** additive. It goes into the same unpublished 0.19.0 as Stage A, in a separate
commit, with no 0.19.1 (Q4).
**Apps:** unchanged in this stage (they keep calling the name-only factories); the wiring
happens in Stage F.
**Tests:** `require` on a missing key → `InvalidOptions`; `Debug` redacts `api_key`; the
wrapper factories behave exactly as before; `env_var_name` normalization (`-`, `_`,
mixed case); profile resolution (`type` absent → name is the kind; `type` present → it
wins; a reserved name with a foreign `type` → `InvalidOptions`; a name with `:` or other
invalid characters → `InvalidOptions`; unknown `type` → `UnknownProvider`; a kind lacking
the axis → `UnknownProvider`); `with_env_overrides` overrides only when the variable is set (the test uses a
unique variable name to avoid races with parallel tests); doc examples on each new public
item.
**Done when:** new public items are documented with examples and `cargo doc` is clean.
**Notes after landing:**
- Files: `tagent/src/providers/options.rs` (`ProviderOptions`, `env_var_name`, re-exported
  from `providers`) and the private `tagent/src/providers/profile.rs` (profile resolution
  and the display-name wrapper). The three `*_with` factories are in `providers/mod.rs`;
  the name-only factories call them with empty options.
- **Profile-name rules apply only when `type` is set.** Without `type` the name *is* the
  kind, so an unknown or malformed name stays `UnknownProvider(<name as given>)`, exactly
  as before (`tagent-cli`'s `provider_error_message` relies on that to list the supported
  values). Only a real profile (`type` present) gets `InvalidOptions` for a bad name, an
  empty `type`, or a built-in name with a foreign `type`. An unknown `type`, and a kind
  lacking the axis, are `UnknownProvider(<type>)`. The axis's name list
  (`TRANSLATION_PROVIDERS`, ...) is the gate; reserved names are the union of all three.
- Display name: a profile whose name differs from its kind is wrapped in
  `profile::Profiled`, which reports `"<provider name> (<profile>)"` and forwards every
  trait method. A comment in each trait says new methods (default ones included, e.g.
  Stage D's `capabilities()`) must be forwarded there too.
- `with_env_overrides` (Stage C hadn't landed): looks up the keys already present plus
  `api_key`, **never `type`** (an env var must not switch the provider kind). An unset,
  empty or non-Unicode variable is ignored, so `TAGENT_X_API_KEY=` doesn't blank a key
  from the file. Stage C can widen the key set to each descriptor's declared options.
- Unit tests never call `std::env::set_var` (it races with `getenv` in other test threads,
  e.g. `reqwest` reading proxy variables): the logic sits in a `pub(crate)`
  `with_overrides_from(profile, lookup)` tested with a fake lookup; only the doctest,
  which runs as its own process, sets a real variable.
- `require` treats an empty or whitespace-only value as missing; its message names the
  key, never a value. `Debug` redacts values whose key contains `key`, `secret`, `token`,
  `password`, `auth` or `credential` (one function, `is_secret_key`, for Stage C to
  replace with `OptionSpec::secret`). Also added: `insert`, `remove`, `is_empty`, `iter`,
  and `FromIterator<(K, V)>` (what Stage F needs to turn a config section's map into
  options).
- `ProviderOptions` isn't `#[non_exhaustive]` as sketched: its only field is private, which
  already prevents construction outside the crate.
- Google ignores options, as planned. No app changes, so no app version bumps.

---

### Stage C — Provider registry and descriptors

**Status:** done (2026-09-26, tagent 0.19.0)
**Goal:** one source of truth per provider (name, display name, options it needs), usable
by apps to build settings UIs.

**API sketch:**
```rust
/// Static description of one provider.
#[non_exhaustive]
pub struct ProviderDescriptor {
    pub name: &'static str,              // canonical lowercase, as passed to the factories
    pub display_name: &'static str,
    pub options: &'static [OptionSpec],  // what the provider accepts
    pub transport: TransportDefaults,    // per-provider retry/timeout defaults (Q2)
}

/// Transport defaults a provider ships with; overridable by the generic
/// `max_retries` / `timeout_secs` options (Stage E).
#[non_exhaustive]
pub struct TransportDefaults {
    pub max_retries: u32,
    pub timeout: Duration,               // total budget for one call, retries included
    pub retry_on_rate_limit: bool,       // false for Google's free endpoint
}

#[non_exhaustive]
pub struct OptionSpec {
    pub key: &'static str,               // "api_key"
    pub required: bool,
    pub secret: bool,                    // UI should mask it; Debug redacts it
    pub description: &'static str,
}

pub fn translation_providers() -> &'static [ProviderDescriptor];
pub fn dictionary_providers() -> &'static [ProviderDescriptor];
pub fn speech_providers() -> &'static [ProviderDescriptor];
```
- **Keep** `TRANSLATION_PROVIDERS` / `DICTIONARY_PROVIDERS` / `SPEECH_PROVIDERS` (the
  `tagent-gui` dropdowns and the `tagent-cli` error messages read them). The unit test is
  extended to check that the descriptor lists and the const lists agree, so everything
  stays additive.
- Factories may dispatch via the registry instead of a `match`, as an internal refactor.
- The registry is per axis, so it also answers "does kind X support axis Y". Apps use that
  to decide which user profiles (Q3) to offer in each axis's picker. Built-in kind names in
  the registry are the reserved profile names from Stage B.

**Semver:** additive → patch.
**Tests:** every descriptor name is accepted by its factory; descriptor lists match the
consts; option keys are lowercase and unique per provider.
**Notes after landing:**
- `tagent/src/providers/registry.rs`, re-exported from `providers`. The descriptor structs
  are `#[non_exhaustive]` with public fields and derive `Debug, Clone, Copy, PartialEq, Eq`
  (everything in them is `&'static`, `bool` or `Duration`). Per axis, one descriptor per
  provider; the Google ones have display names `"Google Translate"` / `"Google Dictionary"`
  / `"Google TTS"`, tested to equal the built provider's `name()`.
- **Google's `TransportDefaults::max_retries` is `0`, not the planned `1`**: the descriptor
  is public API and describes what happens *today*, and there are no retries before the
  shared transport exists. Stage E flips it to `1` together with the retry (noted there).
  `timeout` (10 s) and `retry_on_rate_limit: false` are already true. The 10 s is now one
  `pub(crate) const GOOGLE_TIMEOUT` in `google.rs`, used by both the three client
  builders and the descriptors.
- The factories still gate on the const lists and dispatch with a `match` (the optional
  registry-dispatch refactor was skipped). Tests keep list, descriptors and factory in
  step: same names in the same order, every descriptor builds, display names match,
  option keys lowercase/unique/non-empty/never `type`, and **every `secret` option is
  caught by `ProviderOptions`' `is_secret_key`**, so `Debug` redacts declared secrets
  (`Debug` itself stays name-based, since it doesn't know the kind).
- **`with_env_overrides` deviates from "only declared keys"**: it looks up the present
  keys ∪ `api_key` ∪ the kind's declared keys (union over all three axes; kind = `type`,
  else the profile name; still never `type`; unknown kinds just declare nothing). This
  keeps Stage B's tested behavior and keeps generic keys such as Stage E's
  `timeout_secs` overridable without every descriptor declaring them. The `pub(crate)`
  helper takes the declared keys as a parameter, so this is tested with a fake set
  (Google declares none, so the real registry can't exercise it yet).
- The "adding a provider" procedure is now: `*_with` branch + const-list entry +
  descriptor; updated in the `providers` module docs, the trait's steps,
  `examples/custom_provider.rs` and CLAUDE.md.

---

### Stage D — Capability metadata

**Status:** done (2026-09-26, tagent 0.19.0)
**Goal:** let apps know in advance what a provider can do instead of discovering it through errors.

**API sketch:**
```rust
#[non_exhaustive]
#[derive(Clone, Debug, Default)]
pub struct TranslationCapabilities {
    pub detects_language: bool,
    pub max_text_len: Option<usize>,
    pub languages: Option<Vec<String>>, // None = unknown/any
}

trait TranslationProvider {
    // ...existing methods...
    /// Describes what this provider supports.
    fn capabilities(&self) -> TranslationCapabilities { TranslationCapabilities::default() }
}
```
- A **default impl** is mandatory, because external implementors (e.g.
  `examples/custom_provider.rs`) must keep compiling. Add similar structs for dictionary
  and speech only when a concrete need appears (YAGNI).
- A provider without detection returns `Error::Unsupported` from `detect_language`.
  `resolve_source_language` already falls back to `"en"` on any error.
- Google overrides `capabilities()` with its real values.

**Semver:** additive → patch.
**Tests:** the default impl returns defaults; Google's capabilities are non-default.
**Notes after landing:**
- Shipped as sketched: `TranslationCapabilities { detects_language, max_text_len,
  languages }`, `#[non_exhaustive]`, `Clone, Debug, Default, PartialEq, Eq`, defined in
  `providers/mod.rs` next to the trait. Outside the crate it's built from `default()` plus
  field assignment (shown in its doc example).
- **The default claims nothing**, including `detects_language: false`: a provider that
  doesn't override `capabilities()` makes no promises. `resolve_source_language` does **not**
  consult capabilities (it still just calls `detect_language` and falls back to `"en"`), so an
  older external provider that does detect languages loses nothing.
- `max_text_len` is in **characters** (Unicode scalar values), as translation APIs
  document their limits; note that `Error::TextTooLong` (TTS chunks) counts bytes.
- Google reports only `detects_language: true`; `max_text_len` and `languages` stay `None`,
  since the unofficial endpoint documents neither and the provider enforces none.
- `profile::Profiled` forwards `capabilities()`; a test checks a profiled Google provider
  reports the same (non-default) capabilities. `examples/custom_provider.rs` compiles
  unchanged, and a test provider without the method checks the default and the `"en"`
  fallback on `Error::Unsupported`.
- No dictionary/speech capability structs (YAGNI, as planned). No app changes.

---

### Stage E — Shared HTTP transport

**Status:** done (2026-09-26, tagent 0.19.0)
**Goal:** a single place for client setup, credentials, retries and status → `Error` mapping.

**Scope** (`tagent/src/providers/http.rs`, most of it `pub(crate)` at first):
- Client builder: timeout (default 10 s, overridable via the `timeout_secs` option), a
  `User-Agent` (per provider; Google keeps its browser UA), and default headers (auth).
- Status mapping: 401/403 → `Auth`, 429 → `RateLimited` (parse `Retry-After`), provider-
  specific quota codes → `QuotaExceeded`, other non-2xx → `Api` (body truncated, **never**
  containing request headers or keys).
- **Retries** (Q2 decision, 2026-09-26). Tagent is interactive: latency beats squeezing
  out a success, so an honest error is better than turning a 10 s wait into 30 s.
  - Retries live **only** in the transport. Neither the apps nor the adapters retry on
    their own.
  - What is retried:

    | Situation | Retry? |
    |---|---|
    | Connection failure (DNS, connect refused/reset; the request never reached the server) | yes |
    | HTTP 502 / 503 / 504 | yes |
    | HTTP 429 with `Retry-After` ≤ 2 s (and the provider's `retry_on_rate_limit` is true) | yes, after that delay |
    | HTTP 429 otherwise | **no** → `Error::RateLimited { retry_after }` |
    | Timeout | **no** (the whole budget was already spent waiting) |
    | HTTP 500 | no (usually a request problem; a retry gives the same answer) |
    | 401 / 403 / quota / other 4xx | never |

  - **Total time budget:** the whole call, retries included, fits into the provider's
    timeout. If there isn't enough time left for another attempt, return the last error.
  - Backoff: short, with jitter, roughly 300–600 ms.
  - Per-provider defaults come from `ProviderDescriptor::transport` (Stage C):

    | Provider | `max_retries` | Timeout | `retry_on_rate_limit` |
    |---|---|---|---|
    | Google (unofficial free endpoint) | 1 | 10 s | **false**: insisting risks captchas or IP blocks |
    | Keyed APIs (DeepL etc.) | 2 | 10 s | true |
    | OpenAI-compatible LLM | 1 | 60 s | true (every retry may cost money) |

  - **User overrides ship in this stage**: generic options `max_retries` (`0` disables
    retries) and `timeout_secs`, handled by the transport, so they work for every HTTP
    provider without adapter code. In config they sit next to other options (e.g.
    `[Provider:deepl]` → `max_retries = 0`). An invalid value → `Error::InvalidOptions`.
  - The library prints nothing about retries (no `eprintln!`). An observer hook can come
    later if diagnostics are needed.
  - Speech: the same policy per chunk. A retry means a short pause mid-utterance, which is
    better than a cut-off one. Cancellation (Esc) still works: dropping the future also
    drops a pending backoff sleep.
  - The backoff sleep needs an async timer: add `tokio` with the `time` feature to
    `tagent`'s regular dependencies. `reqwest` 0.11 already depends on `tokio`
    (verified in `Cargo.lock` 2026-09-26), so no new crate enters the tree.
- Migrate the three Google providers onto it. The **only** intended behavior change is the
  new single retry on connection failures and 502/503/504. Together with the retry, set Google's
  `TransportDefaults::max_retries` from `0` to `1` in `registry.rs` (Stage C shipped `0`
  because no retries existed yet) and mention it in the changelog. Existing tests must pass
  unchanged, and the changelog mentions the retry.
- Optional, separate decision: `reqwest` 0.11 → 0.12. It isn't exposed in the public API
  (`Error::Network` stores a `String`), so it's non-breaking; do it only if a need appears.

**Semver:** internal → patch.
**Tests** (mock server):
- The status-mapping table.
- Every row of the retry table: retried vs. not retried, and the attempt count.
- 429 with a short `Retry-After` is retried for keyed providers but never for Google.
- The total budget is never exceeded, including across retries.
- `max_retries = 0` disables retries; invalid `max_retries` / `timeout_secs` →
  `InvalidOptions`.
- Error messages don't contain the key (a regression test using a sentinel key).

**Notes after landing:**
- `tagent/src/providers/http.rs`, all `pub(crate)`: `HttpTransport::builder(TransportDefaults)`
  `.user_agent(..)`, `.secret_header(name, prefix, secret)` (sensitive header; the secret
  is redacted from error bodies), `.auth_statuses(..)` (default 401/403),
  `.quota_statuses(..)` (default none), `.options(&ProviderOptions)` (`max_retries`,
  `timeout_secs`), `.build()`; then `send(|client| request) -> Result<Vec<u8>, Error>`,
  which returns the body of the first 2xx response. `build` is called once per attempt.
- **`send` reads the body itself**, so body-read errors and the budget are handled in one
  place. The budget is a deadline; each attempt gets `RequestBuilder::timeout(remaining)`,
  and a retry happens only if `delay + min_attempt` (1 s) still fits. Backoff 300–600 ms,
  jitter from `RandomState` (no `rand` dependency). Tests shrink the timing.
- **A timeout is checked before a connect error** (a timed-out connect is both in reqwest).
- **Google keeps 401/403 as `Api`** (`auth_statuses(&[])`), deviating from "401/403 →
  `Auth`": it has no credentials, so "authentication failed" would mislead. Its 429 now
  becomes `RateLimited` (was `Api`), never retried.
- Message wording changed (no test depended on the old one): `Api("HTTP 503 Service
  Unavailable")`, plus `": <excerpt>"` of a non-HTML body (whitespace collapsed, ≤ 200
  characters cut on a char boundary, secrets redacted). `Retry-After` is parsed as whole
  seconds only; an HTTP date gives `None` (no retry).
- `From<reqwest::Error>` now strips the URL (`without_url`) for every non-timeout error,
  also outside the transport: Google's URLs carry the user's text in `q=`.
- Google: `GOOGLE_TRANSPORT.max_retries` 0 → 1 (as planned in Stage C). The three
  providers gained `with_options(&ProviderOptions) -> Result<Self, Error>` (`new()` =
  defaults, still infallible), and the factories pass the profile's options to it. The
  generic options are declared once (`TRANSPORT_OPTIONS` in `registry.rs`) and referenced
  by all three Google descriptors, so settings forms and env overrides see them.
- The dictionary's second request for a suggested spelling is a separate `send` with its
  own budget, so `lookup` can still take up to 2× the budget, as before; documented in the
  `google` module docs.
- `tokio` (feature `time` only) is now a regular dependency (already in the tree via
  `reqwest`); `wiremock` 0.6 is a dev-dependency (it pulls `hyper` 1 into the dev tree).
- Tests: every retry-table row with attempt counts (wiremock, or a closure counter for
  connect-refused and a never-answering listener), the budget across retries, the status
  mapping, the excerpt rules, option validation, a sentinel key in an auth header echoed
  back in a 401 body, and a sentinel in the URL of a refused connection; Google against a
  mock server (query, parsing, 503 retried once, 429 never, 403 → `Api`, `max_retries = 0`,
  TTS bytes). The live `#[ignore]` Google tests and a `tagent-cli` CLI translation pass.
- `reqwest` stays at 0.11 (no need for 0.12 appeared).

---

### Stage F — App-side options wiring

**Status:** done (2026-09-26) — F1 (config, env, `0600`, `/config`, call sites, pickers:
`tagent-cli` 0.16.0+008, `tagent-gui` 0.14.0+021) and F2 (`tagent-gui` Settings option
fields: 0.14.0+022)
**Goal:** let users configure keys, endpoints and models in each app's own config, and
pass them to the `*_with` factories.

Follows the Q1 decision (2026-09-26). Resolution order for every option:
**environment variable → app config file → provider default**.

**Critical constraint:** both apps rewrite their config file **entirely** from an
in-memory struct on save: `tagent-cli`'s `/save` → `ConfigManager::save_config()` →
`create_ini_content()`, and `tagent-gui`'s `GuiConfigManager::update()` → serde, with no
`#[serde(flatten)]`. Anything not modeled in the struct is **silently dropped** on the next
save. So provider options must be a real field of each app's config struct, never
hand-added sections that only the reader knows about.

- **`tagent-cli`** (`tagent-cli.conf`):
  - One section per **profile** (Q3), named `[Provider:<profile>]` (colon, not dot). The
    optional `type` key picks the provider kind and defaults to the profile name, e.g.
    ```ini
    [Provider]
    TranslateProvider = ollama

    [Dictionary]
    ; One profile can serve several axes.
    DictionaryProvider = ollama

    [Provider:ollama]
    type = openai-compat
    endpoint = http://localhost:11434/v1
    model = ...

    ; No type: the built-in deepl.
    [Provider:deepl]
    api_key = ...
    ```
    (The INI parser has no inline comments: text after a value is part of the value.)
    `TranslateProvider` / `DictionaryProvider` / `SpeechProvider` take a profile name (a
    bare built-in name keeps working, since that's a profile with no section). The
    hand-written parser (`config.rs` `parse_ini`) already accepts these section names:
    everything between `[` and `]` is the section name.
  - Keys inside these sections are the **library's option keys verbatim** (`api_key`,
    not `ApiKey`). The app forwards them without knowing the provider, so there's no
    mapping table to maintain. This is an intentional exception to the PascalCase style of
    the other sections.
  - `Config` gains `provider_options: BTreeMap<String, BTreeMap<String, String>>`.
    `parse_ini` fills it from every `Provider:*` section, and `create_ini_content()` writes
    it back, so `/save` keeps it.
  - `/config` (`display_config`) masks secret values (`••••` plus the last 4 characters)
    and shows where each value came from (`(from env TAGENT_DEEPL_API_KEY)`).
- **`tagent-gui`** (`tagent-gui.json`):
  - `GuiConfig` gains `#[serde(default)] provider_options: BTreeMap<String, BTreeMap<String, String>>`,
    e.g. `"provider_options": { "deepl": { "api_key": "..." } }`, so it survives `update()`.
    Keys of `provider_options` are profile names; the optional `"type"` entry works as in
    the INI file.
  - Settings > General pickers list the built-in providers plus the user profiles whose
    `type` supports that axis (via the registry, Stage C). For the selected entry, option
    fields are rendered from `ProviderDescriptor::options`, with `secret` fields as password
    inputs.
  - Creating and deleting profiles happens **by hand-editing the file only** in this
    stage. A Settings UI for that is in the Backlog.
  - Options (and their values) are never written to `tagent-gui.log`.
- **Environment variables** have the highest priority. Both apps call
  `ProviderOptions::with_env_overrides(provider)` (Stage B), so one variable, e.g.
  `TAGENT_DEEPL_API_KEY`, works for **both** apps. The shared `TAGENT_` prefix is a
  deliberate exception to the apps' independence: a key belongs to the service, not to an
  app's settings.
- **File permissions:** on Linux/macOS, every write of `tagent-cli.conf` /
  `tagent-gui.json` sets mode `0600`. That covers the initial default-config creation,
  `/save` and `update()`. On Windows, `%APPDATA%` is already per-user.
- Secrets never go into translation history or error messages (see Stage E).
- Both apps switch every call site listed under "Current state" to the `*_with` factories.
- Live-reload keeps working (options are re-read along with the provider name).

**Semver:** apps: `+BUILD` bump each; changelog entries in `tagent-cli` and `tagent-gui`.
**Tests:**
- Round trip: a config with `[Provider:deepl]` / `provider_options` survives `/save` and
  `update()` unchanged (the regression test for the "rewrite drops unknown data" trap).
- Env overrides the file value; the file value is used when the env var is unset.
- `/config` output and `Debug` never contain a sentinel secret in full.
- On Unix, the file mode is `0600` after each write path.
- A missing required key → the provider is disabled with a clear warning (translation
  provider: the same fatal/non-fatal behavior as today for a bad name).

**Split** (2026-09-26): F1 is everything except the `tagent-gui` Settings form that renders
option fields from `ProviderDescriptor::options` (F2), which is Slint UI work of its own.

**Notes after landing (F1):**
- Library additions it needed (in the same 0.19.0): `ProviderProfiles` (below),
  `ProviderOptions::with_env_overrides_using`
  (environment as a lookup function, so apps test env-vs-file precedence without
  `std::env::set_var`) and `providers::is_secret_option(kind, key)` (descriptor `secret` or
  the key-name heuristic), used by `/config`'s masking.
- **Both apps keep profiles in the library's new `ProviderProfiles`** (instead of a bare
  `BTreeMap` in each): one implementation of the case-insensitive lookup, the env
  overrides and `profiles_of_kinds`, and a `Debug` that masks secrets. The apps' config
  structs derive `Debug`, and a bare map would have printed raw keys (e.g. into
  `tagent-gui.log`); tests assert a sentinel key never appears in `{config:?}`. Names and
  keys are lowercased on load, so a hand-written `"Work"` / `[Provider:Work]` is saved back
  as `work`.
- `tagent-cli`: `Config::provider_options: ProviderProfiles` (the `[Provider:` prefix is
  matched case-insensitively, since an unrecognized section would be dropped by `/save`;
  an empty section is kept); `Config::create_*_provider()`
  wrap the `*_with` factories and format the error (`provider_error_message` gained a
  `profile` argument; `InvalidOptions` gets "check [Provider:<name>] … or the
  TAGENT_<NAME>_<KEY> environment variables"). `SpeechManager::resolve_speech_language`
  takes `&Config` instead of a provider name. Sections are appended after `[Speech]` with
  an explanatory comment block. The existing positional round-trip test covers the new field.
- `/config` lists every `[Provider:*]` section plus the three selected profiles; a secret is
  `••••` + last 4 characters, or only `••••` below 12 characters.
- **The INI parser has no inline comments** (`timeout_secs = 20 ; x` is an invalid value);
  documented in the README, and this plan's own example above is fixed accordingly (it also
  said `[Translation]` for `TranslateProvider`, which lives in `[Provider]`).
- `tagent-gui`: `GuiConfig::provider_choice(name)` → `ProviderChoice { name, options }`
  replaces the plain provider name in `TranslationRequest` and in `start_speaking`. The
  Settings save builds a whole new `GuiConfig`, so it carries `provider_options` over, read
  fresh from the manager after a reload (like `popup_position`), so a hand-edit made while
  the dialog is open isn't reverted. Pickers append `profiles_of_kinds(<axis list>)`.
- `0600`: both apps open the file with mode `0600` and also `set_permissions(0600)` on the
  handle before writing, which tightens an existing `0644` file; tested on every write path.
- Behavior on a bad profile is unchanged in kind: a bad translate profile is fatal at
  `tagent-cli` startup (as a bad name was); dictionary/speech degrade with a warning.
- Verified end to end with a throwaway `XDG_CONFIG_HOME`; `cargo check --target
  x86_64-pc-windows-gnu` for both apps is clean.

**Notes after landing (F2):**
- Settings > General "Provider options", below the pickers: for each distinct profile the
  three pickers select, a heading (`work (google)`) and one `LineEdit` per `OptionSpec` its
  kind declares on any axis (deduplicated by key), with the description underneath;
  `secret` → `InputType.password`, `required` → `*` and a "required" placeholder. A field an
  environment variable overrides right now names the variable. Profiles whose kind declares
  nothing (or is unknown) get no rows.
- Pure logic in `tagent-gui/src/provider_form.rs` (`fields`, `apply`, unit-tested); the
  dialog gets a `[ProviderOptionField]` model, rebuilt on every picker `selected`. That
  handler reads all three picker indices, which is correct because the std `ComboBox` sets
  `current-index` *before* firing `selected` (checked in Slint 1.17.1's
  `widgets/common/combobox-base.slint`, `select()`).
- Edits to a profile that is no longer selected when OK is pressed are still saved (they
  were typed in this dialog); kept deliberately, open for review.
- Edits go through `provider-option-edited(row, value)` into an `Edits` map keyed by
  `(profile, key)` (two-way binding to model rows isn't documented in Slint, so no `<=>`),
  so switching a picker back and forth keeps typed values; nothing is written before OK.
- On OK, only the edited keys are applied onto a fresh (reloaded) `provider_options`: an
  empty value removes the key (new library `ProviderProfiles::remove`, which also drops a
  profile left empty); untouched keys, `type` and other profiles are kept.
- "Reset to Defaults" doesn't reset provider options (they can hold API keys); the list
  just follows the pickers back to the defaults. Stated in the dialog's hint text.
- No value validation in the dialog: an invalid value surfaces as the provider's own
  `InvalidOptions` message at the next translation (a dictionary/speech profile degrades
  with a warning instead).
- Not verified live (Settings needs a click; GUI automation is off-limits without
  agreement): build, `cargo check --target x86_64-pc-windows-gnu`, clippy and unit tests
  only. Creating/deleting profiles in the UI stays in the Backlog.

---

### Stage F3 — `tagent-cli` config in TOML

**Status:** done (2026-09-27, tagent-cli 0.17.0)
**Goal:** replace `tagent-cli.conf` (INI, hand-written parser) with `tagent-cli.toml`
before P1, so DeepL ships with the new format and its profile docs are written once.
Only `tagent-cli` changes; `tagent-gui` keeps `tagent-gui.json` (its Settings dialog is the
main path, JSON suits a machine-written file, and the app is independent).

**Why:** the INI format's weak spots grew with Stage F:
- `/save` rewrites the whole file from a template (`create_ini_content`), so a user's
  comments are lost (it only ever changes the languages);
- no inline comments: `key = value ; note` makes `; note` part of the value;
- two key styles in one file (PascalCase keys, lowercase library keys in profiles);
- `parse_ini` and the per-key string parsing are a few hundred lines of our own code.

**Decisions (2026-09-27):**
1. **TOML, one file** `tagent-cli.toml` in the same directory. No separate providers file:
   secrets can already live in `TAGENT_<PROFILE>_<KEY>` env vars and the file is written
   with mode `0600`; a second file would mean two places to look, two mtimes to watch and
   cross-file references. An optional `include` can come later without a format change.
2. **No migration and no hint.** The old `tagent-cli.conf` is simply not read any more;
   on first start a default `tagent-cli.toml` is created. The CHANGELOG documents the
   rename and the key mapping. The legacy color-key fallbacks (`AutoPromptColor` →
   `SourcePromptColor`, `TranslationPromptColor` → `TargetPromptColor`) go too.
3. **Names follow the earlier decisions** (Q1, Q3, Stage F), mechanically converted:
   sections lowercase, keys snake_case, same sections as today. The profile-selecting
   keys are named as in `tagent-gui.json` (`translate_provider`, `dictionary_provider`,
   `speech_provider`), and profiles live under `provider_options` (as in
   `tagent-gui.json`), one table per profile. Library option keys stay verbatim; with
   snake_case everywhere they are no longer an exception.
4. **`tagent-cli` version bump:** the format change is breaking, so the minor goes up
   (`0.16.0+008` → `0.17.0`, release build per the build-mode rule; add the CHANGELOG
   section first, then bump).

**Layout** (the generated default file keeps explanatory comments, like today's):

```toml
[provider]
translate_provider = "google"   # a profile name; a built-in kind is a profile too

[translation]
source_language = "Auto"
target_language = "Russian"

[dictionary]
show_dictionary = true
spell_check = true
dictionary_provider = "google"

[interface]
show_terminal_on_translate = true
auto_hide_terminal_seconds = 3
copy_to_clipboard = false

[colors]
source_prompt_color = "None"
target_prompt_color = "BrightYellow"
dictionary_prompt_color = "BrightYellow"
part_of_speech_color = "..."
synonym_color = "..."
notice_color = "..."
error_color = "..."

[history]
save_translation_history = false
history_file = "..."

[hotkeys]
translate_hotkey = "Alt+A"

[speech]
enable_text_to_speech = true
speech_hotkey = "Alt+S"
enable_speech_hotkey = true
speech_provider = "google"

# Provider profiles: name = [a-z0-9_-]+, `type` defaults to the name.
[provider_options.deepl]
api_key = "...:fx"              # or the env var TAGENT_DEEPL_API_KEY

[provider_options.deepl-work]
type = "deepl"
api_key = "..."
```

(Default values are today's; the `...` are filled from the existing constants.)

**Key mapping** (old → new, for the CHANGELOG): every `[Section] PascalKey` becomes
`[section] snake_key` (`[Interface] AutoHideTerminalSeconds` →
`[interface] auto_hide_terminal_seconds`), and `[Provider:<name>]` becomes
`[provider_options.<name>]` with its keys unchanged.

**Scope / files:**
- `tagent-cli/Cargo.toml`: a TOML crate. Candidates: `toml` (serde) for reading plus
  `toml_edit` for `/save`, or `toml_edit` alone (with its serde support). Choose after
  reading their official docs.
- `tagent-cli/src/config.rs`:
  - Read: `serde` structs per section with `#[serde(default)]` (a missing key or section
    = today's default). Values are typed (bools, integer seconds). Unknown keys are
    ignored (kept on disk by `/save`, see below).
  - `provider_options` deserializes into `ProviderProfiles` (names and keys lowercased,
    as today; profile values are strings, so a non-string value is an error naming the
    key).
  - Errors: a syntax or type error names the file, line and key (from the TOML crate).
    At startup it is fatal with that message; on hot reload the previous config stays in
    effect and a warning is printed (today's reload behavior for a broken file is checked
    and kept if it differs).
  - `/save` edits the parsed document in place (`source_language`, `target_language`,
    the only values the app changes), preserving comments, order, unknown keys and
    `provider_options`. If the file is gone, it writes the default template with the
    current values. Still through `write_config_file` (mode `0600`).
  - The default template: generated with comments (including the list of providers
    `tagent` offers, as `generated_config_comments_list_the_providers_tagent_offers`
    checks today).
  - Removed: `parse_ini`, `create_ini_content`, `PROVIDER_SECTION_PREFIX` and the
    per-key string parsing. `provider_error_message` points an `InvalidOptions` at
    `[provider_options.<name>]`.
  - `/config` output shows the new key names and the file path.
- Paths: `get_default_config_path()` → `tagent-cli.toml` (Windows
  `%APPDATA%\tagent-cli\tagent-cli.toml`, Linux/macOS `~/.config/tagent-cli/tagent-cli.toml`).
  `--config` and any help text follow.

**Semver:** `tagent-cli` breaking config change → `0.17.0` (decision 4). `tagent` and
`tagent-gui` unchanged.
**Changelogs:** `tagent-cli/CHANGELOG.md` (`0.17.0`: Changed — breaking, file renamed and
reformatted, the key mapping, no migration; Removed — legacy color keys).
**Tests:**
- The generated template parses back into `Config::default()`.
- A full file with every key, and a minimal file (only one section) → defaults for the rest.
- A type error (`copy_to_clipboard = "yes"`) and a syntax error → an error naming the key/line.
- `/save` round trip: comments, key order, an unknown key and `provider_options` (with
  `type` and a secret) survive; only the languages change.
- `provider_options`: names/keys lowercased, a non-string value rejected, env overrides
  still win, `Debug` masks a sentinel secret.
- Mode `0600` on Unix for a new file and after `/save` (existing tests, adapted).
- Hot reload: a broken edit keeps the previous config.
**Docs:** `tagent-cli/README.md` (configuration section and example), CLAUDE.md
("Configuration System", "Configuration File Location", "Changing Hotkey Combination",
the provider sections that say `tagent-cli.conf`), `docs/ARCHITECTURE.md`; mentions of
`tagent-cli.conf` in `tagent-gui` docs ("doesn't read it") updated to the new name. The
provider examples in this plan (Stage F text stays as history; P1 uses the new form).
**Done when:** `cargo test`, clippy `-D warnings`; a fresh start creates a commented
`tagent-cli.toml`; hand-editing it hot-reloads; `/save` keeps comments; `/config` shows
masked profile secrets; a release build syncs the `0.17.0` version into the docs.


**Implementation notes (2026-09-27):**
- Crate: `toml_edit` 0.25 alone, `serde` feature (`de::from_str` to read, `DocumentMut`
  for `/save`); `toml` would have needed `toml_edit` for the in-place edit anyway.
- `ConfigFile` (one `#[serde(default)]` struct per section, defaults taken from
  `Config::default()` via a small macro) ↔ the flat `Config`, which the rest of the app
  keeps using unchanged. `Config` gained `PartialEq` for the tests.
- Errors carry line and column plus the offending line (so the key is visible), not a key
  path; that turned out to be enough. `main.rs` prints a startup error with `Display` and
  exits 1 (returning it from `main` printed the `Debug` form). Reload behavior changed
  for a broken file: the INI parser never failed, so there was nothing to keep; now the
  mtime is recorded before loading, so a broken edit warns once and the old config stays.
  The `.ok()` reload callers (interactive, CLI, speech) now print it (`reload_or_warn`).
- `render_config` = the commented template parsed, values set with `set_value`, profiles
  appended under an implicit `provider_options`, below the template's closing
  explanation. Two `toml_edit` traps found by experiment: `Table::insert` on an existing
  key drops the comment lines above it (they are the key's decor), so values are replaced
  through `get_mut`; indexing a missing section creates an inline table, so a real
  `Table` is inserted instead.
- `/save` = `with_languages`: only the two language values of the existing document
  (their inline comments kept); a missing file gets the full template.
- Checked live with `XDG_CONFIG_HOME` in a scratch dir: a fresh start creates a commented
  `tagent-cli.toml` (mode `0600`); `/config` masks a profile's `api_key`; a wrong value
  type exits with the line. Hot reload and `/save` are covered by unit tests only (the
  unified mode would grab the real global hotkeys).
- `/config` (0.17.0+001) prints the settings as `[section]` + `key = value` lines, generated
  from the same `ConfigFile` serialization as the file (so no field list of its own), and
  profiles as `[provider_options.<name>]` tables; a test parses the settings part back.
---


























## Provider stages

Each provider stage follows the template at the end. Planned order (it can be changed):

| Stage | Provider | Axis | Kind | Depends on |
|-------|----------|------|------|------------|
| P1 | DeepL, plus Cargo features per provider (former Stage G) | translation | native, keyed | A–F, F3 |
| P2 | OpenAI-compatible chat | translation | generic | A, B, E |
| P3 | OpenAI-compatible chat | dictionary | generic (structured JSON) | P2 |
| P4 | Declarative HTTP (reference config: LibreTranslate) | translation | generic | A, B, E |
| P5+ | *(backlog, below)* | | | |

### Stage P1 — DeepL (translation)

**Status:** in progress — part 1 implemented 2026-09-28 (tagent 0.19.0), live tests and
part 2 pending
**Why first:** a well-documented keyed API. It validates options, auth errors, quota
errors and language-code mapping end to end.
**Goal:** `DeepLTranslateProvider`, selectable in both apps through a `deepl` profile
(`[provider_options.deepl]` in `tagent-cli.toml`, `provider_options` in `tagent-gui.json`)
with an `api_key`, with no app code changes (the Stage F
wiring and the registry-driven Settings fields pick it up). Part 2 then puts every
provider behind its own Cargo feature.

**Official docs** (read 2026-09-27; re-check before landing):
- Translate text: <https://developers.deepl.com/api-reference/translate>
- Authentication: <https://developers.deepl.com/docs/getting-started/auth>
- Error handling: <https://developers.deepl.com/docs/best-practices/error-handling>
- Supported languages: <https://developers.deepl.com/docs/getting-started/supported-languages>
- OpenAPI spec (machine-readable, used to confirm the limits below):
  <https://github.com/DeepLcom/openapi/blob/main/openapi.yaml>

**API facts the adapter relies on:**
- `POST {base}/v2/translate`, JSON body `{"text": ["..."], "target_lang": "DE",
  "source_lang": "EN"}`; `source_lang` omitted → DeepL detects the language itself.
  Response: `{"translations": [{"detected_source_language": "EN", "text": "..."}]}`.
- Base URL: `https://api-free.deepl.com` for a Free key (suffix `:fx`),
  `https://api.deepl.com` otherwise.
- Auth header: `Authorization: DeepL-Auth-Key <key>` (header only, fits `secret_header`).
- Limit: the whole request body ≤ 128 KiB. The "1024 UTF-8 bytes" figure in the docs is a
  glossary-entry limit, not a text limit (confirmed in the OpenAPI spec).
- Statuses: 403 auth, 456 quota exhausted (terminal), 429 and 529 too many requests,
  500/503/504 temporary. The error body is JSON with `message` (or `error.message`).
- Codes are case-insensitive. Sources are base codes only (`EN`, `PT`, `ZH`); targets
  also take variants (`EN-GB`, `EN-US`, `PT-BR`, `PT-PT`, `ZH-HANS`, `ZH-HANT`, `ES-419`,
  ...), and a bare `EN`/`PT` target is still accepted. There is no detection endpoint.

**Decisions (2026-09-27):**
1. **`detect_language` via `/translate`** on a short prefix of the text (≈100
   characters, cut on a char boundary), reading `detected_source_language` and discarding
   the translated text. `target_lang` is a fixed `EN`; the live test checks that English
   input still reports `EN` (if DeepL rejects or misreports a same-language request, use
   `DE` instead). It bills those characters, but its only caller is the TTS `"auto"`
   resolution, and without it "DeepL + speech of `auto` text" would always speak as `en`.
   `capabilities().detects_language = true`.
2. **A bare target `en` / `pt` is passed as `EN` / `PT`**, no variant option: DeepL picks
   the variant, and a user who wants one sets `en-GB` etc. as the target language.
3. **HTTP 529 counts as rate limiting.** The transport gains a crate-private
   `rate_limit_statuses(&[u16])` (default `&[429]`, so Google is unchanged); DeepL sets
   `&[429, 529]`, so 529 becomes `RateLimited` and is retried with a short `Retry-After`
   like 429.
4. **The `deepl` feature is not in `default`** (part 2): library users opt in, the apps
   enable it in their own `Cargo.toml`.

Deliberate deviation from DeepL's advice: DeepL calls 500 retryable, but the Q2 policy
(no retry on 500) is kept for every provider.

**Scope / files (part 1, one commit):**
- New `tagent/src/providers/deepl.rs`, `pub struct DeepLTranslateProvider` with
  `with_options(&ProviderOptions) -> Result<Self, Error>` (no infallible `new()`: a key is
  required). Base URL from the key's `:fx` suffix, overridden by `endpoint` (which is also
  how mock-server tests point it at the mock; no `#[cfg(test)]` hook needed). Transport:
  `HttpTransport::builder(DEEPL_TRANSPORT).secret_header("Authorization",
  "DeepL-Auth-Key ", key)?.quota_statuses(&[456]).rate_limit_statuses(&[429, 529])
  .options(&options)?.build()?`; 403 → `Auth` via the default `auth_statuses`.
- Pure, unit-tested functions: `build_request_body(text, from, to)`,
  `parse_response(bytes) -> (text, detected)`, `to_deepl_source`, `to_deepl_target`,
  `from_deepl`.
- `tagent/src/providers/http.rs`: `rate_limit_statuses` (decision 3) plus its tests.
- `tagent/src/providers/mod.rs`: `pub mod deepl;`, `"deepl"` in `TRANSLATION_PROVIDERS`,
  a branch in `create_provider_with`; module docs mention the second provider.
- `tagent/src/providers/registry.rs`: `DEEPL_TRANSPORT` (2 retries, 10 s,
  `retry_on_rate_limit: true`, per Q2) and a descriptor with display name `"DeepL"` and a
  full option list of its own (`api_key`, `endpoint`, `timeout_secs`, `max_retries`; a
  `static` can't concatenate `TRANSPORT_OPTIONS`, so the two generic entries are repeated
  or shared as separate `const OptionSpec`s).
- `capabilities()`: `detects_language: true`, `max_text_len: None` (the limit is bytes of
  the whole request, not characters), `languages: None` (DeepL's list keeps growing; a
  hardcoded one would go stale).

**Options:**

| Key | Required | Secret | Default |
|---|---|---|---|
| `api_key` | yes (`InvalidOptions` if missing/empty) | yes | — (env `TAGENT_DEEPL_API_KEY` works through Stage B) |
| `endpoint` | no | no | by key: `https://api-free.deepl.com` (`:fx`) or `https://api.deepl.com`; the adapter appends `/v2/translate` |
| `timeout_secs`, `max_retries` | no | no | transport defaults (10 s, 2) |

**Language codes** (BCP-47 in, DeepL at the edge; case-insensitive):
- Source: the primary subtag, uppercased (`en-US` → `EN`, `zh-TW` → `ZH`); `"auto"` → no
  `source_lang`.
- Target: `zh`, `zh-CN`, `zh-Hans`, `zh-SG` → `ZH-HANS`; `zh-TW`, `zh-HK`, `zh-Hant` →
  `ZH-HANT`; other region tags uppercased as is (`pt-BR` → `PT-BR`, `en-GB` → `EN-GB`);
  a bare code uppercased (`en` → `EN`). An unsupported code is left to DeepL, whose 400
  surfaces as `Api` with its `message`.
- Back (`detect_language`): lowercased primary subtag (`EN` → `en`, `ZH` → `zh`).

**Semver:** additive; goes into the current cycle's `tagent` 0.19.0 (unpublished; the
latest `v*` tag carries 0.18.1), same changelog section, no bump. Part 1 is expected to
change no app code, so no `+BUILD` bumps; if an app needs a change, it gets one.
**Changelogs:** `tagent/CHANGELOG.md`, 0.19.0 section: the DeepL provider (with its
529 handling; `rate_limit_statuses` itself is crate-private and not mentioned). The apps get a short
"DeepL available via a profile" note only when they change (part 2 does change their
`Cargo.toml`).

**Tests:**
- Unit: request body (with/without `source_lang`), response parsing (incl. missing
  `translations`, empty list → `Decode`), every code-mapping row, key → base URL (`:fx`
  vs Pro vs `endpoint` override), missing `api_key` → `InvalidOptions`,
  `capabilities()`, the prefix cut for detection (multi-byte text).
- Mock server (`wiremock`): success, checking the `Authorization` header and the JSON
  body; `detect_language` round trip; 403 → `Auth`; 456 → `QuotaExceeded` (not retried);
  429 / 529 with short `Retry-After` retried, without → `RateLimited`; 503 retried; a
  sentinel key never appears in error messages; `max_retries = 0`.
- Factory: `create_provider_with("deepl", ..)`, a profile `work` with `type = deepl`
  (label `"DeepL (work)"`), registry/list/factory agreement (existing tests).
- Live, `#[ignore]`: `TAGENT_LIVE_TESTS=1` plus `TAGENT_DEEPL_API_KEY` (a Free key is
  available): translate `en → de`, `auto → ru`, `detect_language`.
- Manual: `tagent-cli` CLI mode with `translate_provider = "deepl"` and
  `[provider_options.deepl]` `api_key = "..."` in `tagent-cli.toml`.

**Docs:** `deepl` module rustdoc (options, code mapping, detection cost, Free/Pro),
`cargo doc -p tagent` clean; CLAUDE.md ("Translation Provider Architecture": a DeepL
bullet next to Google's) and `docs/ARCHITECTURE.md`; fill "Notes after landing" here.
**Done when:** `cargo test`, clippy `-D warnings`, `cargo doc -p tagent` clean; the live
tests pass with the Free key; a CLI translation through a `deepl` profile works;
`tagent-gui` Settings offers DeepL with `api_key` as a password field (checked by build
and unit tests; the dialog itself only if a live check is agreed).

**Notes after landing (part 1):**
- Shipped as planned: `providers/deepl.rs` (`DeepLTranslateProvider::with_options`, pure
  `base_url`/`build_request_body`/`parse_response`/`to_deepl_source`/`to_deepl_target`/
  `from_deepl`/`detection_prefix`), `DEEPL_TRANSPORT` and `DEEPL_OPTIONS` in the registry
  (the generic options became two `const OptionSpec`s, `TIMEOUT_SECS`/`MAX_RETRIES`,
  shared by both lists), `rate_limit_statuses` in the transport (the two hardcoded 429s —
  retry decision and `status_error` — now read it; `status_error` takes a `StatusMap`).
- Small additions beyond the plan: blank text → `Error::EmptyText` with no request (no
  billed call for nothing); `endpoint` must be an `http(s)` URL (`InvalidOptions`
  otherwise); the key is trimmed before the `:fx` check; `zh-MO` and `zh-Hant-TW` map to
  `ZH-HANT`; `_` is accepted as a subtag separator; `User-Agent: tagent/<version>`.
- Tests that built every listed provider with empty options (registry, factory list, the
  `TRANSLATION_PROVIDERS` doctest) now fill each descriptor's `required` options with a
  dummy value (`registry::required_options`, test-only); a new test checks that a
  provider with a required option refuses to build without it.
- Live tests are the first to use the `TAGENT_LIVE_TESTS=1` convention: `#[ignore]` plus an
  early return without it. Run: `TAGENT_LIVE_TESTS=1 TAGENT_DEEPL_API_KEY=...
  cargo test -p tagent deepl::tests::live -- --ignored --nocapture`.
- No app code changed. A `tagent-gui` test pinning "DeepL's `api_key` is a password
  field" is left for part 2, which changes the apps' `Cargo.toml` anyway.

#### P1 part 2 — Cargo features per provider (former Stage G)

Moved here from the foundation (2026-09-26): with a single provider there is nothing to
choose between, and DeepL is the first provider worth leaving out. Done after DeepL itself
works, in its own commit.

**Goal:** users of the library compile only the providers they need.
- Features named after the provider kinds: `google` (default), `deepl` (**not** default,
  decision 4), and later `openai-compat`, `http`. The registry (descriptors and
  `*_PROVIDERS` lists), the factory branches, the provider modules and their tests are all
  `cfg`-gated consistently; the shared transport and `ProviderOptions`/`ProviderProfiles`
  stay unconditional. Keeping `google` in `default` makes the change additive.
- A profile whose `type` names a kind that is compiled out gets `UnknownProvider`, the same
  as an unknown kind, and the reserved built-in names follow the enabled features.
- With `--no-default-features` every list is empty: the factories' `match` needs a
  fallback arm (`UnknownProvider`), and items used only by providers (e.g. `GOOGLE_*`
  constants, transport helpers) must not trigger dead-code warnings.
- `examples/*` that need Google get `required-features = ["google"]`.
- The apps enable what they ship: `tagent = { path = "../tagent", version = "0.19.0",
  features = ["deepl"] }` in `tagent-cli` and `tagent-gui` (Google comes from `default`),
  with `+BUILD` bumps and changelog entries ("DeepL translation provider available via a
  profile").
- CI (`ci.yml`): build/test with `--all-features`, plus `cargo check -p tagent
  --no-default-features`, `--no-default-features --features google` and
  `--no-default-features --features deepl`; the packaging dry run covers the app
  dependencies. Check `release.yml` for anything that builds `tagent` alone.
- Docs: the "Adding a New … Provider" sections in CLAUDE.md / ARCHITECTURE.md gain the
  feature step; the `tagent` README lists the features.

### Stage P2 — OpenAI-compatible chat (translation)

**Status:** planned
**Before starting:** read the official docs of the chat-completions API and at least one
compatible local server (e.g. Ollama's OpenAI-compatible endpoint) to confirm the common
subset.
**Scope:** `tagent/src/providers/openai_compat.rs`; options `endpoint` (required), `model`
(required), `api_key` (optional, secret: local servers need none), `temperature`
(default low), `system_prompt` (optional override). The default prompt instructs the model
to return only the translation. `detect_language` is also done via a prompt (returns a
BCP-47 code; validate the answer's shape).
**Risks:** extra text or quotes in the model output (strip and validate), latency, cost.
**Tests:** prompt building; output cleanup; mock-server round trip.

### Stage P3 — OpenAI-compatible chat (dictionary)

**Status:** planned
**Scope:** `OpenAiCompatDictionaryProvider` in the same module. It asks for a JSON object
matching the `DictionaryEntry` shape (structured output if the endpoint supports it,
otherwise JSON-in-prompt plus strict parsing). It must honor the `DictionaryProvider`
contract: definitions in `to`, synonyms in `from`, lowercase English part-of-speech
labels, and `Ok(None)` for a miss (never `Some` with no definitions).
**Tests:** parsing of valid, partially valid and invalid JSON; contract normalization
(part-of-speech labels, empty groups dropped).

### Stage P4 — Declarative HTTP (translation)

**Status:** planned
**Scope:** `tagent/src/providers/http_generic.rs`. A provider described entirely by
options: `url` (template with `{text}`, `{from}`, `{to}`), `method`, `body` (template,
JSON-escaped substitution), `headers`, `result_path` (a small JSONPath subset, or plain
text), and optional `lang_map`. Ship a reference configuration for LibreTranslate (taken
from its official docs) in the docs and in a test.
**Constraints:** translation only; no scripting or conditionals, since a growing schema
would turn into a programming language. Anything that needs logic gets a native adapter.
**Naming/selection** (Q3): kind `http`. Each declarative service is a user profile, e.g.
`[provider_options.libre]` with `type = "http"` plus its templates (in `tagent-cli.toml`),
selected with `translate_provider = "libre"`.
The LibreTranslate reference config is documented as such a profile.
**Tests:** template substitution and escaping (injection-safe), the JSON-path subset,
LibreTranslate reference config against the mock server.

### Backlog (candidates, not yet planned)

- Speech: OpenAI-compatible TTS endpoint; a local engine (e.g. Piper) or system TTS
  (check `rodio` decodability, which currently has only `symphonia-mp3` enabled).
- Translation: Microsoft Translator, Yandex, Lingva (probably via P4 config instead of code).
- Dictionary: a Wiktionary-based provider.
- `tagent-gui` Settings UI for creating, editing and deleting provider profiles (Q3):
  "Add profile" → pick a type → a form generated from that kind's `OptionSpec` list
  (Stage C). Deferred until at least one generic provider (P2 or P4) exists.
- Secrets: OS keyring (`keyring`-style crate) as an additional source, deferred by Q1.
  Reasons for deferring: an extra per-OS dependency; on Linux it needs Secret Service
  over D-Bus plus a running daemon, which minimal X11 setups often lack; and it breaks the
  hand-editable config. Can be added later without changing the library or the config
  format, e.g. as a value reference `api_key = keyring:` resolved by the app.

---

## Open questions

- **Q1. Where do secrets live on the app side?** **Resolved 2026-09-26:**
  - Resolution order: environment variable → app config file → provider default.
  - Env var naming: `TAGENT_<PROVIDER>_<KEY>` (uppercase, non-alphanumerics → `_`). The
    prefix is shared by both apps, and the rule lives in the library (`env_var_name`,
    Stage B).
  - Config file: `[Provider:<name>]` sections in `tagent-cli.conf` (library keys verbatim;
    `[provider_options.<name>]` tables in `tagent-cli.toml` since Stage F3)
    and `provider_options` in `tagent-gui.json`. Both are modeled in the config structs, so
    saves don't drop them. The file is written with mode `0600` on Unix, and secrets are
    masked in `/config` and Settings.
  - OS keyring: deferred to the Backlog.

  Details are in Stage F.
- **Q2. Retry defaults.** **Resolved 2026-09-26:**
  - Retries happen only in the transport.
  - Connection failures and 502/503/504 are retried; 429 is retried only with a short
    `Retry-After`, and never for Google; timeouts, 500 and other 4xx are never retried.
  - A total time budget per call.
  - Per-provider defaults in `ProviderDescriptor::transport`: Google 1 retry / 10 s,
    keyed APIs 2 / 10 s, LLM 1 / 60 s.
  - The user-facing `max_retries` / `timeout_secs` options ship with Stage E.

  Details are in Stages C and E.
- **Q3. Several instances of one provider.** **Resolved 2026-09-26:**
  - Named profiles with an optional `type` (not composite `kind:instance` names).
  - Every `[Provider:<name>]` section / `provider_options` entry is a profile, and `type`
    defaults to the name, so the Q1 `[Provider:deepl]` case is just a profile.
  - `type` is a reserved option key resolved by the library's `*_with` factories, so the
    apps need no profile logic.
  - Profile names are `[a-z0-9_-]+` (no colon); built-in kind names are reserved.
  - One profile can serve several axes.
  - Profiles are created by hand-editing for now; a `tagent-gui` Settings UI for them is
    in the Backlog.

  Details are in Stages B, C, F and P4.
- **Q4. Ship Stage A and B together or separately?** **Resolved 2026-09-26:**
  - Separate stages and commits (A first, since B uses `InvalidOptions`), but **one**
    version, `tagent` 0.19.0, which also absorbs every later stage until the next release.
  - The general rule "one `tagent` version per release cycle" is recorded in CLAUDE.md
    ("Version Management").
  - The investigation found that `release-notes.sh` drops unpublished intermediate
    `tagent` sections (0.18.2 today). Fixed by the new prerequisite Stage 0.
- **Q5. Where should this plan live?** **Resolved 2026-09-26:**
  - Moved from the gitignored `.debug/` to the tracked `docs/providers-dev-plan.md`: it's a
    long-lived document whose decision history belongs in git, and `.debug/` is for
    temporary files.
  - The tagent-gui plan moved too (`docs/tagent-gui-dev-plan.md`), which fixed
    `docs/ARCHITECTURE.md`'s links to it.
  - `CLAUDE.md` is gitignored as well, so the Q4 "one `tagent` version per release cycle"
    rule is also recorded in the tracked `docs/ARCHITECTURE.md` ("`build.rs`: version sync"
    section).

---

## Stage template

Copy this for every new stage (foundation or provider):

```markdown
### Stage <ID> — <Provider/feature name> (<axis>)

**Status:** planned | in progress | done (YYYY-MM-DD, tagent X.Y.Z)
**Goal:** one or two sentences.
**Before starting:** official docs to read (links), open questions to settle.
**Scope / files:** new and changed files, public API sketch.
**Options:** key, required?, secret?, default (for provider stages).
**Language codes:** mapping rules (for provider stages).
**Semver:** kind of change (compatible / breaking); the number follows the
"one tagent version per release cycle" rule; app +BUILD bumps.
**Changelogs:** which crates.
**Tests:** unit / mock-server / #[ignore] live (env var).
**Docs:** rustdoc examples; CLAUDE.md / ARCHITECTURE.md sections to update.
**Done when:** concrete, checkable criteria.
**Notes after landing:** what actually shipped, deviations, follow-ups.
```
