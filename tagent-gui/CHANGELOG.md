# Changelog

All notable changes to `tagent-gui` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent-gui` versions independently of `tagent-cli` (see the root
[CHANGELOG.md](../CHANGELOG.md) for that application's history) and independently of
the `tagent` library. See [`tagent-gui development plan.md`](../.debug/tagent-gui%20development%20plan.md)
for the roadmap and design decisions behind this project.

## [Unreleased]

## [0.14.0+014] - 2026-09-13

### Added
- Clipboard support (Stage 4 of the development plan): a "📋" button next to
  Translate pulls the current system clipboard contents into the input box.
  Backed by a new per-OS `ClipboardManager` in `tagent-gui/src/platform/`
  (Linux: `arboard` + `xdotool`; Windows: `clipboard-win` + `SendInput`; macOS:
  stub, not yet implemented), ported independently from `tagent-cli`'s own
  `ClipboardManager` — no dependency on `tagent-cli` introduced. Runs off the UI
  thread so the window doesn't freeze during the underlying copy.

### Known limitation
- Because clicking "📋" gives `tagent-gui` window focus first, its simulated
  Ctrl+C targets `tagent-gui` itself rather than whatever was focused right
  before the click — unlike a future global hotkey, which wouldn't need to
  steal focus to fire. In practice this makes the button behave as "paste
  whatever the clipboard already holds," not "grab the current selection with
  no prior manual copy." See `docs/ARCHITECTURE.md` for the full explanation.
  Expected to be superseded once Stage 5 (global hotkeys) lands.

## [0.14.0+013] - 2026-09-12

### Added
- Seven more entries in the "Color scheme" preset picker (Settings > View):
  Tokyo Night, Catppuccin Mocha, Night Owl, Ayu Dark (dark), and GitHub
  Light, Gruvbox Light, Catppuccin Latte (light) — 14 presets total now.

### Fixed
- Reopening Settings always showed the "Presets…" placeholder in the "Color
  scheme" dropdown, even right after applying and saving one of the presets
  — each Settings dialog is a fresh instance, so its `color-scheme-index`
  never carried over from the previous session. It now shows the matching
  preset's name instead, whenever the five colors currently in effect
  (`background_color`/`phrase_*`/`translation_*` in `tagent-gui.json`)
  exactly equal one of the presets.

## [0.14.0+012] - 2026-09-12

### Added
- "Color scheme" preset picker in Settings > View: seven popular schemes
  (Solarized Dark, Solarized Light, Dracula, Nord, Gruvbox Dark, Monokai, One
  Dark). Picking one immediately fills in Background color, phrase text/
  background, and translation text/background to that scheme's colors and
  switches Theme to match (dark or light), so the window chrome and the
  transcript stay consistent. It's a one-shot bulk-fill, not a persisted
  setting of its own — the five color fields it sets remain individually
  editable afterward, same as if set by hand.

## [0.14.0+011] - 2026-09-12

### Added
- "Background color" setting in Settings > View: sets the shared background
  for both main panels (the transcript log and the input box), via the same
  "theme default or pick a color" field used for the phrase/translation
  colors. Persisted as `background_color` in `tagent-gui.json` (empty =
  follow the theme). Phrase/translation backgrounds left at "theme default"
  now follow this color too (custom or theme), so a custom app background
  and per-line "theme default" backgrounds always stay visually consistent
  instead of the latter secretly meaning "the raw, un-customized theme
  color".

### Fixed
- The input bar's own frame (behind the "[Auto]:" language prompt, the
  typing field, and the Translate button) shared the same `panel-background`
  property as the transcript log, so a custom "Background color" bled into
  the gaps around those controls and looked like they'd been recolored too.
  That frame now always uses the raw theme color instead; only the
  transcript log's reading area picks up the custom background.

## [0.14.0+010] - 2026-09-12

### Changed
- Reworked "Show prompt" (Settings > View, phrase/translation) back down to
  just a visibility toggle, dropping the separate prompt size/color settings
  added in +009. Slint's plain `Text`/`TextInput` can't mix two styles within
  one wrapped paragraph, so giving the prompt its own size/color required
  splitting it into a separately positioned element next to the text — which
  then either hanging-indented wrapped continuation lines under the text
  (rather than wrapping flush from the row's left margin like a normal
  paragraph) or, when top-aligned to fix a vertical mismatch at differing
  sizes, still didn't read as "one line" the way a single-style line does.
  The prompt is back to being part of the same string as the text (one
  font/color for the whole line, prompt included), toggled on/off by baking
  it into the string when the entry is created rather than rendered as its
  own styled element. `phrase_prompt_size`/`phrase_prompt_color` and the
  matching `translation_*` fields are gone from `tagent-gui.json`;
  `phrase_show_prompt`/`translation_show_prompt` remain.
- `TranscriptEntry` is back to one `phrase`/`translation` string per line
  (was briefly split into separate prompt/text fields in +009).

## [0.14.0+009] - 2026-09-12

### Added
- "Show prompt" setting in Settings > View, separately for the phrase and the
  translation: whether the `[Auto]:`/`[Russian]:`-style label in front of
  each line is shown at all (default: shown). Persisted as
  `phrase_show_prompt`/`translation_show_prompt` in `tagent-gui.json`. An
  entry with no prompt of its own (an error line) never shows one, regardless
  of this setting.

## [0.14.0+008] - 2026-09-12

### Added
- "Blocks spacing (px)" setting in Settings > View: controls the vertical gap
  between one phrase/translation pair and the next in the transcript, in
  pixels (default 20). Persisted as `block_spacing_px` in `tagent-gui.json`.
- "Phrases spacing (px)" setting in Settings > View: controls the vertical
  gap between a phrase and its own translation, within one pair (default 2).
  Persisted as `phrases_spacing_px` in `tagent-gui.json`.

### Fixed
- A phrase and its translation still had a visible gap between their text at
  `phrases_spacing_px: 0`, reading as a stray blank line. Cause:
  each row carried its own fixed vertical padding regardless of the spacing
  setting, so that padding put a floor under the gap no setting could remove.
  All vertical padding on the phrase/translation rows is now removed, so the
  spacing setting is the sole contributor to that gap and 0 means visually
  flush, whether the two rows share a background or not.

## [0.14.0+007] - 2026-09-12

### Fixed
- The HEX field in the phrase/translation color picker (Settings > View)
  didn't update while dragging the R/G/B sliders — it only reflected typed
  input, since Slint has no built-in way to format numbers as hex. Each
  slider now fires a `rgb-changed` callback that `main.rs` uses to reformat
  the field's current color into `"#RRGGBB"` and write it back into the HEX
  text as you drag.

## [0.14.0+006] - 2026-09-12

### Fixed
- The phrase/translation color-picker popup (Settings > View) closed itself on
  every slider drag instead of staying open, because `PopupWindow`'s default
  `close-policy` is `close-on-click` — any click, including one on a slider
  inside the popup, counted as "close". Set explicitly to
  `close-on-click-outside` so the popup only closes when clicking elsewhere.

## [0.14.0+005] - 2026-09-12

### Added
- Independent transcript styling for the phrase (original text) and its
  translation in the `View` tab: font family (Monospace/Sans Serif/Serif),
  font size, text color, and background color, each configurable separately
  for the two roles. Colors default to "Theme default" (follows the active
  theme) or can be set via a HEX field plus an in-app RGB-slider color
  picker. Persisted as `phrase_font`/`phrase_size`/`phrase_color`/
  `phrase_background` and the matching `translation_*` fields in
  `tagent-gui.json` (empty color string = theme default).

### Changed
- The transcript is now rendered from a structured list of entries (one
  phrase/translation pair each) instead of a single flat text blob, so the
  two lines can carry independent styling. Each line remains its own
  read-only, selectable/copyable text field; selecting text no longer spans
  across entries in one continuous drag the way the old single-blob
  transcript did.

## [0.14.0+004] - 2026-09-12

### Added
- `About` tab in the Settings dialog, showing the application name and current
  version (`env!("CARGO_PKG_VERSION")`).

## [0.14.0+003] - 2026-09-11

### Added
- Theme setting: a new `View` tab in the Settings dialog with `Auto`/`Light`/`Dark`,
  persisted as `theme` in `tagent-gui.json` (default `"auto"`, following the
  system setting). Applied live on save — no restart needed — to both the main
  window and any Settings dialog opened afterward.
- Full theme support, not just widget chrome: the transcript and input panels
  (previously hardcoded near-black regardless of system theme) now derive their
  colors from `Palette`'s semantic roles (`background`, `alternate-background`,
  `border`, `foreground`, `selection-*`, `control-*`), so `Light` actually looks
  light, not just the buttons/dropdowns/dialog frame.

### Changed
- `GuiConfig` gained a `theme` field (`#[serde(default)]`, so existing
  `tagent-gui.json` files without it still load fine, defaulting to `"auto"`).

