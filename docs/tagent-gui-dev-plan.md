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
7. **Global hotkeys on Wayland (GNOME 50)**: next, planned in detail
   [below](#planned-stage-w--global-hotkeys-on-wayland-gnome-50).

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

### Planned stage W — Global hotkeys on Wayland (GNOME 50)

**Status:** planned 2026-10-04, not started. Version: the first code change goes on
`0.15.0+NNN` (0.15.0 is unreleased; the latest tag is `v0.16.0` of `tagent-cli`).

**Why.** Ubuntu 26.04 (GNOME Shell 50.1, Mutter 50.1) has no Xorg session any more
(`/usr/share/xsessions/` is gone; only Wayland sessions are left), so the X11 assumptions
of Stages 4–6 no longer hold on the maintainer's machine:
- `XGrabKey` + `rdev` (`platform/linux/keyboard.rs`, `xgrab.rs`) see keys only while an
  XWayland window has focus. `Alt+A`/`Alt+S` do nothing from any native Wayland app, and
  nothing is logged (the grab itself succeeds, `DISPLAY=:0` is set).
- The simulated Ctrl+C (XTest, `clipboard.rs`) reaches only XWayland clients, so even a
  delivered hotkey would copy nothing from a Wayland app.
- The global Esc that stops speech rides the same `rdev` stream: gone too.
- Slint/winit now runs natively on Wayland (`WAYLAND_DISPLAY` is set; no `tagent-gui`
  window in `xwininfo -root -tree`), where a toplevel can't place itself, can't stay
  above other windows, and can't read the global cursor position. The popup's placement,
  drag, always-on-top and geometry restore depend on all three.

**What the platform offers** (checked on this machine, 2026-10-04):
- `org.freedesktop.portal.GlobalShortcuts`, interface version 1
  (`xdg-desktop-portal` 1.21.1, `xdg-desktop-portal-gnome` 50.0). Version 1 has
  `CreateSession`, `BindShortcuts`, `ListShortcuts` and the `Activated`/`Deactivated`/
  `ShortcutsChanged` signals; `ConfigureShortcuts` (version 2) is **not** available.
- `org.freedesktop.host.portal.Registry.Register(s app_id, a{sv})`, version 1. Since
  `xdg-desktop-portal` 1.20 a non-sandboxed app must call it before any other portal call
  on the same D-Bus connection; since 1.21.0 `GlobalShortcuts.CreateSession` rejects an
  empty app id ("An app id is required"). `xdg-desktop-portal-gnome` additionally
  discards a bind request whose app id isn't reverse-DNS **and** backed by an installed
  `.desktop` file ("invalid app_id"; `Register` itself fails with "App info not found"
  when the file is missing). Our app id today is `tagent-gui`: not reverse-DNS.
- On the first `BindShortcuts`, GNOME shows a consent dialog where the user accepts or
  changes the proposed triggers; the result persists per app id (gsettings
  `org.gnome.settings-daemon.global-shortcuts applications`, empty today) and can later be
  changed in GNOME Settings. The `preferred_trigger` we send is only a suggestion.
- No data-control protocol: the compositor advertises only `wl_data_device_manager` and
  `zwp_primary_selection_device_manager_v1`, so an unfocused native Wayland client can't
  read the selection. The XWayland bridge can: an unfocused `xclip -o -selection
  clipboard` returns exactly what `wl-paste -n` returns. Whether **PRIMARY** (the mouse
  selection) is mirrored as promptly is to be confirmed (step 0).

**Goal.** On GNOME Wayland: `translate_hotkey` and `speech_hotkey` work from any app,
translate/speak the current mouse selection, and the popup still appears on top at a
sensible place. Windows, macOS and X11 sessions keep today's behavior unchanged.

**Behavior.**
- **Session detection** (once, at startup, before anything touches `WAYLAND_DISPLAY`):
  Wayland when `XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` is set; cached in
  `platform::linux::session()` and used by the hotkey, selection and popup code.
- **Hotkeys on Wayland** go through the GlobalShortcuts portal, two shortcuts with ids
  `translate` and `speech` (the latter only when `enable_speech_hotkey` is on and it
  parses), descriptions "Translate the selection" / "Speak the selection", preferred
  triggers converted from `translate_hotkey`/`speech_hotkey` (`Alt+A` → `ALT+a`,
  `Ctrl+Shift+T` → `CTRL+SHIFT+t`, `Win+T` → `LOGO+t`, `F9` → `F9`; per the XDG shortcuts
  spec: `CTRL`/`ALT`/`SHIFT`/`LOGO`, `+`, an xkb keysym name). A double-press hotkey
  (`Ctrl+Ctrl`) has no trigger form: it isn't sent as a preference (GNOME's dialog then
  asks the user for one) and the log says so. `Activated` for `translate`/`speech` calls
  the same `on_translate_trigger`/`on_speech_trigger` as today.
