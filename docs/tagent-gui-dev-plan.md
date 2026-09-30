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

## Current state (`0.14.0+019`, 2026-09-25)

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
   [`providers-dev-plan.md`](providers-dev-plan.md) Stage F and its Backlog.
4. **Multi-line provider options** (decided 2026-09-30, after `tagent`'s Stage P2 in
   [`providers-dev-plan.md`](providers-dev-plan.md)). The "Options…" panel
   (`provider_form.rs`) renders an option with `OptionSpec::multiline` (today the
   `openai` kind's `translate_prompt`, later P3's `dictionary_prompt`) as a multi-line
   `TextEdit` instead of a `LineEdit`, pre-filled from `OptionSpec::default` when the
   profile has no value, with a "Reset to default" button that clears the value (empty =
   the built-in default, as for every option). A soft ⚠ when a `translate_prompt` lacks
   `{to}` (accepted by the library, but the model then doesn't learn the target language).
   Saving a value equal to the default stores nothing. Until then the option works as a
   single-line field. Detailed plan: [below](#planned-stage--multi-line-provider-options).
5. **Slint upgrade** once [slint-ui/slint#13624](https://github.com/slint-ui/slint/issues/13624)
   (empty tray menu after a slow start) is fixed upstream. Bump `slint` and `slint-build`
   together and drop the known-gap entry.

### Planned stage — Multi-line provider options

**Status:** planned (2026-09-30). Once shipped, condense this section to a row of the
"Shipped stages" table and a changelog entry, like the other stages.

**Goal.** An option whose `OptionSpec::multiline` is `true` (today only the `openai`
kind's `translate_prompt`; P3 adds `dictionary_prompt`) gets a real multi-line editor
in the "Options…" panel, starting from the built-in text, with a way back to it. Still
**no provider-specific GUI code**: everything is driven by the `OptionSpec` fields
`tagent` already has (`default`, `multiline`), so P3's prompt works without a GUI change.

**Behavior.**
- A multiline option renders as a `TextEdit` (`wrap: word-wrap`, fixed height of about
  8 lines) under its label, instead of the label + `LineEdit` row. Description and the
  env-override note stay below it as for every field.
- With no value (no edit, nothing saved), the editor shows `OptionSpec::default`
  (pre-filled, not a placeholder, so it can be edited in place). The pre-fill is
  **display only**: the field's `value` stays empty and nothing is stored unless the user
  edits.
- A "Reset to default" button under the editor (only for options with a `default`) puts
  the default text back and records the edit as empty, which removes the key on save
  (empty = the built-in default, as for every option).
- An edit whose text equals the default (after CRLF → LF and trimming both sides) is
  recorded as empty, so saving it stores nothing. A value that was already saved equal to
  the default (hand-edited JSON) is left as it is unless edited; deliberate, to keep the
  panel from rewriting what the user didn't touch.
- Soft ⚠ under the editor, live while typing and also when the panel opens: "The prompt
  has no `{to}`: the model won't be told the target language." Never blocks OK; the
  library accepts such a prompt (the language may be written into it).
- `secret` wins over `multiline` (`TextEdit` has no password mode): such an option stays a
  masked `LineEdit`. No such option exists; the rule just fixes the precedence.
- Optional, cheap: a single-line option with a `default` shows it as the `LineEdit`'s
  placeholder instead of "default". None exist today.

**Decision to make first: where the `{to}` rule lives.** CLAUDE.md promises that a
provider's options appear in the panel "with no GUI code", so the GUI must not know the
key `translate_prompt`. Options:
- **(a) Derived from `OptionSpec::default` (recommended, no API change):** for each
  placeholder `{from}` / `{to}` that the default contains, the rule is "warn if the
  effective value (empty = the default) lacks it". Only `{to}` gets a ⚠ (a missing
  `{from}` is harmless: the model sees the text). P3's `dictionary_prompt` is covered
  automatically if its default uses `{to}`.
- (b) An additive `OptionSpec` field in `tagent` (e.g. `placeholders: &[&str]`, required
  ones flagged). Explicit, but a public API addition for one hint; would go into `tagent`
  0.19.0 (still unpublished: the last release `v0.16.0` shipped `tagent` 0.18.1), with a
  `tagent/CHANGELOG.md` entry and no version bump.

**Implementation steps.**
1. **`tagent-gui/src/provider_form.rs`** (pure, no Slint types):
   - `Field` gains `multiline: bool` (`spec.multiline && !spec.secret`) and
     `default: String` (`spec.default.unwrap_or("")`). `fields()` fills them; `value`
     keeps meaning "edit, else saved value", never the default, so `missing_required`
     and `warning` are unaffected.
   - `pub fn normalize_edit(default: &str, value: &str) -> String`: CRLF → LF; if the
     trimmed result equals the trimmed default (and the default isn't empty), return
     `""`, else the value unchanged (`apply` already trims on save, as the library does).
   - `pub fn soft_warning(default: &str, value: &str) -> String`: rule (a) above, `""`
     when nothing to say. Empty/blank value → checks the default → never warns.
2. **`tagent-gui/ui/app.slint`**:
   - `ProviderOptionField` gains `multiline: bool` and `default-value: string` (not
     `default`, to stay clear of any keyword).
   - New `in-out property <[string]> provider-option-warnings` (indexed by row), next to
     `provider-option-fields`. Warnings **must not** go through the fields model: replacing
     or changing that model on a keystroke recreates/refreshes the repeater rows and the
     editor loses its cursor and focus.
   - Import `TextEdit`. In the field loop: `if !field.multiline:` the current row;
     `if field.multiline:` label, `prompt-editor := TextEdit { text: field.value != "" ?
     field.value : field.default-value; ... edited(value) => { provider-option-edited(row,
     value); } }`, the warning `Text` (bound to `provider-option-warnings[row]`), and
     "Reset to default" (`visible: field.default-value != ""`). Reset assigns
     `prompt-editor.text = field.default-value;` **in Slint** and then calls
     `provider-option-edited(row, "")`: typing has already broken the `text:` binding,
     so re-sending the model row wouldn't update the editor.
   - No new callback: Reset reuses `provider-option-edited` (callback names are global
     to the component; grep before adding any).
   - Check the editor inside the panel's outer `ScrollView`: fixed height so the layout
     doesn't collapse or grow unbounded; wheel scrolling over it scrolls the editor, the
     rest of the panel still scrolls elsewhere.
3. **`tagent-gui/src/main.rs`**:
   - `show_provider_options`: copy `multiline` / `default` into `ProviderOptionField`,
     and fill `provider-option-warnings` with `soft_warning(default, value)` per row
     (empty for single-line fields).
   - `on_provider_option_edited`: store `normalize_edit(&field.default_value, value)`
     in `panel_edits`, then `set_row_data` on the warnings model for that row only.
     (Row data of `provider-option-fields` stays untouched.)
4. **Version and docs** (implementation work, so `+BUILD` only):
   - `tagent-gui/Cargo.toml` `0.14.0+035` → `+036`; `tagent-gui/CHANGELOG.md` entry
     (Added); no README change (no semver bump).
   - Replace "single-line field for now" in: CLAUDE.md (OpenAI provider section),
     `docs/ARCHITECTURE.md` (~l. 212-213; plus the Options panel mechanics in the
     tagent-gui section), `docs/providers-dev-plan.md` (P2's `tagent-gui` notes,
     ~l. 1294, 1392, 1489).
   - This document: Roadmap item 4 struck through, a "Shipped stages" row, "Current
     state" refreshed, this section condensed.
   - Only with option (b): `tagent/src/providers/registry.rs` + tests,
     `tagent/CHANGELOG.md` (0.19.0), `cargo doc -p tagent`.

**Tests and verification.**
- Unit tests in `provider_form.rs`: `openai`'s `translate_prompt` field has
  `multiline` and the `DEFAULT_TRANSLATE_PROMPT` default, its `value` stays empty;
  `google`'s fields have neither; `normalize_edit` (equal modulo trim/CRLF → `""`,
  different → kept, empty → `""`, empty default → never collapses); `soft_warning`
  (no `{to}` → ⚠, with `{to}` → none, blank → none, default without placeholders →
  none); secret-over-multiline via a constructed case if the registry has none.
- `cargo test -p tagent-gui`, `cargo clippy --workspace -- -D warnings`,
  `cargo check --target x86_64-pc-windows-gnu -p tagent-gui`.
- UI by screenshot only (testing boundary): panel with an `openai` profile, empty and
  with a saved prompt; typing a prompt without `{to}` shows the ⚠; Reset brings the
  default back; OK + Settings OK with an unchanged or reset prompt writes no
  `translate_prompt` into `tagent-gui.json`; a custom prompt is saved and used by the
  next translation (live-reloaded); Cancel of the panel drops the edit.
- Note: a saved multi-line prompt is a `\n`-escaped JSON string in `tagent-gui.json`:
  correct, but awkward to hand-edit. The panel is the intended way to edit it.

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
