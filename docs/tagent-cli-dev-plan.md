# tagent-cli Development Plan

Living document: standing decisions and planned stages for `tagent-cli`. Update it when a
decision is made or a stage lands; this document *is* the current state of the plan.

Where the rest lives:
- **How things work** (config system, platform layer, hotkeys):
  [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) and the project's `CLAUDE.md`.
- **What changed and when**: [`tagent-cli/CHANGELOG.md`](../tagent-cli/CHANGELOG.md).
- **The provider architecture** (shared with `tagent-gui`): [`providers-dev-plan.md`](providers-dev-plan.md).
- **`tagent-gui`** (an independent application with its own plan):
  [`tagent-gui-dev-plan.md`](tagent-gui-dev-plan.md).

## Stage C — Config file upgrades (done, 2026-09-29: `0.17.0+005`–`+008`)

Landed as planned, C1 → C4, one changelog section each (`+007` was never built on its
own: C3 and C4 were finished together); code in `tagent-cli/src/config/upgrade.rs`.
Where the implementation settled a question the plan left open, or differs from it:

- **C1 line numbers**: `toml_edit::Document<String>` keeps spans (`Key::span()`), so
  `unknown_keys` takes a `Document` rather than a `DocumentMut` and every warning has a line.
  It is called from `load_config` (which knows the file name), not from `parse_config`.
- **C1 suggestions**: an edit distance of at most a quarter of the longer name (at least
  1) catches typos, but the plan's own example (`auto_hide_seconds` →
  `auto_hide_terminal_seconds`) is distance 9, so a second rule also suggests a candidate
  that contains every `_`-word of the unknown name (at least 6 letters in all). Keys at
  the top level, outside any section, are reported too, with the section they belong in.
- **C3 ordering**: a missing section is placed after the last template section the file
  has (`Table::set_position`), not appended at the end of the file, which would put it
  after the user's profiles. A missing file is created (the same file a first start
  writes). The validity check is the same `ConfigFile` deserialization as at startup, so
  a file that is valid TOML but has e.g. a string where a number belongs is refused too.
  In interactive mode the new file is read back into the configuration in effect (not just
  its mtime recorded), so an edit the hot reload hadn't picked up yet isn't lost.
- **C3 profile examples**: the plan's marker, the `# Provider profiles` line, is older
  than the examples: files written by `0.17.0+000` to `+003` have the explanation (with a
  small `# Example:`) but none of the ready-made blocks. So the examples have a marker of
  their own (`# Ready-made profiles`): a file with neither gets both, a file with the
  explanation only gets the examples. C4 counts keys only, so such a file gets no startup
  notice for the examples.
- **C3/C4 commented-out keys**: a commented-out section header (`# [colors]`) also starts
  a section for this check, so a whole section can be commented out on purpose.
- **Dispatch**: both flags run in `main.rs` before `CliHandler::new()`
  (`cli::ConfigFileCommand`), which would otherwise create a missing file and print the C1
  warnings a second time. `/c update` works as well as `/config update`.

### Problem

`tagent-cli.toml` is written once, when it doesn't exist, from the commented
`config_template()`. After that the application only ever edits the two language keys
(`/save`). When a later version changes the file's contents, an existing file doesn't
follow. There are two separate problems:

1. **New settings are invisible.** A missing key takes its default (`#[serde(default)]`),
   so nothing breaks, but a user with an old file never learns about new options, their
   explanations or the example provider profiles (added in `0.17.0+004`, which reach only
   a newly created file).
2. **A renamed or removed key fails silently.** Unknown keys are ignored without a word,
   so after a rename the user's old value is quietly replaced by the default. This is the
   more dangerous of the two, since the user loses a setting without noticing.

### Decisions

- **The application never rewrites the config file on its own.** The file is hand-edited.
  An unexpected rewrite (key order, blank lines, comments) is unwelcome, and a write at
  startup would also trigger the mtime-based hot reload. Every write is an explicit user
  command, like `/save` today.
