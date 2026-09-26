//! Unified error type for the `tagent` library.

use std::time::Duration;

/// Errors that can occur while translating, looking up dictionary entries,
/// or synthesizing speech.
///
/// The enum is `#[non_exhaustive]`: new variants can be added in a compatible release, so a
/// `match` outside this crate needs a wildcard arm.
///
/// # Examples
///
/// ```
/// use tagent::error::Error;
///
/// fn describe(error: &Error) -> String {
///     match error {
///         Error::Auth(_) => "check your API key".to_string(),
///         Error::RateLimited { .. } => "too many requests, try again later".to_string(),
///         other => other.to_string(),
///     }
/// }
///
/// assert_eq!(describe(&Error::Auth("invalid key".into())), "check your API key");
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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
    /// [`create_provider`](crate::providers::create_provider),
    /// [`create_dictionary_provider`](crate::providers::create_dictionary_provider) or
    /// [`create_speech_provider`](crate::providers::create_speech_provider) was called
    /// with a name that does not match any known provider.
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    /// The provider rejected the credentials: missing, invalid or expired (HTTP 401/403).
    #[error("authentication failed: {0}")]
    Auth(String),
    /// The provider is throttling requests (HTTP 429).
    #[error("rate limited by the provider{}", retry_after_hint(.retry_after))]
    RateLimited {
        /// How long the provider asked the caller to wait (its `Retry-After`), if it said.
        retry_after: Option<Duration>,
    },
    /// The account's quota or character allowance is used up (e.g. DeepL's HTTP 456).
    #[error("provider quota exceeded: {0}")]
    QuotaExceeded(String),
    /// The provider doesn't support this operation or language pair, e.g.
    /// [`detect_language`](crate::providers::TranslationProvider::detect_language) on a
    /// backend without language detection.
    #[error("not supported by the provider: {0}")]
    Unsupported(String),
    /// A provider option is missing or has an invalid value (e.g. a required API key).
    #[error("invalid provider options: {0}")]
    InvalidOptions(String),
}

/// Formats [`Error::RateLimited`]'s optional wait for its `Display` message.
fn retry_after_hint(retry_after: &Option<Duration>) -> String {
    match retry_after {
        Some(wait) => format!(" (retry after {} s)", wait.as_secs_f64().ceil() as u64),
        None => String::new(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_variants_display() {
        assert_eq!(
            Error::Auth("invalid key".into()).to_string(),
            "authentication failed: invalid key"
        );
        assert_eq!(
            Error::RateLimited { retry_after: None }.to_string(),
            "rate limited by the provider"
        );
        assert_eq!(
            Error::RateLimited { retry_after: Some(Duration::from_secs(3)) }.to_string(),
            "rate limited by the provider (retry after 3 s)"
        );
        // A fractional wait rounds up, so the hint never suggests retrying too early.
        assert_eq!(
            Error::RateLimited { retry_after: Some(Duration::from_millis(1200)) }.to_string(),
            "rate limited by the provider (retry after 2 s)"
        );
        assert_eq!(
            Error::QuotaExceeded("character limit reached".into()).to_string(),
            "provider quota exceeded: character limit reached"
        );
        assert_eq!(
            Error::Unsupported("language detection".into()).to_string(),
            "not supported by the provider: language detection"
        );
        assert_eq!(
            Error::InvalidOptions("missing api_key".into()).to_string(),
            "invalid provider options: missing api_key"
        );
    }

    #[tokio::test]
    async fn reqwest_timeout_still_maps_to_network() {
        // A listener that accepts but never answers, so the request can only time out.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let error: Error = client.get(url).send().await.unwrap_err().into();
        assert!(
            matches!(&error, Error::Network(message) if message == "request timed out"),
            "{error:?}"
        );
        drop(listener);
    }
}
