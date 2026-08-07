# Tagent

Cross-platform text translation, split across three Cargo workspace crates:

| Crate | What it is | README |
|---|---|---|
| **`tagent`** | Translation/dictionary/TTS library (Google Translate provider, no app code) | [tagent/README.md](tagent/README.md) |
| **`tagent-cli`** | The Tagent application — global hotkeys, interactive terminal, CLI mode | [tagent-cli/README.md](tagent-cli/README.md) |
| **`tagent-gui`** | Slint desktop GUI prototype, translate-only | [tagent-gui/README.md](tagent-gui/README.md) |

Both `tagent-cli` and `tagent-gui` depend on the `tagent` library; `tagent` depends on
neither. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full breakdown.

**Most users want [`tagent-cli`](tagent-cli/README.md)** — that's the actual
translator application, including installation and usage instructions.

## Building everything

```bash
git clone https://github.com/holgertkey/tagent
cd tagent
cargo build --release
```

Builds all three crates. The `tagent-cli` package's binary is still named `tagent`, so
it lands at `target/release/tagent` (`target/release/tagent.exe` on Windows).

See [CHANGELOG.md](CHANGELOG.md) for version history and [LICENSE](LICENSE) for
license terms (MIT).
