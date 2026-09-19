# tagent

Translation, dictionary lookup, and text-to-speech library, powered by the Google
Translate API. This is the reusable core behind [Tagent](../tagent-cli/README.md)'s
CLI/hotkey application and the `tagent-gui` Slint prototype — it has no knowledge of
config files, clipboards, hotkeys, or any other application concern.

## What's here

- **`providers`** — two independent provider axes: the `TranslationProvider` trait
  (translate, dictionary lookup, language detection) with its `create_provider()`
  factory, and the `SpeechProvider` trait (text-to-speech) with `create_speech_provider()`.
  Ships Google implementations of both (`providers::google::GoogleTranslateProvider`,
  `providers::google::GoogleSpeechProvider`).
- **`languages`** — human-readable language name ↔ BCP-47 code mapping
  (`name_to_code` / `code_to_name`).
- **`error`** — unified `Error` type (via `thiserror`) used across the crate.

## Usage

```rust
#[tokio::main]
async fn main() -> Result<(), tagent::error::Error> {
    let provider = tagent::providers::create_provider("google")?;
    let translated = provider.translate_text("Hello world", "auto", "ru").await?;
    println!("{translated}");
    Ok(())
}
```

Run `cargo doc -p tagent --open` for the full API reference. The crate and `providers`
module documentation cover the shared contracts (language codes, the `"auto"` source
language, the error variants) and how to write a new provider; the `google` module
documentation lists the caveats of the built-in providers, which use unofficial endpoints.

## Examples

Runnable examples live in [`examples/`](examples/):

```bash
cargo run -p tagent --example translate -- "Hello world" ru   # needs network
cargo run -p tagent --example dictionary -- vialent           # needs network
cargo run -p tagent --example speak -- "Hello world" en       # needs network, writes a temp-dir .mp3
cargo run -p tagent --example custom_provider                 # offline: both traits on a toy backend
```

## Status

Not yet published to crates.io. Versioned independently of `tagent-cli` and
`tagent-gui` (plain semver, currently pre-1.0 — see
[`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md)); history in [CHANGELOG.md](CHANGELOG.md).