- **No migrations for renamed keys** (the maintainer's standing preference: a pure rename
  plus a changelog note, no compatibility fallbacks). C1's warning tells the user what
  happened instead; the old name keeps not working.
- **No `config_version` key.** "What is new" is computed by comparing the file with the
  template, so there is no counter to maintain or to forget to bump.
- **Upgrades are additive.** Missing keys, sections and comments are added; nothing is
  deleted or renamed. Unknown or obsolete keys are reported, and the decision stays with
  the user.

### Steps

Suggested order: C1 first (it prevents real data loss), then C2 and C3 (C3 builds on the
code C2 needs), then C4 (one line on top of C3's comparison). Each step is its own `+BUILD`
bump with a `tagent-cli/CHANGELOG.md` entry.

#### C1 — Warn about unknown keys on load

- The known keys come from the parsed `config_template()` (every `[section]` and `key`),
  so the list can't drift from the template. `provider_options` is free-form and is
  handled separately (see below).
- Warn once per load (startup and every hot reload that actually re-reads the file), one
  line per key, on stderr. Never fatal:
  ```
  Warning: tagent-cli.toml line 12: unknown key `auto_hide_seconds` in [interface]
           (did you mean `auto_hide_terminal_seconds`?)
  ```
- Unknown sections are reported the same way (e.g. `[Provider]`: sections are lowercase),
  with a suggestion against the known section names.
- The "did you mean" suggestion uses a small edit-distance function (a few lines, no new
  crate), offered only below a distance threshold, and also across sections: a key that
  moved to another section is the typical case after a restructuring
  (`copy_to_clipboard` under `[translation]` → "it belongs in [interface]").
- Line numbers: `DocumentMut` drops spans. Check in the `toml_edit` docs whether parsing
  with `toml_edit::Document` (which keeps spans) gives the key's line. Without it, name
  the section and key only.
- `[provider_options.<name>]`: for a profile whose kind is a built-in provider, warn about
  option keys the kind doesn't declare in `tagent`'s registry (`OptionSpec`s, plus
  `type`), e.g. `apikey` instead of `api_key`. A profile of an unknown kind is not checked
  here: building it already reports the error.
- Implementation: a pure function `unknown_keys(&DocumentMut) -> Vec<UnknownKey>`, called
  from `parse_config`'s callers next to the existing `target_language = "Auto"` notice.
- Tests: an unknown key with a suggestion, one without, an unknown section, a key in the
  wrong section, an undeclared profile option, and no warnings at all for a freshly
  generated file (`render_config(&Config::default())`).

#### C2 — `--print-default-config`

- Prints `render_config(&Config::default())` (the full template, with every explanation
  and the example profiles) to stdout, and touches no file. This is the fallback for users
  who maintain the file by hand: `tagent-cli --print-default-config > new.toml`, then diff.
- Handled in `cli.rs` like `--config`. Add it to `--help` and `tagent-cli/README.md`.
- Test: the output parses back into `Config::default()` (already covered for
  `render_config`, so the test only checks the flag's wiring).

#### C3 — `--update-config` and `/config update`

- Explicit command, from the command line (`tagent-cli --update-config`) and in
  interactive mode (`/config update`; add it to `SLASH_COMMANDS`, `display_help()` and the
  README's command list, see "Adding New Interactive Commands" in `CLAUDE.md`).
- Steps:
  1. Copy the file to `tagent-cli.toml.bak` next to it (overwriting an older backup),
     with the same `0600` mode as the config file.
  2. Parse the file and the template as `DocumentMut`s.
  3. For each template section missing from the file, append the whole table with its
     comments (the template item's decor).
  4. For each template key missing from an existing section (and not commented out
     there, see C4), insert it with its
     default value and the template's comments above it (copy the key's decor, as
     `set_value` already does for values). Append it at the end of the section; `toml_edit`
     keeps insertion order, so matching the template's order exactly isn't worth the extra
     code.
  5. Add the "Provider profiles" explanation and the example blocks (`profile_examples()`)
     at the end of the file if the file doesn't contain them yet (detected by the
     `# Provider profiles` marker line).
  6. Write through `write_config_file`. In interactive mode, update the stored mtime so
     the hot reload doesn't re-announce the change.
- Nothing is removed or renamed. The report lists what was added, where the backup is,
  and C1's unknown keys ("not changed: ...").
- When nothing is missing: no backup, no write, just "tagent-cli.toml is up to date".
- A file that doesn't parse is not touched; the parse error is shown (same message as at
  startup).
- Tests: an old-style file (a few keys missing, a section missing, custom comments and
  an unknown key) → the user's values, comments and the unknown key are unchanged, the
  missing keys arrive with their comments, the result parses, and a second run changes
  nothing. Plus: the backup exists and equals the original; an up-to-date file is left
  untouched (mtime unchanged).

#### C4 — Startup notice about new settings

- In unified/interactive mode, one line after the banner when the file lacks template
  keys (the same comparison as C3):
  ```
  Config: 3 new settings are available (run /config update or tagent-cli --update-config)
  ```
- Not in one-shot CLI mode, whose output may be consumed by scripts.
- No state file and no "already announced" memory (decided 2026-09-28). A key the user
  commented out (a comment line of the form `# key = ...` in its section) counts as
  present, so that is the way to silence the notice for a key on purpose. A key deleted
  outright keeps the notice showing on every start, deliberately.
- Known limitation, accepted: the template's own example comments (`#   translate_hotkey =
  "Ctrl+Ctrl"` under `[hotkeys]`) have the same form, so deleting such a key's line but
  keeping the examples above it also silences the notice. The main case, a key a new
  version adds, is unaffected: an old file has neither the key nor its comments.
- C3 treats a commented-out key the same way and doesn't add it back, so the two stay
  consistent: after `--update-config` the notice is gone.
- Tests: a missing key triggers the notice, a commented-out one doesn't (including an
  indented `#   key = ...` form), and a key commented out in another section doesn't
  count for this one.

### Out of scope

- `tagent-gui`: its `tagent-gui.json` has no comments, missing fields already take their
  defaults through serde, and Settings rewrites the whole file on save. Its only related
  gap is that unknown fields are dropped silently on save (see "Lessons worth keeping" in
  [`tagent-gui-dev-plan.md`](tagent-gui-dev-plan.md)); warning about them there would be
  its own stage in its own plan.
- Converting the pre-0.17.0 INI file `tagent-cli.conf`: it is no longer read, with no
  migration (decided with Stage F3).

## Stage S — Switching the translation provider in a session (done, 2026-09-29: `0.17.0+009`)

Landed S1–S3 in one `+BUILD` (`+009`, one changelog section) rather than one each, plus
S4. Where the implementation settled a question the plan left open, or differs from it:

- **S3, a file without `translate_provider`**: `set_value` would add a `[provider]` section
  on every `/save`, even when the provider was never switched. `with_session_settings`
  writes the key only when the file already has it or the value differs from the default
  a missing key stands for; the languages are written as before.
- **S2, `/p` without arguments** re-reads the file first (`reload_or_warn`), like `/config`,
  so the list shows the profiles as they are on disk. `/p <name>` does the same before
  validating.
- **S2, the switch** is a free function `switch_translate_provider(&ConfigManager, name)`
  in `interactive.rs`, so it is tested without an `InteractiveMode`. Errors are shown in
  `error_color`, with "(translation provider unchanged)".
- **S2, the list** (`Config::translation_provider_list`, `provider_list_lines`) lives in
  `config.rs`; names are aligned in a column. A profile shows as
  `DeepL (<profile>)`, the same form `profile::Profiled` reports.
- **S1, the failed value** is forgotten once `translate_provider` is back at the working
  one, so switching to the bad value again reports it again.

### Problem

The translation provider is picked only in `tagent-cli.toml` (`[provider]
translate_provider`), and even there a change needs a restart: `Translator::build` creates
the provider once (`translator.rs`, `provider: Arc<dyn TranslationProvider>`) and every
clone of the `Translator` (the hotkey path and interactive mode) keeps it for the whole
run. Trying another provider for a moment (e.g. DeepL for one text) means editing the
file and restarting. The one place that already follows the file is speech: TTS resolves
an `"auto"` source through `config.create_translate_provider()` on every playback
(`speech.rs`), so after a hot reload translation and speech can use different providers.

### Decisions

- **New interactive command `/p`, long form `/provider`**, following `/l`/`/lang`:
  - `/p` without arguments lists the available translation providers and marks the
    active one.
  - `/p <name>` switches to that provider (a kind like `deepl` or a profile name from
    `[provider_options.<name>]`) for the current session. The file is not touched.
  - `/save` then writes the provider to `tagent-cli.toml`, along with the languages.
- **Only the translation provider**, not the dictionary or speech provider. Those can
  follow later (e.g. `/p dict <name>`, `/p speech <name>`) if needed; the command syntax
  leaves room for that, since no provider kind is called `dict` or `speech`.
- **Session state lives in the in-memory `Config`**, exactly like `/l`: `/p` sets
  `translate_provider` via `ConfigManager` (no separate "session override" state as in
  `tagent-gui`'s `session_provider.rs`). Everything that reads `translate_provider` from
  the config (translation, TTS `"auto"` resolution, `/config`, the banner) follows
  automatically.
- **A hot reload of an edited file resets the session choice**, same as for `/l` today:
  `check_and_reload` replaces the whole in-memory `Config`. Accepted for consistency;
  `/save` before editing the file keeps the choice.
- **`Translator` rebuilds the provider when `translate_provider` changes** (by comparing
  the name it was built from with the config's), instead of `/p` pushing a new provider
  into it. This is what makes the hotkey path see the switch, and as a side effect
  `translate_provider` becomes live-reloaded from the file too, which also removes the
  translation/speech mismatch above.
- **`/p <name>` validates immediately**: it builds the provider before switching. On
  failure (unknown name, missing `api_key`, ...) it prints the error and keeps the current
  provider, so a bad choice never surfaces later on a hotkey translation.

### Steps

Each step is its own `+BUILD` bump with a `tagent-cli/CHANGELOG.md` entry (S4 is
documentation only, no bump).

#### S1 — Switchable translation provider in `Translator`

- Replace `provider: Arc<dyn TranslationProvider>` with a slot shared by all clones, like
  `last_translation`:
  ```rust
  struct ActiveTranslation {
      /// The `translate_provider` value this provider was built from.
      name: String,
      provider: Arc<dyn TranslationProvider>,
  }
  translation: Arc<Mutex<ActiveTranslation>>,
  ```
  (`std::sync::Mutex`: the lock is held only to compare/replace, never across an `.await`.)
- New private `fn translation_provider(&self, config: &Config) -> Arc<dyn
  TranslationProvider>`: if `config.translate_provider` differs from `name`
  (case-insensitively, since the factories are), build the new one with
  `config.create_translate_provider()` and replace the slot; return a clone of the `Arc`
  and drop the lock before the caller awaits.
- **Rebuild failure** (a hot reload brought a bad `translate_provider`): keep the old
  provider and print the error once, not on every translation. Remember the failed name
  in the slot (`failed: Option<String>`) and warn again only when the name changes. Startup
  stays as it is: an unusable `translate_provider` at start is fatal.
- Call sites: `Translator::translate_text_internal` (the only place that calls
  `provider.translate_text`) gets the provider through `translation_provider`, using
  `config_manager.get_config()` read at that moment (its callers have already run
  `check_and_reload`), and `active_providers` (banner) goes through
  `translation_provider(config)` too, so a banner shown after `/p` and before any
  translation (`/clear`) already names the new provider.
- `ConfigManager::set_translate_provider(&self, name: &str)`, in memory only, next to
  `set_languages`.
- Changelog (`Changed`): `translate_provider` is now live-reloaded from the file, no
  restart needed; translation and speech's language detection always use the same provider.
- Tests (with the existing `MockProvider` seam):
  - a changed `translate_provider` rebuilds the provider, an unchanged one doesn't
    (same `Arc`, `Arc::ptr_eq`);
  - two clones of a `Translator` see the same switch (the hotkey path case);
  - a failing rebuild keeps the previous provider and reports the error only once;
  - a name differing only in case doesn't rebuild.
  Building a real provider in a test needs a name the factory accepts without network
  access (`google`; `deepl` with a dummy `api_key` in `provider_options`); nothing is
  translated in these tests, so no request goes out.

#### S2 — The `/p` and `/provider` command

- Parsing: a pure `parse_provider_command(text) -> Option<ProviderCommand>` (`List` /
  `Switch(name)`), like `parse_speech_command`: the bare `/p`/`/provider` and the
  `"/p "`/`"/provider "` prefixes; `text` is already trimmed. More than one argument is an
  error with a usage line (`Usage: /p [provider]`).
- `/p` (list): the translation kinds compiled in (`TRANSLATION_PROVIDERS`) and the
  profiles whose kind is one of them (`ProviderProfiles::profiles_of_kinds`), sorted as
  those two lists already are, with the active one marked and each line showing the
  display name:
  ```
  Translation providers:
  * google       Google Translate
    deepl        DeepL (missing: api_key)
    deepl-work   DeepL (deepl-work)
  Switch with /p <name>; /save keeps the choice.
  ```
  - The display name and required options come from `tagent`'s registry
    (`translation_providers()`, matched by the profile's `kind_of`); a required option is
    "missing" when it is absent or empty in `config.provider_options(name)` (which
    includes `TAGENT_<NAME>_<KEY>` env overrides). No provider is built for the list, so it
    makes no network calls and shows no secrets.
  - A pure helper `provider_list(config) -> Vec<ProviderListEntry>` does the work, and
    printing is a thin layer on top (tested without stdout).
- `/p <name>` (switch):
  1. Name normalized to lowercase (profile names are lowercase in `ProviderProfiles`).
  2. Same as the active one → `Translation provider: DeepL (already active)`.
  3. Otherwise build it: a copy of the current `Config` with the new `translate_provider`,
     then `create_translate_provider()`. On error print the ready-made message
     (`provider_error_message` already names `[provider_options.<name>]` for an
     `InvalidOptions` and lists the supported kinds for an unknown one) and change nothing.
  4. On success: `config_manager.set_translate_provider(name)`, and print
     `Translation provider: DeepL (this session; /save to keep)`. The provider built for
     validation is discarded; S1's rebuild on the next translation is cheap (no network
     in the constructors), and keeping one path for "use the configured provider" is
     simpler than handing it over.
- Registration (see "Adding New Interactive Commands" in `CLAUDE.md`): `SLASH_COMMANDS`
  (`/p`, `/provider`), `ConfigManager::display_help()`, the `Commands:` block of
  `display_banner()` (`/p (provider)`), and the "Interactive Commands" list in
  `tagent-cli/README.md`. Tab-completion of provider names after `/p ` is a nice-to-have,
  left out of this stage.
- Tests: the parser (bare forms, with an argument, extra whitespace, too many arguments,
  `/pp` and `/print` not matching); the list (built-in kinds, a profile of a translation
  kind, a profile of an unknown kind left out, the active marker, a missing required
  option, a required option supplied through the env override via `options_using`); the
  switch (success changes the config, failure leaves it unchanged, the same name is a
  no-op).

#### S3 — `/save` also saves the translation provider

- `with_languages` becomes `with_session_settings`: besides `source_language`/
  `target_language` it sets `[provider] translate_provider` through the same `set_value`
  (comments, order, unknown keys and profiles are kept; a missing `[provider]` section or
  key is created, which `set_value` already does). Writing an unchanged value is a no-op
  on the text.
- A missing file is still written in full by `render_config`, which already includes
  `translate_provider`.
- Update the doc comments of `save_config` and `with_*` ("the only values the app itself
  changes" now lists three keys) and the `/save` line in `display_help()`/README
  ("saves the languages and the translation provider").
- The saved provider name is the one `/p` validated, so `/save` needs no extra check.
- Tests: `/p`-style change + save → the file has the new `translate_provider` and the
  user's comment above the key and its inline comment survive; a file without a
  `[provider]` section gets one; languages are still saved as before; a second save with
  no changes leaves the text byte-identical.

#### S4 — Documentation

- `CLAUDE.md`: the `Translator Orchestrator` and `Configuration System` sections
  (`translate_provider` is live-reloaded; `/save` writes the provider too), the
  "Adding New Interactive Commands" example list if it names the commands.
- `docs/ARCHITECTURE.md`: wherever it says the translate provider is built once for the
  whole run.
- `tagent-cli/README.md`: the command list (done in S2/S3), and the configuration
  section's note on which settings need a restart.
- This plan: mark the stage done, with what the implementation settled differently.

### Out of scope

- Switching the dictionary and speech providers (`/p dict ...`, `/p speech ...`); see
  Decisions. The dictionary provider is also built once per run today, so it would need
  the same slot as S1.
- `tagent-gui`: it already has its own session-only provider picker (`tagent-gui` 0.14.0+030).
- A one-shot CLI flag (`tagent-cli --provider deepl "text"`): a separate small feature
  if wanted; S1's rebuild-on-change makes it trivial.
- Showing in `/config` that the active provider differs from the file.

## Stage L — Language codes in the config, target language from the locale (done, 2026-09-29: `0.17.0+010`–`+011`)

Landed as planned: L1 in `tagent` (`0.19.0`, unreleased) and `tagent-gui` `0.14.0+033`, L2
as `tagent-cli` `0.17.0+010`, L3 as `+011`, L4 with them. Where the implementation settled
a question the plan left open, or differs from it:

- **L1 in `tagent-gui`**: `known_language_code` accepts codes only, as before, so it is
  `language_code` filtered to a result that equals the input and isn't `"auto"`. Taking
  `language_code` as it is would have made the GUI accept names (`"Russian"` → `"ru"`),
  a behavior change the plan ruled out.
- **L1 locales**: `sys-locale` 0.3 already converts POSIX locales to BCP 47 on Unix
  (`ru_RU.UTF-8` → `ru-RU`); `language_for_locales` still splits on `-`, `_`, `.` and
  `@`, so either form works.
- **L2 input**: `ConfigManager::normalize_language` is gone; `config::language_code` (name
  or code → code, anything else kept as written) replaces it for `/l` and `-l`. `-l` still
  counts only listed languages (and `auto`) as a source, as before.
- **L2 warnings**: `parse_config_with_warnings` returns them and `load_config` prints them
  with the file name (`Warning: tagent-cli.toml: ...`), so `/config update`'s re-read
  doesn't repeat them; the `Auto`-target warning moved there too. `parse_config` (tests,
  the upgrade check) drops them.
- **L2 display**: `/config` shows the name after a listed code only
  (`target_language = "ru"  # Russian`); an unlisted code gets no note instead of
  repeating itself. `language_pair_description` does the same (`English (en) -> uk`).
- **L3 template**: `target_language`'s value is `Config::default()`'s, interpolated like
  the provider lists, so the "template equals the defaults" test holds on any locale. The
  test suite was run under `ru`, `de`, `C` and `eo` locales.

### Problem

- **`tagent-cli.toml` stores language names** (`source_language = "Auto"`,
  `target_language = "Russian"`), while everything behind the config works with codes:
  about thirty places convert with `tagent::languages::name_to_code` (`get_language_codes`,
  `LanguagePair`, the banner, `/config`, `/l`, `-l`). A language missing from `tagent`'s
  table can only be stored as a code anyway (`/l uk` → "Unknown language 'uk', using as
  language code"), so files already mix both forms. `tagent-gui.json` stores codes.
- **The default target language is hard-coded** as `"Russian"` (`Config::default()`), so a
  user with an English or German system gets Russian translations on first run.
  `tagent-gui` has taken its default from the system locale since `0.14.0+031`
  (`target_language_for_locales`, `sys-locale`).

### Decisions

- **Codes in the file, names only on screen.** `source_language`/`target_language` hold
  codes as `tagent`'s table spells them (`"auto"`, `"ru"`, `"en"`); prompts
  (`[Russian]: `), the banner (`Auto (auto) -> Russian (ru)`) and `/l`'s messages keep
  showing names, through `code_to_name`, which returns the code itself for a language the
  table doesn't list.
- **Both forms are read** (decided 2026-09-29, an exception to the "no migration shims"
  preference, since otherwise every existing file would silently become a broken code):
  on load, a listed name or code (any case) becomes the listed code, `Auto` becomes
  `"auto"`. The same leniency `/l` and `-l` already have. **The file is not rewritten**
  (Stage C's rule); the codes reach it with the next `/save`, which writes the values in
  effect.
- **An unknown value is kept, with a warning**, unlike `tagent-gui` (which replaces it
  with the default): `tagent-cli` passes any code to the provider, including languages
  `tagent`'s table doesn't list, and that stays possible. The warning (once per load, like
  C1's unknown keys) names the key and says the value is used as a language code.
- **Default target language = the system locale**, like `tagent-gui`: the first locale
  (most preferred first) whose primary subtag is in `tagent`'s table; `"en"` when none is.
  It applies wherever `Config::default()` does: a new file, and a file without
  `target_language`. An existing value is never replaced.
- **The locale lookup moves into `tagent`** so both apps share it (see the memory
  "prefer calling into tagent's library modules"): a pure function over the locale list,
  with `sys-locale` staying in the applications, so the library gains no dependency and
  no environment access.
- **`AUTO_TARGET_FALLBACK` stays English** (as the code `"en"`): it replaces an `Auto`
  target and the new target of `/l` swapping an `Auto` source. Taking the locale there too
  is possible but a separate behavior change, not part of this stage.

### Steps

Order: L1 first (both apps build on it), then L2, then L3. `tagent` has no bump: its
`0.19.0` is unreleased (the last release, `v0.16.0`, published `0.18.1`), so the addition
joins the open `0.19.0` section of `tagent/CHANGELOG.md`. `tagent-cli` and `tagent-gui`
get a `+BUILD` bump and a changelog entry per step that touches them.

#### L1 — Language lookup helpers in `tagent::languages`

- `pub fn language_code(input: &str) -> Option<&'static str>`: the table's code for a
  listed code or name, compared case-insensitively (`"Russian"`, `"ru"`, `"RU"` → `"ru"`);
  `"auto"` (any case) → `"auto"`, matching `name_to_code`/`code_to_name`; `None`
  otherwise. Callers that need a concrete language check for `"auto"` themselves.
- `pub fn language_for_locales(locales: impl IntoIterator<Item = impl AsRef<str>>) ->
  Option<&'static str>`: the code of the first locale (BCP 47 or POSIX, `"ru-UA"`,
  `"de_DE.UTF-8"`) whose primary subtag is listed; `None` when none is. The fallback
  (`"en"`) is the caller's choice. Check what `sys_locale::get_locales()` returns on
  Linux/Windows (docs.rs) for the `_`/`.` forms, and cover them in the tests.
- Doc comments with `# Examples` (RFC 1574), `cargo doc -p tagent` clean.
- `tagent-gui`: `target_language_for_locales` becomes
  `languages::language_for_locales(locales).unwrap_or("en")` and `known_language_code`
  becomes `languages::language_code` (keeping the `"auto"` check where a target is meant).
  Behavior unchanged; its existing tests keep passing, the moved cases go to `tagent`.
- Tests in `tagent`: names and codes in any case, `auto`, an unlisted value; locales in
  order of preference, an unknown first locale skipped, region and encoding suffixes, an
  empty list.

#### L2 — Codes in `tagent-cli.toml`

- **Load** (`parse_config`, which already resolves an `Auto` target through
  `LanguagePair`): each language value goes through `language_code`; an unlisted value is
  kept as written (not lowercased: a region subtag like `zh-TW` is conventionally
  uppercase) with the warning from Decisions.
- **In memory**: `Config::source_language`/`target_language` are codes. Remove the
  `name_to_code` round trips (`get_language_codes` returns the fields,
  `LanguagePair::finish` compares codes directly, the banner, `/config`'s note);
  display sites use `code_to_name` (prompts in `translator.rs`/`interactive.rs`, the
  banner, `/l`'s "Languages set: Russian (ru) -> ..." lines, `/config`'s note becomes the
  name: `target_language = "ru"  # Russian`).
- **Input**: `ConfigManager::normalize_language` (name or code → name) becomes a
  name-or-code → code function built on `language_code` (unknown input kept, as now,
  with `/l`'s existing warning); used by `/l` and `-l`. `LanguagePair`,
  `AUTO_TARGET_FALLBACK` (`"en"`) and `is_auto` work on codes; their doc comments say so.
- **Template**: `source_language = "auto"`, `target_language = "<default>"`; the
  "Supported values" comments list codes with names (`en (English), ru (Russian), ...`),
  generated from `tagent::languages::LANGUAGES` like the `{translate_providers}`
  placeholder, so a new language reaches the template by itself. A line saying names are
  accepted too.
- **Write**: `/save` (`with_session_settings`) and `render_config` write the codes. A
  file with names therefore switches to codes on its first `/save` (both keys, since
  `/save` always writes the languages); mention it in the changelog.
- **Stage C interplay**: `missing_settings`/`upgrade` compare keys only, so nothing
  changes there; `--update-config` doesn't convert names (it never changes values).
- **History, CLI output**: check `save_translation_history` and the CLI mode's output for
  places that print `config.*_language` expecting a name.
- Tests: loading names, codes and mixed case gives codes; `Auto`/`auto` → `"auto"`; an
  unlisted value is kept and warned about; an `Auto` target is still replaced (`"en"`);
  `/save` of a file with names writes codes and keeps comments; prompts still show names
  (the existing `hotkey_translation_emits_label_and_text_in_one_printer_call` asserts
  `[Russian]: `); `/l` with a name and with a code store the same code; the template
  parses back into `Config::default()` (existing test).

#### L3 — Default target language from the locale

- Add `sys-locale = "0.3"` to `tagent-cli/Cargo.toml` (the version `tagent-gui` uses).
- `Config::default()` takes `target_language` from `default_target_language()` =
  `language_for_locales(sys_locale::get_locales()).unwrap_or("en")`. `ConfigFile`'s section
  defaults come from `Config::default()` (the macro), so a missing key gets it too.
- The template's comment says `Default: the system language (from the locale), else en`
  instead of naming one. `--print-default-config` then prints the local default, which is
  fine: it is "what a new file would be here".
- Tests must not depend on the machine's locale: keep the pure part in `tagent` (L1) and
  have tests that need a fixed default build their `Config` explicitly. Audit the tests
  that assume `"Russian"`/`"ru"` from `Config::default()` (e.g. `test_config_manager` in
  `translator.rs` writes its languages, others compare with `Config::default()` and are
  fine either way).
- README: the configuration example and the first-run description.

#### L4 — Documentation

- `CLAUDE.md` ("Configuration System": codes, both forms read; "Adding a New Language":
  the template list is generated), `docs/ARCHITECTURE.md` (the config file section),
  `tagent-cli/README.md` (config example with codes, `/l` accepts both, default target),
  `tagent-gui/CHANGELOG.md` for L1's refactor (no user-visible change: a `Changed` line or
  none, the maintainer's call).
- This plan: mark the stage done, with what the implementation settled differently.

### Out of scope

- Rewriting existing files to codes on load or with `--update-config` (see Decisions:
  the next `/save` does it).
- A locale-based `AUTO_TARGET_FALLBACK`.
- Language names in other languages (`Русский`) as input: the table has English names
  only.
