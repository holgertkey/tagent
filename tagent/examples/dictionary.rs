//! Look up a single word in the dictionary, showing the spelling-correction notice.
//!
//! Needs network access. Usage:
//!
//! ```text
//! cargo run -p tagent --example dictionary -- vialent [source] [target]
//! ```
//!
//! `source` defaults to `en` and `target` to `ru`. Try a misspelled word such as `vialent`
//! or `violnt` to see [`DictionaryEntry::corrected_word`] in action.

use tagent::providers::{self, DictionaryEntry};

#[tokio::main]
async fn main() -> Result<(), tagent::error::Error> {
    let mut args = std::env::args().skip(1);
    let Some(word) = args.next() else {
        eprintln!("usage: dictionary <word> [source] [target]");
        std::process::exit(2);
    };
    let source = args.next().unwrap_or_else(|| "en".to_string());
    let target = args.next().unwrap_or_else(|| "ru".to_string());

    let provider = providers::create_provider("google")?;

    // A dictionary miss is `Ok(None)`, not an error. Callers usually fall back to a plain
    // translation for it (and for multi-word input, which is not a dictionary lookup).
    let Some(entry) = provider
        .get_dictionary_entry(&word, &source, &target)
        .await?
    else {
        println!("no dictionary entry for {word:?}; falling back to a plain translation:");
        println!(
            "{}",
            provider.translate_text(&word, &source, &target).await?
        );
        return Ok(());
    };

    print_entry(&word, &entry);
    Ok(())
}

fn print_entry(original: &str, entry: &DictionaryEntry) {
    // `corrected_word` is the word the provider actually looked up. It is `Some` even for a
    // correctly spelled input, so compare it with what the user typed before showing a notice.
    if let Some(corrected) = &entry.corrected_word {
        if corrected.to_lowercase() != original.to_lowercase() {
            println!("(showing results for {corrected:?} instead of {original:?})");
        }
    }

    println!("{}", entry.word);
    for pos in &entry.definitions {
        println!("  {}", pos.part_of_speech);
        for definition in &pos.definitions {
            println!("    - {}", definition.text);
            if !definition.synonyms.is_empty() {
                println!("      synonyms: {}", definition.synonyms.join(", "));
            }
        }
    }
}