## [0.14.0+002] - 2026-09-11

### Fixed
- Settings dialog no longer opens at an oversized default size with its OK/Cancel
  buttons stretched into full-height vertical bars — it had no explicit
  `preferred-width`/`preferred-height`, so it fell back to Slint's default window
  size with the small amount of real content pinned in a corner. Now sized
  explicitly (420×300) with OK/Cancel laid out as a normal row at the bottom.

### Added
- Settings dialog content is now organized into tabs (`General`, `Hotkeys & Tray`)
  instead of a single flat panel — laid out ahead of the settings that will
  populate the second tab in a later stage (hotkey string, start-minimized-to-tray,
  popup auto-hide delay), so adding a new settings category later is a new `Tab {
  }` block, not a redesign. `General` holds the one setting that exists today
  (`translate_provider`); `Hotkeys & Tray` is a placeholder pending that stage.

## [0.14.0+001] - 2026-09-11

### Added
- Settings dialog: a ⚙ button (top-right of the main window) opens a dialog to
  change `translate_provider` from a dropdown of known providers (currently just
  `"google"`), instead of hand-editing `tagent-gui.json`. OK saves and closes;
  Cancel (or the dialog's own close button) discards the change. The file stays
  hand-editable to any provider string regardless — the dropdown is a convenience,
  not a validation gate.
- `GuiConfigManager::update()`: applies a config change in memory immediately and
  persists it to disk, refreshing the tracked modification time so the write
  doesn't trigger a redundant reload on the next translation.

## [0.14.0+000] - 2026-09-11

### Added
- Own configuration file, `tagent-gui.json` (`~/.config/tagent-gui/tagent-gui.json`
  on Linux/macOS, `%APPDATA%\tagent-gui\tagent-gui.json` on Windows), holding
  `translate_provider`. Plain, pretty-printed JSON meant to be hand-editable — there
  is no Settings window yet. A missing file gets a fresh default written
  (`"google"`); a present-but-invalid file is left untouched on disk and the app
  falls back to its last valid in-memory config, logging a warning.
- Live-reload: the config file's modification time is checked before each
  translation, so a hand-edit takes effect on the next translation without
  restarting the app — no separate reload action needed.

### Changed
- Established `tagent-gui` as a fully independent application from `tagent-cli`:
  own interface, own configuration, own feature set, own versioning — built only
  on the `tagent` library. This file starts tracking notable changes from this
  point forward.
- Versioning now uses the same `MAJOR.MINOR.PATCH+BUILD` format and increment
  rules as `tagent-cli`, with its own independent counter (no shared version
  number, no `build.rs` auto-sync).

### Removed
- The inline `tagent-cli.conf` reader for `TranslateProvider`. **No migration
  path**: an existing `TranslateProvider` setting in `tagent-cli.conf` is no
  longer read by `tagent-gui` — set `translate_provider` in the new
  `tagent-gui.json` instead (it's created with the `"google"` default on first
  run if missing).

## [0.13.0] - 2026-08-05

### Added
- Initial `tagent-gui` prototype: Slint desktop GUI, translate-only, built directly
  on the `tagent` library. Not tracked by this changelog prior to this entry.
