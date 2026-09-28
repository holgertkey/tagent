//! DeepL translation provider, over DeepL's official, keyed API.
//!
//! [`DeepLTranslateProvider`] implements [`TranslationProvider`]; DeepL has no dictionary
//! or speech service, so it provides no other axis.
//!
//! # Options
//!
//! | Key | Required | Meaning |
//! |---|---|---|
//! | `api_key` | yes | The DeepL authentication key; secret. `TAGENT_DEEPL_API_KEY` (or `TAGENT_<PROFILE>_API_KEY`) supplies it from the environment. |
//! | `endpoint` | no | API base URL, without `/v2/translate`. Default: `https://api-free.deepl.com` for a Free key (one ending in `:fx`), `https://api.deepl.com` otherwise. |
//! | `timeout_secs`, `max_retries` | no | The generic transport options (default: 10 s budget, 2 retries). |
//!
//! # Behavior worth knowing
//!
//! - **Language codes** are BCP-47 in and DeepL's at the edge, case-insensitively. A source
//!   language becomes its primary subtag (`en-US` → `EN`), and `"auto"` leaves the source
//!   out so DeepL detects it. A target keeps its region (`pt-BR` → `PT-BR`, `en-GB` →
//!   `EN-GB`), except Chinese, which DeepL splits by script: `zh`, `zh-CN`, `zh-SG`,
//!   `zh-Hans` → `ZH-HANS`; `zh-TW`, `zh-HK`, `zh-MO`, `zh-Hant` → `ZH-HANT`. A bare `en` or
//!   `pt` target is sent as `EN` / `PT` and DeepL picks the variant; set e.g. `en-GB` to
//!   choose one. A code DeepL doesn't support comes back as its own error ([`Error::Api`],
//!   HTTP 400, with DeepL's message).
//! - **Language detection costs characters.** DeepL has no detection endpoint, so
//!   [`detect_language`](TranslationProvider::detect_language) translates the first
//!   100 characters of the text into English and reports the detected source language;
//!   those characters are billed like any translation. Its caller in the bundled
//!   applications is speech of `"auto"`-source text.
//! - **Errors**: HTTP 403 → [`Error::Auth`], 456 (character quota used up) →
//!   [`Error::QuotaExceeded`], 429 and 529 → [`Error::RateLimited`] (retried once the
//!   service's `Retry-After` is at most 2 seconds), 502/503/504 are retried, 500 is not.
//!   The key is sent in a header only and never appears in an error message.
//! - **Limits**: DeepL accepts a request body of up to 128 KiB. That is a byte limit of the
//!   whole request, not a character count, so
//!   [`capabilities`](TranslationProvider::capabilities) reports no `max_text_len`, nor a
//!   language list (DeepL's keeps growing).
//!
//! # Examples
//!
//! ```no_run
//! use tagent::providers::{create_provider_with, ProviderOptions};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), tagent::error::Error> {
//! // The key usually comes from a config file or TAGENT_DEEPL_API_KEY.
//! let options = ProviderOptions::new().with_env_overrides("deepl");
//! let deepl = create_provider_with("deepl", &options)?;
//! println!("{}", deepl.translate_text("Hello, world", "auto", "de").await?);
//! # Ok(())
//! # }
//! ```

use super::http::HttpTransport;
use super::registry::DEEPL_TRANSPORT;
use super::{ProviderOptions, TranslationCapabilities, TranslationProvider};
use crate::error::Error;
use async_trait::async_trait;
use serde_json::{json, Value};

/// Base URL for Free keys (those ending in [`FREE_KEY_SUFFIX`]).
const FREE_API_URL: &str = "https://api-free.deepl.com";
/// Base URL for Pro keys.
const PRO_API_URL: &str = "https://api.deepl.com";
/// The suffix that marks a Free key.
const FREE_KEY_SUFFIX: &str = ":fx";
/// The translate endpoint, relative to the base URL.
const TRANSLATE_PATH: &str = "/v2/translate";
/// How much of the text `detect_language` sends, in characters.
const DETECTION_PREFIX_CHARS: usize = 100;
/// The target language of a detection request; the translation itself is discarded.
const DETECTION_TARGET: &str = "EN";
/// Sent as `User-Agent`, as DeepL asks integrations to identify themselves.
const USER_AGENT: &str = concat!("tagent/", env!("CARGO_PKG_VERSION"));

