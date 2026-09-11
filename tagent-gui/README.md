# tagent-gui

A [Slint](https://slint.dev/) desktop GUI translator, built directly on the
[`tagent`](../tagent/README.md) library. Currently a **translate-only prototype**.

`tagent-gui` is a fully independent application from
[`tagent-cli`](../tagent-cli/README.md) — its own interface, its own configuration
(roadmap; see below), its own feature set, and its own versioning and
[CHANGELOG.md](CHANGELOG.md). The only thing the two share is the `tagent` library
underneath. See [`tagent-gui development plan.md`](../.debug/tagent-gui%20development%20plan.md)
for the reasoning and roadmap.

## Running

```bash
cargo run -p tagent-gui
```

Pick a source/target language, type text, press Enter (or click Translate). The ⇄
button swaps source and target.

## What it does and doesn't do

- Reads `translate_provider` from its own `tagent-gui.json` config file (see
  [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md)), defaulting to `"google"` and
  creating the file with that default on first run. Plain, pretty-printed JSON,
  meant to be hand-editable — there's no Settings window yet. Changes are
  live-reloaded (checked before each translation), no restart needed. A missing
  file gets a fresh default written; a present-but-invalid file is left untouched
  and the app logs a warning and keeps using its last valid config in memory.
  Location: `~/.config/tagent-gui/tagent-gui.json` on Linux/macOS,
  `%APPDATA%\tagent-gui\tagent-gui.json` on Windows.
- Hardcodes a 6-language list (Auto/English/Russian/Spanish/French/German). Not
  required to match `tagent-cli`'s ~16 — `tagent-gui` sets its own feature roadmap.
- No dictionary/spell-check display, no text-to-speech, no clipboard integration, no
  global hotkeys, no history logging yet. These are independent roadmap items, not a
  parity checklist against `tagent-cli` — see the development plan for what's
  actually planned.

## Status

Prototype — not linked from `tagent-cli`, no shared launch path between the two, and
no obligation to reach feature parity with it. See
[CHANGELOG.md](CHANGELOG.md) for its own version history.
