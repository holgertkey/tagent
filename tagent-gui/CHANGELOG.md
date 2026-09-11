# Changelog

All notable changes to `tagent-gui` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent-gui` versions independently of `tagent-cli` (see the root
[CHANGELOG.md](../CHANGELOG.md) for that application's history) and independently of
the `tagent` library. See [`tagent-gui development plan.md`](../.debug/tagent-gui%20development%20plan.md)
for the roadmap and design decisions behind this project.

## [Unreleased]

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
