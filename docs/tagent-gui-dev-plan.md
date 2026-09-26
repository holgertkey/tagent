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
  `tagent-cli.conf` (no migration path from the old inline reader). The file is plain
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
  There's a hardcoded 6-language list, and "Auto" is offered only as a source language.
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

1. **Stage 2 — Language list expansion.** The 6-language list is `tagent-gui`'s own call
   (not tied to `tagent-cli`'s ~16). Low effort, self-contained.
2. **History logging.** A candidate, not prioritized; no design yet.
3. **Linux desktop integration.** A `.desktop` file plus `slint::set_xdg_app_id(...)`, so
   GNOME's dock shows the app icon (it matches icons through an installed `.desktop` file,
   not `_NET_WM_ICON`). This is packaging/installation work; `tagent-cli` has the same gap.
4. **Provider options in Settings.** Keys, endpoints and user profiles, following
   [`providers-dev-plan.md`](providers-dev-plan.md) Stage F and its Backlog.
5. **Slint upgrade** once [slint-ui/slint#13624](https://github.com/slint-ui/slint/issues/13624)
   (empty tray menu after a slow start) is fixed upstream. Bump `slint` and `slint-build`
   together and drop the known-gap entry.

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