/// [`TranslationProvider`] backed by the DeepL API (`POST /v2/translate`).
///
/// See the [module documentation](self) for the options, the language-code mapping and
/// what language detection costs.
///
/// # Examples
///
/// ```
/// use tagent::providers::{deepl::DeepLTranslateProvider, ProviderOptions, TranslationProvider};
///
/// let options = ProviderOptions::new().with("api_key", "0000-0000:fx");
/// let deepl = DeepLTranslateProvider::with_options(&options).unwrap();
/// assert_eq!(deepl.name(), "DeepL");
/// assert!(deepl.capabilities().detects_language);
///
/// // No key, no provider.
/// assert!(DeepLTranslateProvider::with_options(&ProviderOptions::new()).is_err());
/// ```
pub struct DeepLTranslateProvider {
    transport: HttpTransport,
    /// The full translate URL, e.g. `https://api-free.deepl.com/v2/translate`.
    url: String,
}

impl DeepLTranslateProvider {
    /// Creates a provider from its options: `api_key` (required), `endpoint`, and the
    /// generic `timeout_secs` / `max_retries`. Other options are ignored.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidOptions`] if `api_key` is missing, empty or not a valid header value,
    /// if `endpoint` isn't an `http(s)` URL, or if a transport option is invalid.
    pub fn with_options(options: &ProviderOptions) -> Result<Self, Error> {
        let key = options.require("api_key")?.trim();
        let url = format!(
            "{}{TRANSLATE_PATH}",
            base_url(key, options.get("endpoint"))?
        );
        let transport = HttpTransport::builder(DEEPL_TRANSPORT)
            .user_agent(USER_AGENT)
            .secret_header("authorization", "DeepL-Auth-Key ", key)?
            .quota_statuses(&[456])
            .rate_limit_statuses(&[429, 529])
            .options(options)?
            .build()?;
        Ok(Self { transport, url })
    }

    /// Sends one translation request; returns the translation and the detected source
    /// language (DeepL's code).
    async fn translate(&self, body: &Value) -> Result<(String, Option<String>), Error> {
        let response = self
            .transport
            .send(|client| client.post(&self.url).json(body))
            .await?;
        parse_response(&response)
    }
}

#[async_trait]
impl TranslationProvider for DeepLTranslateProvider {
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Error> {
        if text.trim().is_empty() {
            return Err(Error::EmptyText);
        }
        let (translation, _) = self.translate(&build_request_body(text, from, to)).await?;
        Ok(translation)
    }

    /// Translates the first 100 characters into English and reports the detected source
    /// language, lowercased (`"de"`); those characters are billed.
    async fn detect_language(&self, text: &str) -> Result<String, Error> {
        let prefix = detection_prefix(text.trim());
        if prefix.is_empty() {
            return Err(Error::EmptyText);
        }
        let body = build_request_body(prefix, "auto", DETECTION_TARGET);
        match self.translate(&body).await? {
            (_, Some(detected)) => Ok(from_deepl(&detected)),
            (_, None) => Err(Error::Decode(
                "DeepL response has no `detected_source_language`".to_string(),
            )),
        }
    }

    fn name(&self) -> &str {
        "DeepL"
    }

    /// Detects languages (at a cost, see the [module documentation](self)). No
    /// `max_text_len` (DeepL's limit is 128 KiB of request body, not characters) and no
    /// language list (DeepL's keeps growing).
    fn capabilities(&self) -> TranslationCapabilities {
        TranslationCapabilities {
            detects_language: true,
            ..TranslationCapabilities::default()
        }
    }
}

/// The API base URL: `endpoint` if set (trailing `/` removed), otherwise by key type.
fn base_url(key: &str, endpoint: Option<&str>) -> Result<String, Error> {
    match endpoint.map(str::trim).filter(|e| !e.is_empty()) {
        Some(endpoint) => {
            let valid =
                url::Url::parse(endpoint).is_ok_and(|url| matches!(url.scheme(), "http" | "https"));
            if !valid {
                return Err(Error::InvalidOptions(format!(
                    "`endpoint` must be an http(s) URL (got `{endpoint}`)"
                )));
            }
            Ok(endpoint.trim_end_matches('/').to_string())
        }
        None if key.ends_with(FREE_KEY_SUFFIX) => Ok(FREE_API_URL.to_string()),
        None => Ok(PRO_API_URL.to_string()),
    }
}

