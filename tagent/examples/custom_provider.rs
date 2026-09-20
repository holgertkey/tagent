//! Implement all three provider traits for toy backends, fully offline.
//!
//! Shows what a new backend has to provide and how the three provider axes fit together,
//! without any network access:
//!
//! ```text
//! cargo run -p tagent --example custom_provider
//! ```
//!
//! A real backend would replace the bodies with calls to its own HTTP API. To make one
//! selectable by name (`create_provider("reverse")`), add a branch to
//! `tagent::providers::create_provider` (or `create_dictionary_provider` /
//! `create_speech_provider`) in the library; using it directly, as here, needs no
//! registration.

use async_trait::async_trait;
use tagent::error::Error;
use tagent::providers::{
    resolve_source_language, Definition, DictionaryEntry, DictionaryProvider, PartOfSpeechEntry,
    SpeechProvider, TranslationProvider,
};

/// A "translator" that reverses its input.
struct Reverse;

#[async_trait]
impl TranslationProvider for Reverse {
    async fn translate_text(&self, text: &str, _from: &str, _to: &str) -> Result<String, Error> {
        if text.trim().is_empty() {
            return Err(Error::EmptyText);
        }
        Ok(text.chars().rev().collect())
    }

    async fn detect_language(&self, _text: &str) -> Result<String, Error> {
        Ok("en".to_string())
    }

    fn name(&self) -> &str {
        "Reverse"
    }
}

/// A "dictionary" that knows exactly one word.
struct Glossary;

#[async_trait]
impl DictionaryProvider for Glossary {
    async fn lookup(
        &self,
        word: &str,
        _from: &str,
        _to: &str,
    ) -> Result<Option<DictionaryEntry>, Error> {
        // A miss is `Ok(None)`, never an error.
        if !word.eq_ignore_ascii_case("hello") {
            return Ok(None);
        }
        // Part-of-speech labels are lowercase English words; `text` is a translation into the
        // target language and `synonyms` are source-language words (both trivially "English"
        // here, since this toy backend does not translate).
        Ok(Some(DictionaryEntry::new(
            word,
            vec![PartOfSpeechEntry::new(
                "interjection",
                vec![Definition::new(
                    "used as a greeting",
                    vec!["hi".to_string(), "hey".to_string()],
                )],
            )],
        )))
    }

    fn name(&self) -> &str {
        "Glossary"
    }
}

/// A "speaker" with a small per-request limit, returning the text bytes as fake "audio".
struct Echo;

const ECHO_LIMIT: usize = 12;

#[async_trait]
impl SpeechProvider for Echo {
    fn split_for_speech(&self, text: &str) -> Vec<String> {
        // The chunk size is this backend's own constraint. A backend with no limit would
        // return `vec![text.to_string()]` instead. Splitting on whitespace keeps this toy
        // simple; a real one must also cope with a single word longer than its limit.
        let mut chunks: Vec<String> = Vec::new();
        for word in text.split_whitespace() {
            match chunks.last_mut() {
                Some(last) if last.len() + 1 + word.len() <= ECHO_LIMIT => {
                    last.push(' ');
                    last.push_str(word);
                }
                _ => chunks.push(word.to_string()),
            }
        }
        chunks
    }

    async fn speak_chunk(&self, text: &str, lang: &str) -> Result<Vec<u8>, Error> {
        if text.is_empty() {
            return Err(Error::EmptyText);
        }
        // Reject rather than truncate: the caller is responsible for splitting first.
        if text.len() > ECHO_LIMIT {
            return Err(Error::TextTooLong {
                len: text.len(),
                max: ECHO_LIMIT,
            });
        }
        // `lang` is always a concrete code by the time it gets here.
        Ok(format!("[{lang}] {text}").into_bytes())
    }

    fn name(&self) -> &str {
        "Echo"
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let translator: Box<dyn TranslationProvider> = Box::new(Reverse);
    let dictionary: Box<dyn DictionaryProvider> = Box::new(Glossary);
    let speaker: Box<dyn SpeechProvider> = Box::new(Echo);

    println!(
        "{}",
        translator
            .translate_text("Hello world", "auto", "en")
            .await?
    );

    match dictionary.lookup("hello", "en", "en").await? {
        Some(entry) => println!(
            "{}: {} part(s) of speech",
            entry.word,
            entry.definitions.len()
        ),
        None => println!("no entry"),
    }
    println!(
        "missing word -> {:?}",
        dictionary.lookup("zzz", "en", "en").await?.map(|e| e.word)
    );

    // Two axes meet only here: speaking "auto" text needs the translator to detect the
    // language first; a concrete code (like "de") would skip the translator entirely.
    let lang = resolve_source_language(translator.as_ref(), "Hello world", "auto").await;
    for chunk in speaker.split_for_speech("Hello wonderful world") {
        let audio = speaker.speak_chunk(&chunk, &lang).await?;
        println!(
            "{} chunk {chunk:?} -> {} bytes",
            speaker.name(),
            audio.len()
        );
    }

    // An oversized chunk is an error, not a truncation.
    let too_long = speaker
        .speak_chunk("far too long for this backend", &lang)
        .await;
    assert!(matches!(too_long, Err(Error::TextTooLong { .. })));
    Ok(())
}
