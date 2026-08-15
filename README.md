# Tagent

Cross-platform text translation, split across three Cargo workspace crates:

| Crate | What it is | README |
|---|---|---|
| **`tagent`** | Translation/dictionary/TTS library (Google Translate provider, no app code) | [tagent/README.md](tagent/README.md) |
| **`tagent-cli`** | The Tagent application — global hotkeys, interactive terminal, CLI mode | [tagent-cli/README.md](tagent-cli/README.md) |
| **`tagent-gui`** | Slint desktop GUI prototype, translate-only, fully independent app | [tagent-gui/README.md](tagent-gui/README.md) |

Both `tagent-cli` and `tagent-gui` depend on the `tagent` library; `tagent` depends on
neither. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full breakdown.

`tagent-gui` is a fully independent application from `tagent-cli` — its own interface,
configuration, feature set, versioning, and [changelog](tagent-gui/CHANGELOG.md); the
`tagent` library is the only thing the two share. This root `CHANGELOG.md` tracks
`tagent-cli` only.

**Most users want [`tagent-cli`](tagent-cli/README.md)** — that's the actual
translator application, including installation and usage instructions.

## Building everything

```bash
git clone https://github.com/holgertkey/tagent
cd tagent
cargo build --release
```

Builds all three crates. The `tagent-cli` package's binary is named `tagent-cli`, so
it lands at `target/release/tagent-cli` (`target/release/tagent-cli.exe` on Windows).

See [CHANGELOG.md](CHANGELOG.md) for version history and [LICENSE](LICENSE) for
license terms (MIT).
