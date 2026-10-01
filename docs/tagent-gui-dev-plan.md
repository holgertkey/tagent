# tagent-gui Development Plan

Living document: the concept, standing decisions and roadmap for `tagent-gui`. Update it
when a decision is made or a stage lands; this document *is* the current state of the
plan.

Where the rest lives:
- **How things work** (mechanics, platform details, known gaps):
  [`docs/ARCHITECTURE.md`](ARCHITECTURE.md), section "tagent-gui: Slint desktop GUI".
- **What changed and when**: [`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md).
- **The provider architecture** (shared with `tagent-cli`): [`providers-dev-plan.md`](providers-dev-plan.md).

Per-stage working notes (step-by-step build plans, verification logs, same-day follow-up
narratives) were condensed out of this document on 2026-09-26.

## Concept (decided 2026-08-15)

`tagent-gui` is a **fully independent application** from `tagent-cli`, not a companion to
it. The only thing the two share is the `tagent` library. "Independent" means:

- **Own interface**: a Slint desktop GUI with its own look, layout and interaction model.
- **Own configuration**: its own `tagent-gui.json` in its own directory, never
  `tagent-cli`'s config file (`tagent-cli.toml`; no migration path from the old inline
  reader). The file is plain
  JSON, meant to be hand-editable, and live-reloaded by mtime before each translation.
- **Own feature set**: the roadmap is not "catch up with `tagent-cli`". Features are chosen
  on their own merits and reimagined for a windowed app; e.g. global hotkeys lead to a popup
  and a tray, not a terminal.
- **Own versioning**: `MAJOR.MINOR.PATCH+BUILD`, with the same format and increment rules as
  `tagent-cli` but an independent counter, and no `build.rs` sync. Only `+BUILD` is bumped
  as part of implementation work; a MAJOR/MINOR/PATCH change is always the maintainer's
  explicit call.
- **Own changelog**: [`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md).
  `tagent-gui/README.md` is updated only on a MAJOR/MINOR/PATCH change, not on every
  `+BUILD`.

## Architecture policy

- `tagent-gui` depends **only** on the `tagent` library, never on `tagent-cli`, not even for
  platform code it also needs (hotkeys, clipboard, window management). Such code is
  duplicated into `tagent-gui/src/platform/` instead (see Q3 below).
- Prefer calling into `tagent`'s library modules over duplicating *domain* logic. Example:
  the dictionary article layout moved into `tagent::article` (`tagent` 0.18.3) and is shared
  by both apps.
- `tagent` has three independent provider axes: translation, dictionary and speech. Each
  has its own factory and its own `tagent-gui.json` key (`translate_provider`,
  `dictionary_provider`, `speech_provider`, all live-reloaded, all with Settings dropdowns).
  No trait depends on another. The only place two axes meet is speaking `"auto"`-source
  text, where a translate provider is built lazily just for `detect_language`.
- A dictionary or speech provider that fails to build never breaks translation: it logs a
  warning and falls back to plain translation.

## Resolved questions

1. **Version discipline** (decided 2026-08-15, revised 2026-09-11): independent versioning
   in the `MAJOR.MINOR.PATCH+BUILD` format; see Concept.
2. **Target scope** (2026-08-15): `tagent-gui` sets its own roadmap; feature parity with
   `tagent-cli` is not a goal.
3. **Where shared orchestration logic lives** (2026-08-15): by default, duplicate it
   independently in `tagent-gui` rather than extracting it into `tagent` or a new shared
   crate. Extract only once duplication is a demonstrated maintenance burden. The
   dictionary layout (`tagent::article`) is the first case that met that bar. Platform
   code stays duplicated.
4. **macOS** (2026-08-15, reaffirmed 2026-09-20): hotkeys, popup, clipboard write and
   window management stay stubs on macOS, matching `tagent-cli`. The tray, translation,
   dictionary and speech work there. Consequence, accepted and documented: with the
   view-only transcript, macOS users can't copy text out of it (Stage 13 open item 1).

## Current state (`0.14.0+036`, 2026-10-01)

Feature summary; full mechanics are in `docs/ARCHITECTURE.md`.

- **Translation**: an input box (Enter submits, Shift+Enter inserts a newline) and a
  transcript. Single words go to a dictionary lookup (`show_dictionary`, `spell_check`).
  The language dropdowns list every language of `tagent::languages::LANGUAGES` (15,
  since 0.14.0+025), and "Auto" is offered only as a source language.
