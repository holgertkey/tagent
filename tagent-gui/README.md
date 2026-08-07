# tagent-gui

A [Slint](https://slint.dev/) desktop GUI for [Tagent](../tagent-cli/README.md),
built directly on the [`tagent`](../tagent/README.md) library. Currently a
**translate-only prototype**.

## Running

```bash
cargo run -p tagent-gui
```

Pick a source/target language, type text, press Enter (or click Translate). The ⇄
button swaps source and target.

## What it does and doesn't do

- Reads `TranslateProvider` from `tagent-cli.conf` at startup (via a small inline reader,
  not the full config system — see [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md)),
  defaulting to `"google"`. Not live-reloaded — restart to pick up a config change.
- Hardcodes a 6-language list (Auto/English/Russian/Spanish/French/German), smaller
  than the ~16 languages `tagent-cli` supports.
- No dictionary/spell-check display, no text-to-speech, no clipboard integration, no
  global hotkeys, no history logging. For the full feature set, use `tagent-cli`
  (the `tagent` binary).

## Status

Prototype — not linked from `tagent-cli`, no shared launch path between the two.
