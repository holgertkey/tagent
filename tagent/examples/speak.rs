//! Synthesize speech to an MP3 file with the built-in Google provider.
//!
//! Needs network access. Usage:
//!
//! ```text
//! cargo run -p tagent --example speak -- "Hello world" [lang] [output.mp3]
//! ```
//!
//! `lang` defaults to `auto`, which is resolved with a translation provider first: speech
//! itself needs a concrete language code. `output` defaults to `tagent-speech.mp3` in the
//! system temp directory, so running the example never dirties your working tree.

use tagent::providers;

#[tokio::main]
async fn main() -> Result<(), tagent::error::Error> {
    let mut args = std::env::args().skip(1);
    let Some(text) = args.next() else {
        eprintln!("usage: speak <text> [lang] [output.mp3]");
        std::process::exit(2);
    };
    let lang = args.next().unwrap_or_else(|| "auto".to_string());
    let output = args.next().unwrap_or_else(|| {
        std::env::temp_dir()
            .join("tagent-speech.mp3")
            .to_string_lossy()
            .into_owned()
    });

    // The speech provider never needs a translation provider; only an "auto" language does,
    // to detect it. `resolve_source_language` passes a concrete code through untouched, so
    // build the translation provider only when it is actually needed.
    let lang = if lang == "auto" {
        let translator = providers::create_provider("google")?;
        providers::resolve_source_language(translator.as_ref(), &text, &lang).await
    } else {
        lang
    };

    let speech = providers::create_speech_provider("google")?;

    // Splitting is pure. Fetching chunk by chunk (instead of all at once) is what lets a
    // player start after the first round trip; here we simply concatenate the MP3 clips.
    let chunks = speech.split_for_speech(&text);
    let mut audio = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        eprintln!("chunk {}/{} ({} bytes)", i + 1, chunks.len(), chunk.len());
        audio.extend(speech.speak_chunk(chunk, &lang).await?);
    }

    std::fs::write(&output, &audio).expect("failed to write the output file");
    println!(
        "{}: wrote {} bytes ({lang}) to {output}",
        speech.name(),
        audio.len()
    );
    Ok(())
}
