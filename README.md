# Tagent

Cross-platform text translation, split across three Cargo workspace crates:

| Crate | What it is | README |
|---|---|---|
| **`tagent`** | Translation/dictionary/TTS library (Google Translate provider, no app code) | [tagent/README.md](tagent/README.md) |
| **`tagent-cli`** | The Tagent application — global hotkeys, interactive terminal, CLI mode | [tagent-cli/README.md](tagent-cli/README.md) |
| **`tagent-gui`** | Slint desktop translator — transcript window, selection hotkeys with a popup, tray, dictionary, text-to-speech; a fully independent app | [tagent-gui/README.md](tagent-gui/README.md) |

Both `tagent-cli` and `tagent-gui` depend on the `tagent` library; `tagent` depends on
neither. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full breakdown.

`tagent-gui` is a fully independent application from `tagent-cli` — its own interface,
configuration, feature set, versioning, and [changelog](tagent-gui/CHANGELOG.md); the
`tagent` library is the only thing the two share. Each crate keeps its own changelog
next to its `Cargo.toml`: [`tagent-cli`](tagent-cli/CHANGELOG.md),
[`tagent-gui`](tagent-gui/CHANGELOG.md), and [`tagent`](tagent/CHANGELOG.md).

**Most users want [`tagent-cli`](tagent-cli/README.md)** — that's the actual
translator application, including installation and usage instructions.

## Building everything

```bash
git clone https://github.com/holgertkey/tagent
cd tagent
cargo build --release
```

Builds all three crates. The binaries land at `target/release/tagent-cli` and
`target/release/tagent-gui` (`.exe` on Windows). On Linux the build needs the X11, XTest,
ALSA and fontconfig development packages, e.g. on Debian/Ubuntu:

```bash
sudo apt-get install libx11-dev libxtst-dev libasound2-dev libfontconfig1-dev
```

Prebuilt `tagent-cli` and `tagent-gui` binaries for Windows and Linux, plus a `.deb` of
`tagent-gui`, are attached to each
[GitHub Release](https://github.com/holgertkey/tagent/releases).

Plans and design notes: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
[`docs/providers-dev-plan.md`](docs/providers-dev-plan.md) (provider roadmap),
[`docs/tagent-gui-dev-plan.md`](docs/tagent-gui-dev-plan.md) and
[`docs/user-docs-plan.md`](docs/user-docs-plan.md) (the planned user book).

See each crate's `CHANGELOG.md` (linked above) for version history and [LICENSE](LICENSE)
for license terms (MIT).
