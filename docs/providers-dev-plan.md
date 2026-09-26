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

Stage 0 is a release-process prerequisite. Stages A–G are the **foundation** (library
infrastructure, done once). Stages P1, P2, … are **provider stages**, appended as
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

**Status:** planned
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

---

### Stage D — Capability metadata

**Status:** planned
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

---

### Stage E — Shared HTTP transport

**Status:** planned
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
  new single retry on connection failures and 502/503/504. Existing tests must pass
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

---

### Stage F — App-side options wiring

**Status:** planned
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
    [Translation]
    TranslateProvider = ollama

    [Dictionary]
    DictionaryProvider = ollama        ; one profile can serve several axes

    [Provider:ollama]
    type = openai-compat
    endpoint = http://localhost:11434/v1
    model = ...

    [Provider:deepl]                   ; no type → the built-in deepl
    api_key = ...
    ```
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

---

### Stage G — Cargo features per provider

**Status:** planned
**Goal:** users of the library compile only the providers they need.

- Features such as `google` (default), `deepl`, `openai-compat`, `http` (feature names match the kind names). Registry,
  factories and name lists are all `cfg`-gated consistently.
- The apps enable what they ship. CI builds with `--all-features` and with
  `--no-default-features` plus each feature alone (at least a `cargo check`).
- It's most useful once a second provider exists, so it can move after P1.

**Semver:** changing default features → decide at implementation time (keeping `google` in
`default` makes it additive).

---

## Provider stages

Each provider stage follows the template at the end. Planned order (it can be changed):

| Stage | Provider | Axis | Kind | Depends on |
|-------|----------|------|------|------------|
| P1 | DeepL | translation | native, keyed | A, B, E (C, D recommended) |
| P2 | OpenAI-compatible chat | translation | generic | A, B, E |
| P3 | OpenAI-compatible chat | dictionary | generic (structured JSON) | P2 |
| P4 | Declarative HTTP (reference config: LibreTranslate) | translation | generic | A, B, E |
| P5+ | *(backlog, below)* | | | |

### Stage P1 — DeepL (translation)

**Status:** planned
**Why first:** a well-documented keyed API. It validates options, auth errors, quota
errors and language-code mapping end to end.
**Before starting:** read the official DeepL API docs (endpoints for free vs pro keys, auth
header format, language-code list, limits, quota status code). Link them here.
**Scope:** `tagent/src/providers/deepl.rs`, `DeepLTranslateProvider`; options `api_key`
(required, secret) and possibly `endpoint`; BCP-47 ↔ DeepL code mapping inside the adapter;
`capabilities()` filled in; registry and const-list entries.
**Tests:** request building and response parsing (pure functions); mock-server tests for
success/401/429/quota; a live test behind `#[ignore]` plus an env var.
**Semver:** additive → `tagent` patch. The apps only need the Stage F wiring.

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
`[Provider:libre] type = http` plus its templates, selected with `TranslateProvider = libre`.
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
  - Config file: `[Provider:<name>]` sections in `tagent-cli.conf` (library keys verbatim)
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