- **Fallback chain on Wayland**: portal available and `Register` succeeds → portal. No
  GlobalShortcuts portal (wlroots compositors, older GNOME) → today's X11 path with a log
  line that hotkeys only work from XWayland windows. `Register` fails because the desktop
  entry is missing → no hotkeys, plus one transcript info row: `[Hotkey]: global hotkeys
  need the desktop entry: run "tagent-gui --install-desktop" once` (the `.deb` installs
  it system-wide, so packaged installs never see this).
- **Selection on Wayland**: the hotkeys read PRIMARY (what is selected with the mouse) via
  `arboard`'s `GetExtLinux::clipboard(LinuxClipboardKind::Primary)` over XWayland, no key
  simulation. Empty PRIMARY → the same "nothing selected" handling as today's empty
  clipboard. X11 sessions keep the XTest Ctrl+C path. The 📋 button is unaffected (it
  copies inside `tagent-gui`'s own window).
- **Stopping speech** on Wayland: Esc can't be observed globally (binding it through the
  portal would take Esc away from every other app). Instead, pressing the speech hotkey
  while something is speaking stops it; Esc still works while a `tagent-gui` window has
  focus. *Decision for the maintainer:* the toggle on every platform (one behavior, simpler
  to document) or on Wayland only. Proposed: every platform.