/// The JSON body of a `/v2/translate` request; `source_lang` is left out for `"auto"`.
fn build_request_body(text: &str, from: &str, to: &str) -> Value {
    let mut body = json!({
        "text": [text],
        "target_lang": to_deepl_target(to),
    });
    if let Some(source) = to_deepl_source(from) {
        body["source_lang"] = Value::String(source);
    }
    body
}

/// Parses a `/v2/translate` response into the (single) translation and the detected
/// source language, if reported.
fn parse_response(body: &[u8]) -> Result<(String, Option<String>), Error> {
    let json: Value = serde_json::from_slice(body)?;
    let first = json
        .get("translations")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .ok_or_else(|| Error::Decode("DeepL response has no translations".to_string()))?;
    let text = first
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Decode("DeepL translation has no `text`".to_string()))?;
    let detected = first
        .get("detected_source_language")
        .and_then(Value::as_str)
        .filter(|code| !code.is_empty())
        .map(str::to_string);
    Ok((text.to_string(), detected))
}

/// Splits a BCP-47 code into lowercase subtags (`-` or `_` separated).
fn subtags(code: &str) -> Vec<String> {
    code.trim()
        .split(['-', '_'])
        .filter(|tag| !tag.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// A source language for DeepL: the primary subtag, uppercased; `None` for `"auto"`
/// (DeepL then detects it).
fn to_deepl_source(code: &str) -> Option<String> {
    let tags = subtags(code);
    match tags.first().map(String::as_str) {
        None | Some("auto") => None,
        Some(primary) => Some(primary.to_ascii_uppercase()),
    }
}

/// A target language for DeepL: Chinese by script (`ZH-HANS` / `ZH-HANT`), everything
/// else uppercased with its region (`pt-BR` → `PT-BR`, `en` → `EN`).
fn to_deepl_target(code: &str) -> String {
    let tags = subtags(code);
    if tags.first().map(String::as_str) == Some("zh") {
        let traditional = tags[1..]
            .iter()
            .any(|tag| matches!(tag.as_str(), "hant" | "tw" | "hk" | "mo"));
        return if traditional { "ZH-HANT" } else { "ZH-HANS" }.to_string();
    }
    tags.join("-").to_ascii_uppercase()
}

/// A DeepL language code as BCP-47: the lowercased primary subtag (`EN-GB` → `en`).
fn from_deepl(code: &str) -> String {
    subtags(code).into_iter().next().unwrap_or_default()
}

/// The first [`DETECTION_PREFIX_CHARS`] characters of `text`, cut on a char boundary.
fn detection_prefix(text: &str) -> &str {
    match text.char_indices().nth(DETECTION_PREFIX_CHARS) {
        Some((cut, _)) => &text[..cut],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::create_provider_with;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // --- Pure functions ------------------------------------------------------------

    #[test]
    fn request_body_with_and_without_source() {
        assert_eq!(
            build_request_body("Hello", "en", "de"),
            json!({"text": ["Hello"], "target_lang": "DE", "source_lang": "EN"})
        );
        assert_eq!(
            build_request_body("Hello", "auto", "pt-BR"),
            json!({"text": ["Hello"], "target_lang": "PT-BR"})
        );
    }

    #[test]
    fn response_parsing() {
        let ok = br#"{"translations":[{"detected_source_language":"EN","text":"Hallo"}]}"#;
        assert_eq!(
            parse_response(ok).unwrap(),
            ("Hallo".to_string(), Some("EN".to_string()))
        );
        let no_detected = br#"{"translations":[{"text":"Hallo"}]}"#;
        assert_eq!(parse_response(no_detected).unwrap().1, None);
        for bad in [
            &br#"{}"#[..],
            br#"{"translations":[]}"#,
            br#"{"translations":[{"detected_source_language":"EN"}]}"#,
            br#"not json"#,
        ] {
            assert!(
                matches!(parse_response(bad), Err(Error::Decode(_))),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn source_codes() {
        for (bcp47, deepl) in [
            ("en", Some("EN")),
            ("en-US", Some("EN")),
            ("zh-TW", Some("ZH")),
            ("PT_br", Some("PT")),
            ("auto", None),
            ("AUTO", None),
            ("", None),
        ] {
            assert_eq!(to_deepl_source(bcp47).as_deref(), deepl, "{bcp47}");
        }
    }

    #[test]
    fn target_codes() {
        for (bcp47, deepl) in [
            ("zh", "ZH-HANS"),
            ("zh-CN", "ZH-HANS"),
            ("zh-Hans", "ZH-HANS"),
            ("zh-SG", "ZH-HANS"),
            ("zh-TW", "ZH-HANT"),
            ("zh-HK", "ZH-HANT"),
            ("zh-Hant", "ZH-HANT"),
            ("zh-Hant-TW", "ZH-HANT"),
            ("pt-BR", "PT-BR"),
            ("en-GB", "EN-GB"),
            ("es-419", "ES-419"),
            ("en_us", "EN-US"),
            ("en", "EN"),
            ("pt", "PT"),
            ("ru", "RU"),
        ] {
            assert_eq!(to_deepl_target(bcp47), deepl, "{bcp47}");
        }
    }

    #[test]
    fn codes_back_from_deepl() {
        assert_eq!(from_deepl("EN"), "en");
        assert_eq!(from_deepl("ZH"), "zh");
        assert_eq!(from_deepl("EN-GB"), "en");
    }

    #[test]
    fn base_url_by_key_and_endpoint() {
        assert_eq!(base_url("abc:fx", None).unwrap(), FREE_API_URL);
        assert_eq!(base_url("abc", None).unwrap(), PRO_API_URL);
        assert_eq!(base_url("abc:fx", Some("  ")).unwrap(), FREE_API_URL);
        assert_eq!(
            base_url("abc:fx", Some("http://localhost:8080/")).unwrap(),
            "http://localhost:8080"
        );
        for bad in ["api.deepl.com", "ftp://api.deepl.com"] {
            assert!(
                matches!(base_url("abc", Some(bad)), Err(Error::InvalidOptions(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn key_is_trimmed_before_choosing_the_url() {
        let options = ProviderOptions::new().with("api_key", " abc:fx \n");
        let deepl = DeepLTranslateProvider::with_options(&options).unwrap();
        assert_eq!(deepl.url, "https://api-free.deepl.com/v2/translate");
        let options = ProviderOptions::new().with("api_key", "abc");
        let deepl = DeepLTranslateProvider::with_options(&options).unwrap();
        assert_eq!(deepl.url, "https://api.deepl.com/v2/translate");
    }

    #[test]
    fn missing_key_is_invalid_options() {
        for options in [
            ProviderOptions::new(),
            ProviderOptions::new().with("api_key", "   "),
        ] {
            assert!(matches!(
                DeepLTranslateProvider::with_options(&options),
                Err(Error::InvalidOptions(_))
            ));
        }
        let options = ProviderOptions::new().with("api_key", "bad\nkey");
        assert!(matches!(
            DeepLTranslateProvider::with_options(&options),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[test]
    fn capabilities_report_detection_only() {
        let deepl = provider("key:fx", "http://localhost");
        let capabilities = deepl.capabilities();
        assert!(capabilities.detects_language);
        assert_eq!(capabilities.max_text_len, None);
        assert_eq!(capabilities.languages, None);
    }

    #[test]
    fn detection_prefix_cuts_on_a_char_boundary() {
        let text = "ж".repeat(150);
        let prefix = detection_prefix(&text);
        assert_eq!(prefix.chars().count(), DETECTION_PREFIX_CHARS);
        assert_eq!(prefix.len(), DETECTION_PREFIX_CHARS * 2);
        assert_eq!(detection_prefix("short"), "short");
    }

    // --- Factory -----------------------------------------------------------------------

    #[test]
    fn factory_builds_deepl_and_its_profiles() {
        let options = ProviderOptions::new().with("api_key", "k:fx");
        assert_eq!(
            create_provider_with("DeepL", &options).unwrap().name(),
            "DeepL"
        );
        let work = options.with("type", "deepl");
        let profiled = create_provider_with("work", &work).unwrap();
        assert_eq!(profiled.name(), "DeepL (work)");
        assert!(profiled.capabilities().detects_language);
        assert!(matches!(
            create_provider_with("deepl", &ProviderOptions::new()),
            Err(Error::InvalidOptions(_))
        ));
    }

    // --- Mock server ---------------------------------------------------------------

    const KEY: &str = "SENTINEL-DEEPL-KEY:fx";

    fn provider(key: &str, endpoint: &str) -> DeepLTranslateProvider {
        let options = ProviderOptions::new()
            .with("api_key", key)
            .with("endpoint", endpoint);
        DeepLTranslateProvider::with_options(&options).unwrap()
    }

    fn translated(text: &str, detected: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({
            "translations": [{"detected_source_language": detected, "text": text}]
        }))
    }

    async fn requests(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    #[tokio::test]
    async fn translate_sends_key_header_and_json_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .and(header(
                "authorization",
                format!("DeepL-Auth-Key {KEY}").as_str(),
            ))
            .and(body_json(
                json!({"text": ["Hello"], "target_lang": "DE", "source_lang": "EN"}),
            ))
            .respond_with(translated("Hallo", "EN"))
            .expect(1)
            .mount(&server)
            .await;
        // A trailing slash on the endpoint doesn't double the separator.
        let deepl = provider(KEY, &format!("{}/", server.uri()));
        assert_eq!(
            deepl.translate_text("Hello", "en-US", "de").await.unwrap(),
            "Hallo"
        );
    }

    #[tokio::test]
    async fn empty_text_makes_no_request() {
        let server = MockServer::start().await;
        let deepl = provider(KEY, &server.uri());
        assert!(matches!(
            deepl.translate_text("  ", "auto", "de").await,
            Err(Error::EmptyText)
        ));
        assert!(matches!(
            deepl.detect_language("").await,
            Err(Error::EmptyText)
        ));
        assert_eq!(requests(&server).await, 0);
    }

    #[tokio::test]
    async fn detect_language_round_trip() {
        let server = MockServer::start().await;
        let text = "Guten Tag ".repeat(30);
        Mock::given(method("POST"))
            .and(body_json(json!({
                "text": [detection_prefix(text.trim())],
                "target_lang": "EN",
            })))
            .respond_with(translated("Good day", "DE"))
            .expect(1)
            .mount(&server)
            .await;
        let deepl = provider(KEY, &server.uri());
        assert_eq!(deepl.detect_language(&text).await.unwrap(), "de");
    }

    #[tokio::test]
    async fn detect_language_without_detected_code_is_decode() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"translations": [{"text": "x"}]})),
            )
            .mount(&server)
            .await;
        let result = provider(KEY, &server.uri()).detect_language("text").await;
        assert!(matches!(result, Err(Error::Decode(_))), "{result:?}");
    }

    async fn failing(status: ResponseTemplate) -> (MockServer, Error) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(status)
            .mount(&server)
            .await;
        let error = provider(KEY, &server.uri())
            .translate_text("Hello", "en", "de")
            .await
            .unwrap_err();
        (server, error)
    }

    #[tokio::test]
    async fn status_403_is_auth_and_never_leaks_the_key() {
        let body = json!({"message": format!("Wrong key {KEY}")});
        let (server, error) = failing(ResponseTemplate::new(403).set_body_json(body)).await;
        assert!(matches!(error, Error::Auth(_)), "{error:?}");
        assert!(!error.to_string().contains(KEY), "{error}");
        assert!(!format!("{error:?}").contains(KEY), "{error:?}");
        assert_eq!(requests(&server).await, 1);
    }

    #[tokio::test]
    async fn status_456_is_quota_and_not_retried() {
        let body = json!({"message": "Quota exceeded"});
        let (server, error) = failing(ResponseTemplate::new(456).set_body_json(body)).await;
        assert!(
            matches!(&error, Error::QuotaExceeded(m) if m.contains("Quota exceeded")),
            "{error:?}"
        );
        assert_eq!(requests(&server).await, 1);
    }

    #[tokio::test]
    async fn status_400_carries_deepls_message() {
        let body = json!({"message": "Value for 'target_lang' not supported."});
        let (_, error) = failing(ResponseTemplate::new(400).set_body_json(body)).await;
        assert!(
            matches!(&error, Error::Api(m) if m.contains("target_lang")),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn throttling_without_retry_after_is_rate_limited() {
        for status in [429, 529] {
            let (server, error) = failing(ResponseTemplate::new(status)).await;
            assert!(
                matches!(error, Error::RateLimited { retry_after: None }),
                "{status}: {error:?}"
            );
            assert_eq!(requests(&server).await, 1, "{status}");
        }
    }

    /// Mounts `first` for the first request only, then a successful translation.
    async fn first_then_ok(first: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(first)
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(translated("Hallo", "EN"))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn throttling_with_short_retry_after_and_503_are_retried() {
        for first in [
            ResponseTemplate::new(429).insert_header("Retry-After", "0"),
            ResponseTemplate::new(529).insert_header("Retry-After", "0"),
            ResponseTemplate::new(503),
        ] {
            let server = first_then_ok(first).await;
            let deepl = provider(KEY, &server.uri());
            assert_eq!(
                deepl.translate_text("Hello", "en", "de").await.unwrap(),
                "Hallo"
            );
            assert_eq!(requests(&server).await, 2);
        }
    }

    #[tokio::test]
    async fn max_retries_zero_disables_retries() {
        let server = first_then_ok(ResponseTemplate::new(503)).await;
        let options = ProviderOptions::new()
            .with("api_key", KEY)
            .with("endpoint", server.uri())
            .with("max_retries", "0");
        let deepl = DeepLTranslateProvider::with_options(&options).unwrap();
        let error = deepl.translate_text("Hello", "en", "de").await.unwrap_err();
        assert!(
            matches!(&error, Error::Api(m) if m.contains("503")),
            "{error:?}"
        );
        assert_eq!(requests(&server).await, 1);
    }

    // --- Live ----------------------------------------------------------------------

    /// The live provider, if `TAGENT_LIVE_TESTS=1` and `TAGENT_DEEPL_API_KEY` are set.
    fn live() -> Option<Box<dyn TranslationProvider>> {
        if std::env::var("TAGENT_LIVE_TESTS").as_deref() != Ok("1") {
            eprintln!("skipped: set TAGENT_LIVE_TESTS=1 and TAGENT_DEEPL_API_KEY");
            return None;
        }
        let options = ProviderOptions::new().with_env_overrides("deepl");
        Some(create_provider_with("deepl", &options).expect("TAGENT_DEEPL_API_KEY is set"))
    }

    /// Run with:
    /// `TAGENT_LIVE_TESTS=1 TAGENT_DEEPL_API_KEY=... cargo test -p tagent --features deepl deepl::tests::live -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_translate_en_to_de() {
        let Some(deepl) = live() else { return };
        let text = deepl
            .translate_text("Good morning", "en", "de")
            .await
            .unwrap();
        println!("en → de: {text}");
        assert!(text.to_lowercase().contains("morgen"), "{text}");
    }

    /// See [`live_translate_en_to_de`].
    #[tokio::test]
    #[ignore]
    async fn live_translate_auto_to_ru() {
        let Some(deepl) = live() else { return };
        let text = deepl
            .translate_text("Good morning", "auto", "ru")
            .await
            .unwrap();
        println!("auto → ru: {text}");
        assert!(text.to_lowercase().contains("утр"), "{text}");
    }

    /// Also checks that English input still reports `EN` although the detection target is
    /// English as well (decision 1 of Stage P1).
    #[tokio::test]
    #[ignore]
    async fn live_detect_language() {
        let Some(deepl) = live() else { return };
        for (text, expected) in [
            ("The weather is nice today.", "en"),
            ("Das Wetter ist heute schön.", "de"),
            ("Сегодня хорошая погода.", "ru"),
        ] {
            assert_eq!(
                deepl.detect_language(text).await.unwrap(),
                expected,
                "{text}"
            );
        }
    }
}
