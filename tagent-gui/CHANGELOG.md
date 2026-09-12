# Changelog

All notable changes to `tagent-gui` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent-gui` versions independently of `tagent-cli` (see the root
[CHANGELOG.md](../CHANGELOG.md) for that application's history) and independently of
the `tagent` library. See [`tagent-gui development plan.md`](../.debug/tagent-gui%20development%20plan.md)
for the roadmap and design decisions behind this project.

## [Unreleased]

## [0.14.0+009] - 2026-09-12

### Added
- "Show prompt" checkbox, prompt size, and prompt color settings in
  Settings > View, separately for the phrase and the translation. Controls
  whether the `[Auto]:`/`[Russian]:`-style label in front of each line is
  shown at all, and its own font size/color independent of the text that
  follows it (default: shown, size 13, theme-default color). Persisted as
  `phrase_show_prompt`/`phrase_prompt_size`/`phrase_prompt_color` and the
  matching `translation_*` fields in `tagent-gui.json`. An entry with no
  prompt of its own (an error line) never shows one, regardless of this
  setting.

### Changed
- `TranscriptEntry` now carries the prompt and text of each line as separate
  fields (`phrase-prompt`/`phrase-text`/`translation-prompt`/
  `translation-text`) instead of one pre-formatted string per line, so the
  prompt can be toggled and styled independently. On wrap, the text no longer
  lines up under the prompt on continuation lines (no hanging indent) — it
  wraps within the width remaining after the prompt.

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
