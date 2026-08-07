//! Unified error type for the `tagent` library.

/// Errors that can occur while translating, looking up dictionary entries,
/// or synthesizing speech.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Network/transport failure. Stored as a message, not `reqwest::Error`,
    /// so a future reqwest major-version bump isn't a breaking change for
    /// consumers of this crate.
    #[error("network error: {0}")]
    Network(String),
    /// The provider's API responded with an error status or unexpected body.
    #[error("provider API error: {0}")]
    Api(String),
    /// A dictionary lookup found no entry for the requested word.
    #[error("word not found in dictionary")]
    NotFound,
    /// Input text was empty when non-empty text was required.
    #[error("text is empty")]
    EmptyText,
    /// Input text exceeded the maximum length a provider accepts for a single request.
    #[error("text too long: {len} chars (max {max})")]
    TextTooLong {
        /// Length of the input text, in bytes.
        len: usize,
        /// Maximum length accepted, in bytes.
        max: usize,
    },
    /// The provider's response body could not be decoded into the expected shape.
    #[error("failed to decode provider response: {0}")]
    Decode(String),
    /// [`create_provider`](crate::providers::create_provider) was called with a name
    /// that does not match any known provider.
    #[error("unknown translation provider: {0}")]
    UnknownProvider(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Error::Network("request timed out".to_string())
        } else {
            Error::Network(e.to_string())
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Decode(e.to_string())
    }
}
