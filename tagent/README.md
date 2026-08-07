# tagent

Translation, dictionary lookup, and text-to-speech library, powered by the Google
Translate API. This is the reusable core behind [Tagent](../tagent-cli/README.md)'s
CLI/hotkey application and the `tagent-gui` Slint prototype — it has no knowledge of
config files, clipboards, hotkeys, or any other application concern.

## What's here

- **`providers`** — the `TranslationProvider` trait (translate, dictionary lookup,
  language detection, text-to-speech) and `create_provider()` factory. Ships a Google
  Translate implementation (`providers::google::GoogleTranslateProvider`).
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

Run `cargo doc -p tagent --open` for the full API reference.

## Status

Not yet published to crates.io. Versioned independently of `tagent-cli` (plain semver,
starting at `1.0.0` — see [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) for why).