- **Transcript**: view-only `StyledText` blocks with semantic highlighting (part of speech,
  synonyms, correction notice, prompt, errors; colors derived from each block's
  background). Right-click copies a block, optionally through a one-item menu
  (`show_context_menu`).
- **Speech**: 🔊/⏹ buttons per transcript row, one playback at a time app-wide; a speech
  hotkey (default `Alt+S`) speaks the selection; Esc cancels from any application.
- **Global hotkey** (default `Alt+A`, Linux/Windows): copies the selection, translates it
  into the transcript, and shows the popup.
- **Popup**: frameless, always on top, next to the cursor, with the same highlighting as
  the transcript. It auto-hides (`popup_auto_hide_seconds`) and stays while hovered.
  Right-click copies a line; left-drag moves it; the position can be remembered and is
  clamped to the desktop. It can be switched off (`show_popup`).
- **Tray**: close-to-tray, `start_minimized`, Show / Settings… / Quit.
- **Settings dialog**: tabs General / View / Popup / Hotkeys & Tray; hotkey "Record" button
  with live validation; Reset to Defaults; themes and color schemes; prompt colors.
- **Providers**: translation, dictionary and speech pickers in Settings > General (plus a
  session-only translation picker in the main window), user profiles from
  `provider_options`, and an "Options…" panel per picker built from `tagent`'s registry
  (password fields for secrets, ⚠ for missing required options, a multi-line editor with
  the built-in default for prompts). Google, DeepL and OpenAI-compatible translation.
- **Window geometry** is remembered (`remember_window_geometry`).
- **Terminal detach** on Linux/macOS (`--foreground` / `-f` to stay attached; the log goes
  to `tagent-gui.log` in the data dir).
- **Linux desktop integration** (0.14.0+023): `--install-desktop` / `--uninstall-desktop`
  write/remove a `.desktop` file and the icon in the user's data dir; the window class is
  pinned to `tagent-gui`, so GNOME's dock shows the app icon. Releases (0.14.0+024) ship the
  same entry and icon in the Linux archive and a `.deb` that installs them system-wide.

## Shipped stages

| # | Stage | Shipped | Essence |
|---|-------|---------|---------|
| 1 | Own config + live-reload | 2026-09-11 | `GuiConfig`/`GuiConfigManager`, JSON, mtime reload |
| 3 | Settings window | 2026-09-11 | ⚙ → Slint `Dialog`, OK saves / Cancel discards; tabs; theme |
| 4 | Clipboard button | 2026-09-13 | per-OS `ClipboardManager` + 📋 button (effectively "paste": the click steals focus first) |
| 5 | Global hotkey | 2026-09-13 | `rdev` + `XGrabKey` (Linux), `WH_KEYBOARD_LL` + Alt swallow-and-replay (Windows) |
| 6 | Popup window | 2026-09-14 | `TranslationPopup` next to the cursor; `Timer` *element* in `.slint` for auto-hide |
| 7 | System tray | 2026-09-15 | Slint's own `SystemTrayIcon` (slint 1.17.1), no new crate; window geometry |
| 8 | Settings wiring | 2026-09-16 | hotkey / popup-delay controls, "Record" button, Reset to Defaults |
| 9 | Dictionary lookup | 2026-09-18 | single-word → dictionary article + correction notice |
| 10 | Text-to-speech | 2026-09-18 | per-row 🔊 buttons via `rodio`; follow-up: speech hotkey + global Esc |
| 11 | `SpeechProvider` (`tagent`) | 2026-09-19 | speech split from translation into its own provider axis |
| 12 | `DictionaryProvider` (`tagent` 0.18.0) | 2026-09-20 | dictionary split into its own axis; `#[non_exhaustive]` entry types |
| 13 | Styled transcript | 2026-09-22 | view-only `StyledText`, semantic highlighting, right-click copy |
| — | Multi-line provider options | 2026-10-01 | `TextEdit` for `OptionSpec::multiline`, pre-filled default, Reset, `{to}` ⚠ (0.14.0+036) |

Later iterations `0.14.0+003`–`+019` (2026-09-22…25) built on Stage 13: prompt colors,
optional right-click menu, popup copy and drag, popup highlighting, terminal detach, a
"Show popup" switch, Windows hotkey/Esc/layout-switch fixes, layout-independent Ctrl+C on
Linux (no `xdotool` needed), and the shared `tagent::article`. See the changelog.

## Roadmap

Candidates, not yet scheduled; the order is a suggestion.