- **Windows on Wayland** (decided by spike 0b; leaning XWayland): the whole app runs on
  XWayland (winit's X11 backend), so the popup keeps positioning, always-on-top, drag and
  the desktop clamp, and the main window keeps its geometry restore. What can't come
  back: the global cursor position is stale while the pointer is over Wayland windows,
  so on Wayland the popup opens at the remembered position (`remember_popup_position`)
  or, without one, at a fixed spot (proposed: the top-right corner of the desktop, 16 px
  inset, via `virtual_screen_bounds`), never "next to the cursor". Focus restore after
  the popup hides is left to the compositor (`foreground_window()` sees only X11
  windows).
- **Settings > Hotkeys & Tray on Wayland**: the two hotkey fields show the trigger GNOME
  actually bound (`trigger_description` from `BindShortcuts`/`ListShortcuts`, updated on
  `ShortcutsChanged`), read-only, with a note: "Change in GNOME Settings > Apps >
  tagent-gui". "Record" and the text fields are disabled there: with portal v1 the value
  in `tagent-gui.json` only seeds the first bind. The enable checkboxes and the "Esc"
  note keep working. Unchanged on X11/Windows.
- **App id** (prerequisite): `tagent-gui` → a reverse-DNS id, proposed
  `io.github.holgertkey.TagentGui` (D-Bus naming: no hyphens in elements). It becomes the
  window's app id, the `.desktop` file name, `StartupWMClass` and the icon name. The
  binary, the config directory and the log stay `tagent-gui`. *Decision for the
  maintainer:* the exact id, and what happens to an already installed
  `~/.local/share/applications/tagent-gui.desktop`. Proposed, following the
  no-migration-shim preference: a pure rename, the changelog tells the user to run
  `--uninstall-desktop` with the old build (or delete the two files) and
  `--install-desktop` with the new one.

**Implementation steps.**

0. **Spikes** (throwaway code in `.debug/TESTS`, results recorded here before step 1):
   - **0a. PRIMARY over XWayland.** Select a word with the mouse (no Ctrl+C) in Firefox,
     GNOME Text Editor, Ptyxis/GNOME Terminal, a Chromium/Electron app and LibreOffice,
     then read it from an unfocused process with `xclip -o -selection primary` and with
     a 10-line `arboard` program using `LinuxClipboardKind::Primary`. Pass: the current
     selection, every time, within ~100 ms. If an app doesn't set PRIMARY, note it as a
     known gap. If the bridge doesn't carry PRIMARY at all, stop and rethink (fallback
     candidates, both worse: `wl-paste --primary`, which briefly maps its own surface to
     get focus; the RemoteDesktop portal to send Ctrl+C, which asks for permission on
     every session).
   - **0b. XWayland vs native for Slint.** Run `tagent-gui` both ways (native: today's
     default; XWayland: `WAYLAND_DISPLAY` removed before Slint starts) and compare: popup
     appears on top of a focused Wayland app without taking its focus; `set_position`
     honored; drag works; main window geometry restore; the tray; the theme follows
     GNOME; text sharp at the maintainer's scaling factor; Ctrl+V with the Russian
     layout. XWayland wins unless it is blurry or loses something native keeps.
   - **0c. Forcing X11.** Preferred: `std::env::remove_var("WAYLAND_DISPLAY")` at the top
     of `main()` in the process that runs Slint (after `detach.rs` re-spawned it, before
     any thread starts; edition 2021, so not `unsafe`), after `session()` has cached the
     session type. Rejected unless that fails: `BackendSelector::with_winit_event_loop_builder`
     (`unstable-winit-030`, an unstable API tied to winit 0.30), and building Slint with
     only `backend-winit-x11` (no way back to native at runtime). Child processes don't
     need the variable (none of them is a Wayland client).
   - **0d. Portal round trip.** A minimal `ashpd` program: `register_host_app`,
     `create_session`, `bind_shortcuts` with `ALT+a`, print `Activated`. Check: the
     consent dialog appears once, not on every launch; the shortcut fires from a native
     Wayland app; the key doesn't also reach that app; what happens when the dialog is
     declined; whether `Deactivated` matters to us (it shouldn't).
1. **App id rename** (`desktop_entry.rs`: `APP_ID`; `main.rs`: `set_xdg_app_id`;
   `assets/linux/`: the `.desktop` file renamed, `Icon=`/`StartupWMClass=`, the test that
   keeps it identical to the generated one; `Cargo.toml` `[package.metadata.deb]` asset
   paths; `release.yml`, which copies `tagent-gui.desktop` into the Linux archive). Its
   own `+BUILD`, verified with the dock icon on GNOME before going on.
2. **Session detection and X11 backend** (per 0b/0c): `platform/linux/mod.rs`
   `session() -> Session { X11, Wayland }` (pure core `session_from(xdg_session_type,
   wayland_display, display)`, tested), the env change in `main()`, the popup's Wayland
   placement (`popup_position::default_wayland_position(bounds, size)`, pure, tested).
3. **Portal hotkeys** in a new `platform/linux/portal.rs`, behind the unchanged
   `KeyboardHook::spawn(...)` signature, so `main.rs` changes only for the info row and
   the Settings fields:
   - `ashpd` as a Linux-only dependency (`[target.'cfg(target_os = "linux")'.dependencies]`,
     `ashpd = { version = "0.13", default-features = false, features = ["tokio",
     "global_shortcuts"] }`; `register_host_app` is in the crate root), so the Windows
     check (`cargo check --target x86_64-pc-windows-gnu -p tagent-gui`) is unaffected.
   - One thread with its own current-thread Tokio runtime (as `spawn_translation` does)
     that owns the portal proxy and the `Session` for the whole process (dropping either
     closes the session and the shortcuts with it): `register_host_app(APP_ID)` first,
     then `create_session`, `bind_shortcuts`, then loop over `receive_activated()` and
     `receive_shortcuts_changed()`.
   - Pure, tested: `to_portal_trigger(&HotkeyType) -> Option<String>` (keysym names from
     the existing keycode table: letters lowercase, digits, `F1`–`F12`, `space`,
     `Return`, `Tab`, `Escape`, arrows, ...; `None` for a double press), and the choice
     portal / X11 fallback / none from (session, portal present, register result).
   - The bound triggers go to the UI (a small `Mutex<BoundTriggers>` plus
     `invoke_from_event_loop` to refresh Settings if open).
4. **Selection source**: `ClipboardManager::get_selected_text()` (`get_text_with_copy` on
   X11, PRIMARY on Wayland); the two hotkey paths in `main.rs` call it. Windows/macOS get
   the same method name (same body as their `get_text_with_copy`), as the platform
   modules require.
5. **Speech toggle** (per the decision above): in `on_speech_trigger`, if speech is
   playing, `speech::request_stop` and return before reading the selection. The
   `is_speech_processing` guard stays.
6. **Settings > Hotkeys & Tray**: `hotkeys-from-desktop` (bool) and the two bound
   trigger strings on the dialog; the fields read-only with the note when it's set.
   Grep `app.slint` callback names before adding any.
7. **Docs** (same commits as the code): `tagent-gui/CHANGELOG.md` (Added: Wayland
   hotkeys; Changed: app id, popup placement on Wayland, speech hotkey stops speech);
   the user book (`gui/hotkeys-and-popup.md`, `gui/settings.md` Hotkeys & Tray,
   `getting-started/install.md` for `--install-desktop` and the consent dialog,
   `troubleshooting/platforms.md`, `reference/file-locations.md` for the renamed
   `.desktop`/icon); `docs/ARCHITECTURE.md` (platform table, "Linux desktop integration",
   the tagent-gui hotkey mechanics, known gaps); CLAUDE.md (the tagent-gui bullets on
   hotkeys, popup and desktop integration); this section condensed into a row of
   "Shipped stages".

**Not in this stage.**
- `tagent-cli` on Wayland: broken the same way (its hotkeys and auto-copy are X11 too).
  A follow-up for `tagent-cli-dev-plan.md` can reuse this design, but as duplicated code
  (Q3: platform code isn't shared), and it would need its own app id and desktop entry.
- `ConfigureShortcuts` (portal version 2): a "Change…" button that opens GNOME's dialog
  from Settings, once the host has version 2.
- wlroots compositors (Sway, Hyprland, niri): no GlobalShortcuts backend in
  `xdg-desktop-portal-wlr`; they get the X11 fallback only.
- KDE Plasma: its portal implements GlobalShortcuts too and should work through the same
  code, but isn't tested here.
- Popup next to the cursor on Wayland.

**Tests and verification.**
- Unit: `session_from` (each env combination), `to_portal_trigger` (every key class,
  modifiers order, `Win` → `LOGO`, double press → `None`), the backend choice,
  `default_wayland_position` (single and multi-monitor bounds, popup larger than the
  inset), the renamed desktop entry (generated = shipped file).
- `cargo test -p tagent-gui`, `cargo clippy --workspace -- -D warnings`,
  `cargo check --target x86_64-pc-windows-gnu -p tagent-gui`.
- Manual (maintainer; no `xdotool`, see "Testing boundary"): with a dev build the portal
  needs an installed desktop entry, so first `cargo run -p tagent-gui -- --install-desktop`
  (it points `Exec=` at the debug binary; reinstall the release one afterwards). Then:
  the consent dialog on the first start only; `Alt+A`/`Alt+S` from Firefox, Text Editor
  and the terminal; the popup on top, at the fixed spot and at a remembered position;
  the speech hotkey stops speech; Settings shows the bound triggers; after changing one in
  GNOME Settings, Settings shows the new one (`ShortcutsChanged`) and it works; without the
  desktop entry the info row appears; Windows build unchanged (CI).

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
