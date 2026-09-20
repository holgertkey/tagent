# tagent

Translation, dictionary lookup, and text-to-speech library, powered by the Google
Translate API. This is the reusable core behind
[Tagent](https://github.com/holgertkey/tagent/tree/main/tagent-cli)'s CLI/hotkey
application and the
[`tagent-gui`](https://github.com/holgertkey/tagent/tree/main/tagent-gui) desktop app —
it has no knowledge of config files, clipboards, hotkeys, or any other application
concern.

## What's here

- **`providers`** — three independent provider axes, so the backend that translates, the
  one that looks words up and the one that speaks are separate choices, usable in any
  combination:

  | Axis | Trait | Factory | Built in |
  |---|---|---|---|
  | Translation (and language detection) | `TranslationProvider` | `create_provider()` | Google |
  | Dictionary | `DictionaryProvider` | `create_dictionary_provider()` | Google |
  | Speech (TTS) | `SpeechProvider` | `create_speech_provider()` | Google |

  `TRANSLATION_PROVIDERS`, `DICTIONARY_PROVIDERS` and `SPEECH_PROVIDERS` list the names
  each factory accepts, so a settings dialog or an error message can offer them without
  hardcoding.
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

A single-word dictionary lookup, with spelling correction, goes through its own provider:

```rust
#[tokio::main]
async fn main() -> Result<(), tagent::error::Error> {
    let dictionary = tagent::providers::create_dictionary_provider("google")?;
    if let Some(entry) = dictionary.lookup("vialent", "en", "ru").await? {
        println!("looked up {:?}", entry.corrected_word);
    }
    Ok(())
}
```

Run `cargo doc -p tagent --open` for the full API reference. The crate and `providers`
module documentation cover the shared contracts (language codes, the `"auto"` source
language, the error variants) and how to write a new provider; the `google` module
documentation lists the caveats of the built-in providers, which use unofficial endpoints.

## Examples

Runnable examples live in [`examples/`](https://github.com/holgertkey/tagent/tree/main/tagent/examples)
(run them from a checkout of the repository):

```bash
cargo run -p tagent --example translate -- "Hello world" ru   # needs network
cargo run -p tagent --example dictionary -- vialent           # needs network
cargo run -p tagent --example speak -- "Hello world" en       # needs network, writes a temp-dir .mp3
cargo run -p tagent --example custom_provider                 # offline: both traits on a toy backend
```

## Status

Pre-1.0: a breaking change to the public API bumps the minor version (`0.18` → `0.19`), a
compatible addition or fix bumps the patch. `1.0.0` waits until the API settles.
Versioned independently of `tagent-cli` and `tagent-gui`; history in
[CHANGELOG.md](CHANGELOG.md).

Releases of this crate up to `0.12.0` were the old, standalone Tagent application; the
library described here starts after that, so a plain `cargo add tagent` gets the library
while an old `0.12` requirement gets the application.
