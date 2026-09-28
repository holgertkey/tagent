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

## Stage C — Config file upgrades (planned, 2026-09-28)

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