1. ~~**Stage 2 — Language list expansion.**~~ Done in 0.14.0+025 (2026-09-27): the
   dropdowns are built from the new `tagent::languages::LANGUAGES` table instead of a
   hardcoded 5-language list.
2. **History logging.** A candidate, not prioritized; no design yet.
3. **Provider options in Settings.** Keys, endpoints and user profiles, following
   [`providers-dev-plan.md`](providers-dev-plan.md) Stage F and its Backlog. Options of
   existing profiles: done ("Options…" panel, 0.14.0+028; multi-line options,
   0.14.0+036). Creating and deleting profiles, plus a "Test" button: planned (decided
   2026-10-01), see [below](#planned-stage--provider-profiles-tab).
4. ~~**Multi-line provider options.**~~ Done in 0.14.0+036 (2026-10-01): a `multiline`
   option (today `openai`'s `translate_prompt`) gets a `TextEdit` pre-filled with its
   `default`, "Reset to default", and a soft ⚠ when `{to}` is missing; driven only by the
   `OptionSpec`, so P3's `dictionary_prompt` needs no GUI change. See the changelog and
   "tagent-gui: Slint desktop GUI" in `docs/ARCHITECTURE.md`.
5. **Slint upgrade** once [slint-ui/slint#13624](https://github.com/slint-ui/slint/issues/13624)
   (empty tray menu after a slow start) is fixed upstream. Bump `slint` and `slint-build`
   together and drop the known-gap entry.

### Planned stage — Provider profiles tab

**Status:** planned (2026-10-01). Once shipped, condense this section to a row of the
"Shipped stages" table and a changelog entry, like the other stages.

**Goal.** Users create and delete provider profiles in Settings instead of hand-editing
`provider_options` in `tagent-gui.json`, typically several `openai` instances side by
side (Ollama, OpenAI, OpenRouter, ...), and can check a profile with a "Test" button.
Generic: any provider kind, driven by `tagent`'s registry, with **no provider-specific
GUI code** and **no presets** (decided 2026-10-01: endpoints and models are typed by the
user; the `OptionSpec` descriptions already give examples).

**Behavior.**
- A new **"Providers"** tab after "General". It lists, one row each:
  - every built-in kind compiled into `tagent` (`google`, `deepl`, `openai`, in registry
    order), marked "built-in", with "Options…" and "Test", but no "Delete";
  - every user profile (a `provider_options` entry whose name isn't a built-in kind), as
    `name (kind)`, with "Options…", "Test" and "Delete". A profile with an unknown kind
    (hand-edited, or its feature compiled out) is listed with "unknown kind" and only
    "Delete".
  "Options…" opens the existing options panel (the same one the General pickers use).
- **Add** (a form under the list): a name field, a kind `ComboBox` (every built-in kind,
  `openai` preselected when compiled in, else the first) and an "Add" button, disabled
  while the name is invalid; the reason shows live under the field (same pattern as the
  hotkey fields). Invalid: empty, characters outside `[a-z0-9_-]` (case-insensitive,
  stored lowercase), a built-in kind's name (its row already exists), or the name of an
  existing profile. After "Add", the options panel opens for the new profile right away,
  since `openai` can't work without `endpoint` and `model` (the ⚠ for missing required
  options does the rest).
- **Delete** removes the profile with all its options. Pickers on the General tab that
  selected it fall back to their axis's first built-in (`google`), and a note under the
  list says so ("translation now uses google"). Deleting and re-adding the same name in
  one session gives a fresh profile (old options gone).
- **Staged like everything else in Settings**: adding, deleting and option edits are kept
  in the dialog and written only on Settings OK; Cancel drops them all. The General
  pickers (and their ⚠ warnings) show added profiles and drop deleted ones immediately.
  On save the config is re-read and only the operations are applied (deletes, then adds,
  then option edits), so a hand-edit made while the dialog was open survives unless it
  touches the same profile.
- **Test** (on each row, and in the options panel, where it uses the panel's unsaved
  edits too): builds the profile exactly as the app would (saved options + this dialog's
  edits + `TAGENT_<NAME>_<KEY>` env overrides, through the `*_with` factories) and runs one
  call per axis the kind implements (from the registry):
  - translation: `"Hello, world!"` from `en` to the configured target language (`de` if
    that is `en`);
  - dictionary: `"hello"`, same pair;
  - speech: `speak_chunk("Hello", "en")`, reporting the audio size; nothing is played.
  One result line per axis: `translation: OK (0.9 s): Hallo, Welt!` or
  `translation: failed: <the error's Display>`. A profile missing a required option
  fails at construction with the factory's `InvalidOptions` message, before any network
  call. Runs off the UI thread (own Tokio runtime, as `spawn_translation` does); the
  button shows "Testing…" and is disabled meanwhile; the time limit is the transport's
  own budget (`timeout_secs`), no extra timer; a result arriving after the dialog closed
  is dropped (`Weak` upgrade fails). A test of a paid API costs a few characters/tokens.
- Not in this stage: renaming a profile or changing its kind (delete + add instead;
  renaming also changes its `TAGENT_<NAME>_*` variable names, easy to miss), fetching a
  model list from `/v1/models`, presets.

**Implementation steps.**
1. **`tagent`** (additive, goes into the unpublished 0.19.0 and its changelog section):
   `pub fn validate_profile_name(name: &str, kind: &str) -> Result<(), Error>` in
   `providers` (re-exported next to `env_var_name`): the checks `profile::resolve` does
   today (valid `[a-z0-9_-]+` name; a built-in kind's name only for that kind), which
   `resolve` then calls, so the messages stay identical. Doc comment with an example;
   tests. `tagent-gui`'s `tagent` dependency `version` stays `0.19.0`.
2. **`tagent-gui/src/provider_form.rs`** (pure, unit-tested):
   - `Draft { created: BTreeMap<String, String> /* name → kind */, deleted:
     BTreeSet<String>, edits: Edits }` replacing the dialog's bare `Edits`;
     `Draft::apply(&self, &mut ProviderProfiles)` (deleted profiles: every key removed;
     created: `type` inserted; then the edits, skipping deleted profiles) and
     `Draft::view(&self, saved) -> ProviderProfiles` (a clone with the draft applied),
     which `fields`/`missing_required`/the pickers then read with no edits of their own.
   - `name_error(view, name, kind) -> String` (`validate_profile_name` + duplicates +
     built-in names; `""` when fine).
   - `profile_rows(view) -> Vec<ProfileRow>` (built-ins first, then profiles, with
     `deletable`, `known_kind`).
   - `axes_of(kind) -> Vec<&'static str>` from the registry.
   - `picker_fallbacks(selected, view) -> ([String; 3], String)`: the new selections and
     the note.
   - `format_test_line(axis, result)`.
3. **`tagent-gui/ui/app.slint`**: the "Providers" `Tab` (a `ScrollView` list of
   `ProfileRow` structs, add form, note, test output `Text`), `ProfileRow` struct,
   properties `profile-rows`, `new-profile-kinds`, `new-profile-error`, `profile-note`,
   `test-output`, `testing`; callbacks `profile-add-requested(string, int)`,
   `profile-delete-requested(int)`, `profile-options-requested(int)`,
   `profile-test-requested(int)`, `panel-test-requested()`, `new-profile-name-edited(string)`
   (grep the component's callback names before adding: they share one namespace). A
   "Test" button and the output line in the options panel.
4. **`tagent-gui/src/main.rs`**: the dialog's `Rc<RefCell<Draft>>` instead of
   `Rc<RefCell<Edits>>`; one `refresh_profiles(dialog, saved, draft)` that refills the
   tab, the three pickers (`provider_choices` from `draft.view(saved)`, keeping the
   selection by name, falling back per `picker_fallbacks`) and the ⚠ warnings, called after
   every add/delete/panel OK; `show_provider_options` takes a profile name (rows and
   pickers both call it); the save closure applies the whole `Draft` to the freshly
   re-read profiles; `run_profile_test(choice, axes, target, weak)` on a background
   thread, result via `invoke_from_event_loop`.
5. **Version and docs**: `tagent-gui` `+BUILD` bumps (two iterations are fine: the tab
   with add/delete, then Test); `tagent-gui/CHANGELOG.md` (Added); `tagent/CHANGELOG.md`
   0.19.0 (Added: `validate_profile_name`); CLAUDE.md ("creating/deleting profiles is
   still hand-edit only" and the profile bullet), `docs/ARCHITECTURE.md` (tagent-gui
   Settings), `docs/providers-dev-plan.md` (Backlog item and Q3: done), this document.

**Tests and verification.**
- `tagent`: `validate_profile_name` (valid names, bad characters, a built-in name with
  its own kind / a foreign kind), `resolve` messages unchanged.
- `provider_form`: `Draft::apply`/`view` (add → `type` only; delete removes every key and
  the profile; delete + re-add = fresh; edits of a deleted profile ignored; untouched
  profiles and hand-edited keys kept), `name_error` (each rule), `profile_rows` (order,
  built-ins not deletable, unknown kind), `axes_of` (`google` → all three, `deepl` /
  `openai` → translation only), `picker_fallbacks`, `format_test_line`.
- `cargo test -p tagent -p tagent-gui`, `cargo clippy --workspace -- -D warnings`,
  `cargo doc -p tagent`, `cargo check --target x86_64-pc-windows-gnu -p tagent-gui`.
- UI by screenshot with an isolated `XDG_CONFIG_HOME` (mouse clicks only; typing the
  name is left to the user): the tab lists built-ins and profiles; Delete + OK removes
  the profile from the JSON and resets a picker that used it; Cancel restores it; Test on
  `google` shows three OK lines. User: Add an `openai` profile, fill `endpoint`/`model`
  in the panel, Test against a real server, OK, pick it in the main window and translate.

## Deliberately not done (revisit only with a new reason)

- **Partial text selection in the transcript** (e.g. a Shift-mode swap to a plain
  `TextInput`): it loses the colors while active, the click that swaps the mode is
  consumed, and Shift may not reach a `FocusScope` while the input has focus.
- **Configurable per-role colors** (part of speech, synonyms, notice, error): these stay
  derived automatically from the background, with WCAG contrast checking. Only the prompt
  color is configurable. `RoleColors` is where more would plug in.
- **A tray-icon re-registration workaround** for slint#13624: it needs a guessed delay,
  makes the icon flicker on every launch, and depends on Slint internals.
- **Recording the currently active hotkey** in "Record": the combination is grabbed
  system-wide before Settings sees it. Typing it manually is the workaround.
- **Monolingual or offline dictionaries**: online, bilingual backends only (decided
  2026-09-19).

## Lessons worth keeping (Slint and this codebase)

- **`show()` before `set_position()`**: a position set before `show()` is silently
  ignored (the popup landed at the top-left corner).
- **`slint::Timer` is `!Send`**: it can't cross into the hotkey path's `Send` callbacks.
  Put timing in a `Timer` *element* in `.slint` instead.
- **`slint` and `slint-build` must be bumped together**
  (`cargo update -p slint -p slint-build --precise X`): both pin an exact
  `i-slint-compiler`. Slint 1.17 also needs the system package `libfontconfig1-dev` on Linux.
- **`StandardButton { kind: cancel }` does nothing by itself**: `kind` only sets the label
  and position, so every button needs its own `clicked` handler.
- **An unbound `visible` on `SystemTrayIcon` is compiled as a constant**: changing it later
  panics ("Constant property being changed").
- **Theme changes go through a `.slint`-side function** (`apply-theme`), not
  `.global::<Palette>()` from Rust.
- **Features that share global state must be reasoned about together**: the live global
  hotkey fired during "Record" and its simulated Ctrl+C was captured as the recorded key.
  Fixed with a time-limited suppression window, not a latch, so a dialog closed mid-record
  can't disable the hotkey forever.
- **UI-thread ordering**: clear the "who is speaking" flag and the button state together
  inside `invoke_from_event_loop`, never one before the hop and one after, or a click in
  between hits a dead button.
- **Settings save rebuilds the whole `GuiConfig` from dialog fields**: a field without a
  dialog control must be carried through explicitly, or it resets on every save. Test it.
- **The whole config file is rewritten on save** (`tagent-gui.json` via serde, no
  `flatten`): unknown fields are dropped. Everything that should persist must be a
  `GuiConfig` field.
- **Every user- or provider-derived string passes through `escape_markdown`** before
  `StyledText`: a translation like `# 1. hello`, `> quote`, `a < b` or a four-space indent
  must render literally.
- **Callback names in `app.slint` are global to the component**: a reused name silently
  wires two features together. Grep before adding one.

## Testing boundary

- No `xdotool`-driven UI automation: simulated keypresses can leak into the developer's own
  terminal session. Pure logic is unit-tested; the UI is checked by screenshots and mouse
  clicks only; real keypress flows (hotkeys, Record, Esc) are verified manually by the
  maintainer.
- The tray can be driven over D-Bus (`busctl ... Activate`), and theme switches with
  `gsettings set org.gnome.desktop.interface color-scheme ...`.
- The Windows code is checked with `cargo check --target x86_64-pc-windows-gnu -p tagent-gui`
  and CI (Windows runner); live Windows behavior is verified manually.
