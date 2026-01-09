use async_trait::async_trait;
use std::error::Error;

pub mod google;

// Common dictionary entry structure for all providers
#[derive(Debug, Clone)]
pub struct DictionaryEntry {
    pub word: String,
    pub definitions: Vec<PartOfSpeechEntry>,
}

#[derive(Debug, Clone)]
pub struct PartOfSpeechEntry {
    pub part_of_speech: String,
    pub definitions: Vec<Definition>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub text: String,
    pub synonyms: Vec<String>,
}

// Main translation provider trait
#[async_trait]
pub trait TranslationProvider: Send + Sync {
    /// Translate text from one language to another
    async fn translate_text(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<String, Box<dyn Error>>;

    /// Get dictionary entry for a single word
    /// Returns None if dictionary lookup is not supported or word not found
    async fn get_dictionary_entry(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Box<dyn Error>>;

    /// Get provider name for display purposes
    fn name(&self) -> &str;
}

/// Create translation provider based on name
pub fn create_provider(provider_name: &str) -> Result<Box<dyn TranslationProvider>, Box<dyn Error>> {
    match provider_name.to_lowercase().as_str() {
        "google" => Ok(Box::new(google::GoogleTranslateProvider::new())),
        _ => Err(format!("Unknown translation provider: {}", provider_name).into()),
    }
}
