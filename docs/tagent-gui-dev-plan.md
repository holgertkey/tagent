# tagent-gui Development Plan

Living document: the concept, standing decisions and roadmap for `tagent-gui`. Update it
when a decision is made or a stage lands; this document *is* the current state of the
plan.

Where the rest lives:
- **How things work** (mechanics, platform details, known gaps):
  [`docs/ARCHITECTURE.md`](ARCHITECTURE.md), section "tagent-gui: Slint desktop GUI".
- **What changed and when**: [`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md).
- **The provider architecture** (shared with `tagent-cli`): [`providers-dev-plan.md`](providers-dev-plan.md).
- **The user book** (`docs/user`): every stage that changes what users see lists its
  "Book" edits next to its changelog entry and makes them in the same commit (the GUI
  pages, Settings tab by tab, and `reference/tagent-gui-json.md`). The README is a short
  introduction now and rarely changes. See "User documentation" in
  [`ARCHITECTURE.md`](ARCHITECTURE.md).

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
- **Settings dialog**: tabs General / Providers / View / Popup / Hotkeys & Tray; hotkey "Record" button
  with live validation; Reset to Defaults; themes and color schemes; prompt colors.
- **Providers**: translation, dictionary and speech pickers in Settings > Providers (plus a
  session-only translation picker in the main window), user profiles from
  `provider_options`, and an "Options…" panel per profile built from `tagent`'s registry
  (password fields for secrets, ⚠ for missing required options, a multi-line editor with
  the built-in default for prompts). The "Providers" tab also creates, deletes and tests
  profiles (0.14.0+037) and hides entries from the pickers ("Show in lists", 0.14.0+038). Google, DeepL and OpenAI-compatible translation.
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
| W | Wayland hotkeys (GNOME 50) | 2026-10-04 | GlobalShortcuts portal (`ashpd`, `async-io`), app id `io.github.holgertkey.TagentGui`, PRIMARY over XWayland, windows on XWayland, popup in a corner, speech hotkey stops speech (0.15.0+006) |
| — | Single instance | 2026-10-04 | a second start shows the running copy over a per-user local socket (`interprocess`), stale socket reclaimed, a silent holder reported (0.15.0+009) |

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
   0.14.0+036). Creating and deleting profiles, plus a "Test" button: implemented in
   0.14.0+037, see [below](#planned-stage--provider-profiles-tab).
4. ~~**Multi-line provider options.**~~ Done in 0.14.0+036 (2026-10-01): a `multiline`
   option (today `openai`'s `translate_prompt`) gets a `TextEdit` pre-filled with its
   `default`, "Reset to default", and a soft ⚠ when `{to}` is missing; driven only by the
   `OptionSpec`, so P3's `dictionary_prompt` needs no GUI change. See the changelog and
   "tagent-gui: Slint desktop GUI" in `docs/ARCHITECTURE.md`.
5. ~~**Provider menu for all three axes.**~~ Done in 0.15.0+004 (2026-10-03, Stage U of
   [`providers-dev-plan.md`](providers-dev-plan.md)): the ComboBox next to ⚙ became a
   menu with a Translation, a Dictionary and a Speech section; picks are session-only
   per axis. The same sections in the tray menu are a possible follow-up, once the menu
   proves itself (and slint#13624 is fixed).
6. **Slint upgrade** once [slint-ui/slint#13624](https://github.com/slint-ui/slint/issues/13624)
   (empty tray menu after a slow start) is fixed upstream. Bump `slint` and `slint-build`
   together and drop the known-gap entry.
7. ~~**Global hotkeys on Wayland (GNOME 50).**~~ Done in 0.15.0+006 (2026-10-04, Stage W):
   see "Wayland: hotkeys through the portal" in `docs/ARCHITECTURE.md`. Follow-ups, not
   scheduled: `tagent-cli` on Wayland (broken the same way; same design, duplicated per
   Q3, with its own app id and desktop entry); a "Change…" button through
   `ConfigureShortcuts` once the host portal has version 2; the popup's corner on the
   primary monitor rather than the bounding box. Not planned: wlroots compositors (no
   GlobalShortcuts backend), the popup next to the cursor on Wayland.
8. ~~**Single instance.**~~ Done in 0.15.0+009 (2026-10-04): see "Single instance" in
   `docs/ARCHITECTURE.md`. Follow-ups, not scheduled: `--replace` (quit the running copy
   and start); passing text to translate (`tagent-gui "text"`); raising a visible window
   that is behind others; a D-Bus `Activate` (`DBusActivatable=true`) for GNOME.
9. **Speak from the popup.** Implemented in 0.15.0+012 (2026-10-05), manual check open; see
   [below](#planned-stage--speak-from-the-popup).

### Planned stage — Provider profiles tab

**Status:** implemented in 0.14.0+037 (2026-10-01), both iterations at once (the tab with
add/delete, and Test); the manual UI check below is still open. Once it passes, condense
this section to a row of the "Shipped stages" table and a changelog entry, like the other
stages.

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

### Planned stage — Pickers on the Providers tab, "Show in lists"

**Status:** implemented in 0.14.0+038 (2026-10-01), on top of the Providers tab above;
the manual UI check is open, together with that stage's.

**Goal.** One place for everything about providers, and short pickers once there are many
profiles.

**Behavior.**
- The translation, dictionary and speech pickers move from General to the top of the
  Providers tab; General keeps the default languages and the rest. The pickers' "Options…"
  buttons go away: each row of the list has its own. A row whose profile lacks a required
  option shows the ⚠ too, so the fix is next to the warning. The note after a delete
  ("translation now uses google") now sits right under the pickers it talks about.
- The main window's ⚠ (and only it, not ⚙) opens Settings on the Providers tab.
- **"Show in lists"**: a checkbox on each row of a known kind, built-ins included. Unchecked
  hides the entry from the three pickers and the main window's picker; it is not
  "disabled": a hidden profile still works wherever it's selected, and "Options…"/"Test"
  work as before. The Providers tab always lists everything, so a hidden entry can come
  back.
  - The entry a picker currently selects stays in that picker even when hidden (the main
    window: the provider in effect, including a session pick), so no selection vanishes.
  - The first built-in of each axis (`google`, the fallback after a delete) can't be
    hidden: its checkbox is disabled, so every picker has an entry.
  - Stored as `hidden_providers` (a list of names, lowercase) in `tagent-gui.json`, not in
    `provider_options`: it's a GUI preference, and an `enabled` key there would reach
    `tagent` as a provider option. Unknown names are kept; a deleted profile's name is
    dropped on save. Staged in the dialog like everything else (OK writes, Cancel drops;
    the pickers follow at once). "Reset to Defaults" keeps it, like the profiles.

**Implementation.**
- `config.rs`: `hidden_providers: Vec<String>` (`#[serde(default)]`, lowercased on load).
- `provider_form.rs` (pure, tested): `picker_entries(kinds, profiles, hidden, keep)` (built-ins
  then profiles of the axis, minus hidden ones except `keep` and the axis's first
  built-in), `can_hide(name)`, `ProfileRow` gains `shown`/`hideable`; `sharing_note` and
  the per-picker panel path go.
- `app.slint`: the picker grid moves into the Providers tab; `ProviderProfileRow` gains
  `shown`, `hideable`, `warning`; callback `profile-shown-changed(int, bool)`; a
  `current-tab` property bound to the `TabWidget`; on `AppWindow` a
  `provider-warning-clicked` callback for the ⚠.
- `main.rs`: the dialog's hidden set next to the `Draft`, used by `refresh_profiles`; the
  save writes it (minus names that no longer exist); `refresh_translate_provider_picker`
  filters with `config.hidden_providers`.

**Follow-up (0.14.0+039, decided 2026-10-01):** "Test" stays only in the options panel;
the rows lose theirs. A test is what you run while editing options, and the panel's Test
already uses the unsaved values; the rows get narrower and the result shows next to the
options it is about.

**Tests.** `picker_entries` (hidden dropped, selection kept, first built-in kept, profiles
of other axes absent), `can_hide`, the new `ProfileRow` fields, `hidden_providers`
round-trip and lowercasing, saving drops deleted names.

### Planned stage — Speak from the popup

**Status:** implemented in `0.15.0+012` (2026-10-05); the manual check below is open.
Once it passes, condense this section to a row of the "Shipped stages" table.
Deviations from the steps below: step 4 got a simpler `format_popup_line` (the
`format_line` string with `SPEAKER_PREFIX` in the prompt; a test checks it against the
stripped template); `styled::render_template` (2 arguments) was folded into
`render_template_with_speaker`, its last caller being the popup; and
`PromptSpeakButton` got a `right-clicked` callback, since its `TouchArea` swallowed the
right button over the prompt (the popup routes it to copy; the transcript's prompts have
the same gap, not fixed here).

**Goal.** The popup's `[Lang]:` prompts become speak buttons, exactly like the
transcript's since 0.14.0+041: `[🔊 English]:` before the phrase speaks it in the source
language (detected if `Auto`), `[🔊 Russian]:` before the translation speaks the
translation. Today hearing a hotkey translation means opening the main window from the
tray and finding the row.

**Behavior (decided).**
- **Only the prompt is clickable**, as in the transcript; not the whole line. A
  `PromptSpeakButton` (the existing component) is laid over the prompt; hover tint,
  stronger tint while speaking, a click while it speaks stops it.
- With the popup's prompt off (Settings > Popup "Show prompt"), a line starts with just
  `🔊`, as a transcript block does.
- **No speaker** (the prompt stays plain, not clickable) when `enable_text_to_speech` is
  off, on the translation line of an error, and when there is nothing to speak. A
  dictionary hit speaks only the primary translation (the entry's `translation_speech`,
  same as the transcript). No new setting: `enable_text_to_speech` governs both windows.
- **One playback app-wide, shared with the transcript**: the popup does not get its own
  speech path. A click goes to the transcript row the popup shows (the hotkey pushes the
  same translation into the transcript) through `AppWindow`'s existing
  `speak-requested(index, is_phrase)`. So everything the transcript has applies as is:
  a second click stops, other speakers are disabled while one plays (in both windows),
  Esc (X11/Windows) and the speech hotkey stop it, `"auto"` is resolved lazily, the
  provider in effect (session pick included) is used. A playback started in the
  transcript for the same row shows as speaking in the popup too, and vice versa.
- **The popup stays while its own row speaks**: the auto-hide doesn't close it then, and
  once that playback ends (finished, stopped, or failed) the full
  `popup_auto_hide_seconds` countdown starts again, so the popup doesn't vanish the
  instant the audio stops. Speech of *another* row (a transcript button, the speech
  hotkey) doesn't hold the popup.
- **A new hotkey translation while the popup's row still speaks**: the popup shows the
  new pair at once (as today); the old playback keeps going, the new speakers are
  disabled until it ends (the transcript's rule), and the popup hides on its normal
  timer, since its new row isn't the one speaking.
- **Dragging**: a left press on the speaker no longer starts a drag (the button takes
  it); everywhere else on the popup drag works as before. Accepted (same trade-off as the
  transcript's click target), no click-vs-drag threshold.
- **Right-click copy** is unchanged, also over the prompt (the button reacts only to the
  left button; check that the right button still reaches the block's `TouchArea`).
- **Focus**: clicking the popup may focus it on X11; the focus goes back to the source
  app when the popup hides (`POPUP_RESTORE_TARGET`), as after a drag today. Check live.
- Not in this stage: a 🔊 for the speech hotkey (it shows no popup), a "replay" key, a
  click-anywhere-on-the-line target.

**Implementation steps.**
1. **Which row the popup shows.** `spawn_translation` calls `on_done` right before
   `push_transcript_entry` on the UI thread, and transcript rows are never removed or
   reordered (`push_transcript_entry` only appends), so the hotkey's `on_done` reads the
   row's index as `window.get_transcript_entries().row_count()` at that moment. Pass a
   `weak` window into the closure (it has `popup_weak2` today) and hand the index to
   `show_popup`. Write that invariant down in `push_transcript_entry`'s doc comment, so a
   future "clear transcript" / row cap knows the popup depends on it (it would then reset
   the popup's index to -1).
2. **`show_popup` arguments**: group what it needs into a `PopupContent` struct (it is at
   6 arguments, the new ones would pass clippy's 7): `entry_index: i32`,
   `phrase_speaker: bool` (`tts && !entry.phrase_speech.is_empty()`),
   `translation_speaker: bool` (`tts && !is_error && !entry.translation_speech.is_empty()`),
   computed in `on_done` from the `&TranscriptEntry` it already receives (rename
   `_entry`) and `window.get_tts_enabled()`.
3. **Templates with the speaker marker**: `popup_templates(outcome, show_prompt,
   phrase_speaker, translation_speaker)` passes the flags on to
   `styled::phrase_template`/`translation_template_from_body` (today hard-coded
   `false`). `restyle_popup` renders with `render_template_with_speaker`, using the
   popup's `phrase-speaker`/`translation-speaker` properties, so a theme change keeps the
   glyph. The flags are decided when the popup is shown; a TTS toggle while it is visible
   only takes effect on the next popup (it lives seconds; not worth a live path).
4. **Width measurement**: `phrase-measure`/`translation-measure` measure `phrase-text`/
   `translation-text` (plain `format_line` strings without the glyph), so the popup would
   come out a glyph too narrow and wrap one line early. Build these plain strings from the
   same rendered template instead (`styled::strip_template` of the template with the
   marker substituted; a small pure helper, e.g. `styled::plain_with_speaker(template,
   speaker)`), so text and measurement can't drift. Unit-test it against `format_line`
   (no speaker: identical; speaker: `[🔊\u{a0}English]: hello` / `🔊\u{a0}hello`).
5. **`app.slint`, `TranslationPopup`**:
   - properties `in entry-index: int` (-1 = no row, no speakers), `in phrase-speaker`,
     `in translation-speaker: bool`, `in phrase-prompt`, `in translation-prompt: string`
     (the language name or "", as on `TranscriptEntry`, for the probe), and the mirrored
     speech state `in speaking-entry-index: int` (-1), `in speaking-is-phrase: bool`;
   - a derived `property <bool> own-row-speaking: entry-index != -1 &&
     speaking-entry-index == entry-index`;
   - in each block a `*-probe` `Text` (copied from the transcript: same string shape as
     `SPEAKER_PREFIX`, `popup-font`/`popup-size`) and, declared after the block's
     `TouchArea` so its left click wins, `if phrase-speaker: PromptSpeakButton { … }`
     with `accent: popup-prompt-accent`, `speaking`/`enabled` as in the transcript but
     against the mirrored properties, `clicked => { root.start-hide-timer();
     popup-speak-requested(true); }`;
   - `callback popup-speak-requested(bool)` (grep: the name must be new in the file);
   - `hide-timer`: `own-row-speaking` joins `has-hover || pressed || drag-pressed` as
     "keep open";
   - `changed own-row-speaking => { if !own-row-speaking { root.start-hide-timer(); } }`
     restarts the full countdown when the popup's playback ends (`changed` handlers are
     available in slint 1.17, `app.slint` already uses them).
6. **Mirroring the speech state into the popup**: `AppWindow` gets
   `callback speaking-state-changed()` and `changed speaking-entry-index => {
   speaking-state-changed(); }`; `main()` wires it to copy
   `speaking-entry-index`/`speaking-is-phrase` onto the popup. This keeps
   `start_speaking` and its end-of-playback hop unchanged (`speaking-is-phrase` is set
   before the index, so the handler sees both). The same values are set in `show_popup`
   too, for a popup shown mid-playback.
7. **The click**: `wire_popup_speak(popup, window_weak)` next to
   `wire_popup_copy`: `popup-speak-requested(is_phrase)` → if `entry-index != -1`,
   `window.invoke_speak_requested(entry_index, is_phrase)`. `on_speak_requested` already
   handles stop/ignore/empty text and re-reads the row, so the popup duplicates none of
   it. Before invoking, re-read `enable_text_to_speech` (`check_and_reload`) and ignore
   the click if it was turned off meanwhile, the way the speech hotkey refreshes
   `tts_enabled`.
8. **Version and docs** (one commit): `tagent-gui` `0.15.0+011` → `+012`;
   `tagent-gui/CHANGELOG.md` 0.15.0 "Added" `(+012)`; the user book
   `docs/user/src/gui/hotkeys-and-popup.md` ("The popup": the prompts speak, the popup
   stays while speaking) and a cross-reference in `gui/main-window.md`'s speak-button
   bullet; `docs/user/src/gui/settings.md` ("Enable text-to-speech" covers the popup too);
   `docs/ARCHITECTURE.md` ("tagent-gui: Slint desktop GUI", popup and speech parts: the
   row-index link and the mirrored state); CLAUDE.md's popup bullet ("purely
   informational, no buttons" is no longer true); this section → "Shipped stages" row.

**Tests and verification.**
- Unit (pure): `popup_templates` with speaker flags (marker present only where speakable:
  not on an error, not with TTS off; prompt on/off shapes), the plain-text helper of
  step 4, and the existing `popup_templates_follow_popup_show_prompt` extended rather
  than duplicated.
- A Slint-level test in the style of the existing window tests in `main.rs` (they build
  an `AppWindow` without showing it), if `TranslationPopup` can be built the same way:
  `own-row-speaking` follows `entry-index`/`speaking-entry-index`; otherwise this is
  covered by the manual check.
- `cargo test -p tagent-gui`, `cargo clippy --workspace -- -D warnings`,
  `cargo check --target x86_64-pc-windows-gnu -p tagent-gui`.
- Manual (maintainer, real hotkey; X11 and Wayland): Alt+A on a sentence → click
  `[🔊 Lang]:` on either line → it speaks, the popup stays to the end and hides
  `popup_auto_hide_seconds` later; a second click stops; Esc (X11) and the speech hotkey
  stop; the transcript row shows the same speaking state and its other buttons are
  disabled; a single word (dictionary) speaks only the primary translation; an error
  line has no speaker; TTS off → plain prompts; prompt off → bare 🔊; drag by the text
  still works, right-click copy still works over the prompt; focus returns to the
  source app after the popup hides; a long prompt line isn't wrapped one glyph early.

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
