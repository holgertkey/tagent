//! Translate a phrase with the built-in Google provider.
//!
//! Needs network access. Usage:
//!
//! ```text
//! cargo run -p tagent --example translate -- "Hello world" [target] [source]
//! ```
//!
//! `target` defaults to `ru` and `source` to `auto` (the provider detects it). Either may be
//! a BCP-47 code or a language name such as `Russian`.

use tagent::{languages, providers};

#[tokio::main]
async fn main() -> Result<(), tagent::error::Error> {
    let mut args = std::env::args().skip(1);
    let Some(text) = args.next() else {
        eprintln!("usage: translate <text> [target] [source]");
        std::process::exit(2);
    };
    let target = args.next().unwrap_or_else(|| "ru".to_string());
    let source = args.next().unwrap_or_else(|| "auto".to_string());

    // `name_to_code` accepts either a name ("Russian") or a code ("ru") and returns the code.
    let target = languages::name_to_code(&target);
    let source = languages::name_to_code(&source);

    // Building a provider allocates an HTTP client, so create it once and reuse it.
    let provider = providers::create_provider("google")?;
    let translated = provider.translate_text(&text, source, target).await?;

    println!("{} ({source} -> {target}):", provider.name());
    println!("{translated}");
    Ok(())
}
