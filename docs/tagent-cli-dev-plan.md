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

## Stage S — Switching the translation provider in a session (planned, 2026-09-29)

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
