//! Shared HTTP transport for the providers: client setup, the time budget, retries, and
//! mapping of HTTP statuses onto [`Error`].
//!
//! # Retry policy
//!
//! Tagent is interactive, so an honest error beats a long wait:
//!
//! | Situation | Retried? |
//! |---|---|
//! | Connection failure (the request never reached the server) | yes, after a short backoff |
//! | HTTP 502 / 503 / 504 | yes, after a short backoff |
//! | HTTP 429 with `Retry-After` ≤ 2 s, if the provider allows it | yes, after that delay |
//! | HTTP 429 otherwise | no → [`Error::RateLimited`] |
//! | Timeout | no: the whole budget was spent waiting |
//! | HTTP 500, 401/403, quota, other 4xx | no |
//!
//! The whole call, retries included, fits into the provider's timeout: a retry is only
//! attempted when its delay plus a minimal attempt still fit into what is left. Retries
//! happen only here; adapters never retry on their own.

use super::options::ProviderOptions;
use super::TransportDefaults;
use crate::error::Error;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Client, RequestBuilder, StatusCode};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, Instant};

/// The option overriding a provider's retry count; `0` disables retries.
pub(crate) const MAX_RETRIES_KEY: &str = "max_retries";
/// The option overriding a provider's time budget, in whole seconds.
pub(crate) const TIMEOUT_SECS_KEY: &str = "timeout_secs";

/// The longest `Retry-After` a rate-limited request is retried after.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(2);
/// The longest response-body excerpt put into an error message, in characters.
const MAX_EXCERPT_CHARS: usize = 200;

/// Backoff and minimum-attempt timing; tests shrink it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timing {
    /// The backoff before a retry is chosen uniformly from this range.
    pub backoff_min: Duration,
    /// Upper end of the backoff range.
    pub backoff_max: Duration,
    /// A retry is skipped unless at least this much of the budget is left after its delay.
    pub min_attempt: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            backoff_min: Duration::from_millis(300),
            backoff_max: Duration::from_millis(600),
            min_attempt: Duration::from_secs(1),
        }
    }
}

/// An HTTP client with a provider's time budget, retry policy and status mapping.
pub(crate) struct HttpTransport {
    client: Client,
    max_retries: u32,
    timeout: Duration,
    retry_on_rate_limit: bool,
    auth_statuses: &'static [u16],
    quota_statuses: &'static [u16],
    /// Secrets sent in default headers, redacted from error messages.
    secrets: Vec<String>,
    timing: Timing,
}

impl std::fmt::Debug for HttpTransport {
    /// Leaves out the client and the secrets.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport")
            .field("max_retries", &self.max_retries)
            .field("timeout", &self.timeout)
            .field("retry_on_rate_limit", &self.retry_on_rate_limit)
            .field("auth_statuses", &self.auth_statuses)
            .field("quota_statuses", &self.quota_statuses)
            .finish_non_exhaustive()
    }
}

/// Builds an [`HttpTransport`].
pub(crate) struct HttpTransportBuilder {
    defaults: TransportDefaults,
    user_agent: Option<String>,
    headers: HeaderMap,
    secrets: Vec<String>,
    auth_statuses: &'static [u16],
    quota_statuses: &'static [u16],
    timing: Timing,
}

impl HttpTransport {
    /// Starts a transport with a provider's defaults. HTTP 401/403 map to
    /// [`Error::Auth`] unless [`auth_statuses`](HttpTransportBuilder::auth_statuses) says
    /// otherwise; no status maps to [`Error::QuotaExceeded`] unless configured.
    pub fn builder(defaults: TransportDefaults) -> HttpTransportBuilder {
        HttpTransportBuilder {
            defaults,
            user_agent: None,
            headers: HeaderMap::new(),
            secrets: Vec::new(),
            auth_statuses: &[401, 403],
            quota_statuses: &[],
            timing: Timing::default(),
        }
    }

    /// Sends the request `build` makes, retrying per the policy, and returns the body of
    /// the first successful (2xx) response.
    ///
    /// `build` is called once per attempt. Errors never contain the request URL (which
    /// can hold user text) or a sensitive header value.
    pub async fn send(&self, build: impl Fn(&Client) -> RequestBuilder) -> Result<Vec<u8>, Error> {
        let deadline = Instant::now() + self.timeout;
        let mut retries = 0;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(timed_out());
            }
            let (error, delay) = match self.attempt(build(&self.client).timeout(remaining)).await {
                Ok(body) => return Ok(body),
                Err(failure) => failure,
            };
            let Some(delay) = delay else {
                return Err(error);
            };
            let left = deadline.saturating_duration_since(Instant::now());
            if retries >= self.max_retries || left < delay + self.timing.min_attempt {
                return Err(error);
            }
            tokio::time::sleep(delay).await;
            retries += 1;
        }
    }

    /// One attempt: the body on success, otherwise the error and, if it may be retried,
    /// the delay before the retry.
    async fn attempt(&self, request: RequestBuilder) -> Result<Vec<u8>, (Error, Option<Duration>)> {
        let response = match request.send().await {
            Ok(response) => response,
            // A timed-out connect is both a timeout and a connect error: check the timeout
            // first, since the budget is spent.
            Err(e) if e.is_timeout() => return Err((timed_out(), None)),
            Err(e) if e.is_connect() => return Err((Error::from(e), Some(self.backoff()))),
            Err(e) => return Err((Error::from(e), None)),
        };
        let status = response.status();
        if status.is_success() {
            return match response.bytes().await {
                Ok(body) => Ok(body.to_vec()),
                Err(e) => Err((Error::from(e), None)),
            };
        }
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after);
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response.bytes().await.unwrap_or_default();

        let delay = match status.as_u16() {
            502..=504 => Some(self.backoff()),
            429 if self.retry_on_rate_limit => {
                retry_after.filter(|wait| *wait <= MAX_RATE_LIMIT_WAIT)
            }
            _ => None,
        };
        let error = status_error(
            status,
            retry_after,
            content_type.as_deref(),
            &body,
            self.auth_statuses,
            self.quota_statuses,
            &self.secrets,
        );
        Err((error, delay))
    }

    /// A random delay from the backoff range.
    fn backoff(&self) -> Duration {
        let Timing {
            backoff_min: min,
            backoff_max: max,
            ..
        } = self.timing;
        if max <= min {
            return min;
        }
        let span = (max - min).as_millis() as u64;
        let random = RandomState::new().build_hasher().finish();
        min + Duration::from_millis(random % (span + 1))
    }
}

impl HttpTransportBuilder {
    /// Sets the `User-Agent` header.
    pub fn user_agent(mut self, user_agent: &str) -> Self {
        self.user_agent = Some(user_agent.to_string());
        self
    }

    /// Adds a header carrying a secret, sent with every request: its value is `prefix`
    /// followed by `secret` (e.g. `"Bearer "` + an API key). The header is marked
    /// sensitive, and `secret` is redacted from error messages (servers sometimes echo it).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidOptions`] if the value isn't a valid header value.
    #[allow(dead_code)] // For keyed providers; Google sends no credentials.
    pub fn secret_header(
        mut self,
        name: &'static str,
        prefix: &str,
        secret: &str,
    ) -> Result<Self, Error> {
        let mut header = HeaderValue::from_str(&format!("{prefix}{secret}")).map_err(|_| {
            Error::InvalidOptions(format!(
                "the value for the `{name}` header contains invalid characters"
            ))
        })?;
        header.set_sensitive(true);
        self.headers.insert(HeaderName::from_static(name), header);
        self.secrets.push(secret.to_string());
        Ok(self)
    }

    /// Sets which statuses mean rejected credentials ([`Error::Auth`]); default 401/403.
    /// A provider without credentials passes `&[]`, so those statuses stay
    /// [`Error::Api`].
    pub fn auth_statuses(mut self, statuses: &'static [u16]) -> Self {
        self.auth_statuses = statuses;
        self
    }

    /// Sets which statuses mean the quota is used up ([`Error::QuotaExceeded`]), e.g.
    /// DeepL's 456.
    #[allow(dead_code)] // For keyed providers; Google has no quota status.
    pub fn quota_statuses(mut self, statuses: &'static [u16]) -> Self {
        self.quota_statuses = statuses;
        self
    }

    /// Replaces the backoff/minimum-attempt timing (tests only).
    #[cfg(test)]
    pub fn timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    /// Applies the generic `max_retries` / `timeout_secs` options, if set.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidOptions`] if `max_retries` isn't a non-negative integer or
    /// `timeout_secs` isn't a positive integer.
    pub fn options(mut self, options: &ProviderOptions) -> Result<Self, Error> {
        if let Some(value) = options.get(MAX_RETRIES_KEY) {
            self.defaults.max_retries = value.trim().parse::<u32>().map_err(|_| {
                Error::InvalidOptions(format!(
                    "`{MAX_RETRIES_KEY}` must be a whole number, 0 or more (got `{value}`)"
                ))
            })?;
        }
        if let Some(value) = options.get(TIMEOUT_SECS_KEY) {
            let secs = value
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|secs| *secs > 0)
                .ok_or_else(|| {
                    Error::InvalidOptions(format!(
                        "`{TIMEOUT_SECS_KEY}` must be a whole number of seconds, 1 or more (got `{value}`)"
                    ))
                })?;
            self.defaults.timeout = Duration::from_secs(secs);
        }
        Ok(self)
    }

    /// Builds the transport.
    ///
    /// # Errors
    ///
    /// [`Error::Network`] if the HTTP client can't be initialized.
    pub fn build(self) -> Result<HttpTransport, Error> {
        let mut client = Client::builder()
            .timeout(self.defaults.timeout)
            .default_headers(self.headers);
        if let Some(user_agent) = &self.user_agent {
            client = client.user_agent(user_agent.as_str());
        }
        Ok(HttpTransport {
            client: client.build()?,
            max_retries: self.defaults.max_retries,
            timeout: self.defaults.timeout,
            retry_on_rate_limit: self.defaults.retry_on_rate_limit,
            auth_statuses: self.auth_statuses,
            quota_statuses: self.quota_statuses,
            secrets: self.secrets,
            timing: self.timing,
        })
    }
}

fn timed_out() -> Error {
    Error::Network("request timed out".to_string())
}

/// Parses a `Retry-After` given in whole seconds; an HTTP date gives `None`.
fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Maps a non-success status (with its `Retry-After`, content type and body) onto an
/// [`Error`].
fn status_error(
    status: StatusCode,
    retry_after: Option<Duration>,
    content_type: Option<&str>,
    body: &[u8],
    auth_statuses: &[u16],
    quota_statuses: &[u16],
    secrets: &[String],
) -> Error {
    let code = status.as_u16();
    if code == 429 {
        return Error::RateLimited { retry_after };
    }
    let message = describe(status, content_type, body, secrets);
    if auth_statuses.contains(&code) {
        Error::Auth(message)
    } else if quota_statuses.contains(&code) {
        Error::QuotaExceeded(message)
    } else {
        Error::Api(message)
    }
}

/// `"HTTP 403 Forbidden"`, plus `": <excerpt>"` of a non-HTML body: whitespace collapsed,
/// at most [`MAX_EXCERPT_CHARS`] characters, secrets redacted.
fn describe(
    status: StatusCode,
    content_type: Option<&str>,
    body: &[u8],
    secrets: &[String],
) -> String {
    let mut message = format!("HTTP {status}");
    let is_html = content_type.is_some_and(|t| t.to_ascii_lowercase().contains("html"));
    let mut text = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if is_html || text.starts_with('<') || text.is_empty() {
        return message;
    }
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        text = text.replace(secret.as_str(), "<redacted>");
    }
    message.push_str(": ");
    match text.char_indices().nth(MAX_EXCERPT_CHARS) {
        Some((cut, _)) => {
            message.push_str(&text[..cut]);
            message.push('…');
        }
        None => message.push_str(&text),
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FAST: Timing = Timing {
        backoff_min: Duration::from_millis(10),
        backoff_max: Duration::from_millis(20),
        min_attempt: Duration::from_millis(50),
    };

    fn defaults(max_retries: u32, retry_on_rate_limit: bool) -> TransportDefaults {
        TransportDefaults {
            max_retries,
            timeout: Duration::from_secs(5),
            retry_on_rate_limit,
        }
    }

    fn transport(max_retries: u32, retry_on_rate_limit: bool) -> HttpTransport {
        HttpTransport::builder(defaults(max_retries, retry_on_rate_limit))
            .timing(FAST)
            .build()
            .unwrap()
    }

    /// Mounts `first` for the first request only, then 200 "ok" for every later one.
    async fn first_then_ok(first: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(first)
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        server
    }

    /// Asserts the total number of requests the server got.
    async fn assert_requests(server: &MockServer, expected: usize) {
        let got = server.received_requests().await.unwrap().len();
        assert_eq!(got, expected, "requests received");
    }

    async fn get(transport: &HttpTransport, url: &str) -> Result<Vec<u8>, Error> {
        transport.send(|client| client.get(url)).await
    }

    // --- Retry table ---------------------------------------------------------------

    #[tokio::test]
    async fn gateway_errors_are_retried_once() {
        for status in [502, 503, 504] {
            let server = first_then_ok(ResponseTemplate::new(status)).await;
            let body = get(&transport(1, false), &server.uri()).await.unwrap();
            assert_eq!(body, b"ok", "{status}");
            assert_requests(&server, 2).await;
        }
    }

    #[tokio::test]
    async fn non_retryable_statuses_are_sent_once() {
        for status in [400, 401, 403, 404, 456, 500] {
            let server = first_then_ok(ResponseTemplate::new(status)).await;
            assert!(
                get(&transport(2, true), &server.uri()).await.is_err(),
                "{status}"
            );
            assert_requests(&server, 1).await;
        }
    }

    #[tokio::test]
    async fn max_retries_bounds_the_attempts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let error = get(&transport(2, false), &server.uri()).await.unwrap_err();
        assert!(
            matches!(&error, Error::Api(m) if m.contains("503")),
            "{error:?}"
        );
        assert_requests(&server, 3).await;

        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert!(get(&transport(0, false), &server.uri()).await.is_err());
        assert_requests(&server, 1).await;
    }

    #[tokio::test]
    async fn short_retry_after_is_retried_when_allowed() {
        let rate_limited = || ResponseTemplate::new(429).insert_header("Retry-After", "0");

        let server = first_then_ok(rate_limited()).await;
        assert_eq!(
            get(&transport(1, true), &server.uri()).await.unwrap(),
            b"ok"
        );
        assert_requests(&server, 2).await;

        // Google's policy: never insist on a 429.
        let server = first_then_ok(rate_limited()).await;
        let error = get(&transport(1, false), &server.uri()).await.unwrap_err();
        assert!(
            matches!(error, Error::RateLimited { retry_after: Some(d) } if d.is_zero()),
            "{error:?}"
        );
        assert_requests(&server, 1).await;
    }

    #[tokio::test]
    async fn long_or_missing_retry_after_is_not_retried() {
        let server =
            first_then_ok(ResponseTemplate::new(429).insert_header("Retry-After", "5")).await;
        let error = get(&transport(2, true), &server.uri()).await.unwrap_err();
        assert!(
            matches!(error, Error::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(5)),
            "{error:?}"
        );
        assert_requests(&server, 1).await;

        for retry_after in [None, Some("Wed, 21 Oct 2015 07:28:00 GMT")] {
            let mut first = ResponseTemplate::new(429);
            if let Some(value) = retry_after {
                first = first.insert_header("Retry-After", value);
            }
            let server = first_then_ok(first).await;
            let error = get(&transport(2, true), &server.uri()).await.unwrap_err();
            assert!(
                matches!(error, Error::RateLimited { retry_after: None }),
                "{error:?}"
            );
            assert_requests(&server, 1).await;
        }
    }

    #[tokio::test]
    async fn connection_failures_are_retried() {
        // A port nothing listens on: connect is refused.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        drop(listener);

        // A large budget, so the attempt count depends on max_retries alone: on Windows a
        // refused localhost connect can take about 2 s (the SYN is re-sent after the RST).
        let transport = HttpTransport::builder(TransportDefaults {
            max_retries: 2,
            timeout: Duration::from_secs(30),
            retry_on_rate_limit: false,
        })
        .timing(FAST)
        .build()
        .unwrap();
        let attempts = AtomicUsize::new(0);
        let result = transport
            .send(|client| {
                attempts.fetch_add(1, Ordering::SeqCst);
                client.get(&url)
            })
            .await;
        assert!(matches!(result, Err(Error::Network(_))), "{result:?}");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn timeouts_are_not_retried() {
        // Accepts connections (in the kernel backlog) but never answers.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let transport = HttpTransport::builder(TransportDefaults {
            max_retries: 3,
            timeout: Duration::from_millis(300),
            retry_on_rate_limit: false,
        })
        .timing(FAST)
        .build()
        .unwrap();

        let attempts = AtomicUsize::new(0);
        let started = Instant::now();
        let result = transport
            .send(|client| {
                attempts.fetch_add(1, Ordering::SeqCst);
                client.get(&url)
            })
            .await;
        assert!(
            matches!(&result, Err(Error::Network(m)) if m == "request timed out"),
            "{result:?}"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(listener);
    }

    #[tokio::test]
    async fn the_budget_covers_all_retries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503).set_delay(Duration::from_millis(150)))
            .mount(&server)
            .await;
        let budget = Duration::from_millis(400);
        let transport = HttpTransport::builder(TransportDefaults {
            max_retries: 10,
            timeout: budget,
            retry_on_rate_limit: false,
        })
        .timing(FAST)
        .build()
        .unwrap();

        let started = Instant::now();
        assert!(get(&transport, &server.uri()).await.is_err());
        let elapsed = started.elapsed();
        // Generous margin for slow CI runners; the point is "not 10 × 150 ms".
        assert!(elapsed < budget + Duration::from_secs(1), "{elapsed:?}");
        let requests = server.received_requests().await.unwrap().len();
        assert!((1..=3).contains(&requests), "{requests} requests");
    }

    // --- Status mapping --------------------------------------------------------------

    fn map(code: u16, content_type: Option<&str>, body: &str) -> Error {
        status_error(
            StatusCode::from_u16(code).unwrap(),
            None,
            content_type,
            body.as_bytes(),
            &[401, 403],
            &[456],
            &[],
        )
    }

    #[test]
    fn statuses_map_onto_error_variants() {
        assert!(matches!(map(401, None, ""), Error::Auth(_)));
        assert!(matches!(map(403, None, ""), Error::Auth(_)));
        assert!(matches!(map(456, None, ""), Error::QuotaExceeded(_)));
        assert!(matches!(map(429, None, ""), Error::RateLimited { .. }));
        assert!(matches!(map(500, None, ""), Error::Api(_)));
        assert!(matches!(map(404, None, ""), Error::Api(_)));
        // Without credentials, 403 is just an API error.
        let no_auth = status_error(StatusCode::FORBIDDEN, None, None, b"", &[], &[], &[]);
        assert!(matches!(no_auth, Error::Api(_)));
    }

    #[test]
    fn messages_carry_a_short_non_html_excerpt() {
        assert_eq!(
            map(500, None, "").to_string(),
            "provider API error: HTTP 500 Internal Server Error"
        );
        assert_eq!(
            map(
                400,
                Some("application/json"),
                "{\"message\":\n  \"bad  lang\"}"
            )
            .to_string(),
            "provider API error: HTTP 400 Bad Request: {\"message\": \"bad lang\"}"
        );
        for html in [
            (Some("text/html; charset=UTF-8"), "Sorry..."),
            (None, "<html><body>x"),
        ] {
            assert_eq!(
                map(403, html.0, html.1).to_string(),
                "authentication failed: HTTP 403 Forbidden"
            );
        }
        // Truncated on a character boundary (multi-byte characters would panic a byte cut).
        let long = "ж".repeat(300);
        let Error::Api(message) = map(400, None, &long) else {
            panic!()
        };
        assert!(message.ends_with('…'));
        assert_eq!(
            message.chars().filter(|c| *c == 'ж').count(),
            MAX_EXCERPT_CHARS
        );
    }

    #[test]
    fn retry_after_parses_seconds_only() {
        assert_eq!(parse_retry_after(" 3 "), Some(Duration::from_secs(3)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after("-1"), None);
    }

    // --- Secrets ---------------------------------------------------------------------

    #[tokio::test]
    async fn errors_never_contain_the_key() {
        let sentinel = "SENTINEL-KEY-4711";
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_string(format!("{{\"error\":\"invalid key {sentinel}\"}}")),
            )
            .mount(&server)
            .await;
        let transport = HttpTransport::builder(defaults(1, true))
            .secret_header("authorization", "Key ", sentinel)
            .unwrap()
            .timing(FAST)
            .build()
            .unwrap();
        let error = get(&transport, &server.uri()).await.unwrap_err();
        assert!(matches!(error, Error::Auth(_)), "{error:?}");
        for text in [
            error.to_string(),
            format!("{error:?}"),
            format!("{transport:?}"),
        ] {
            assert!(!text.contains(sentinel), "{text}");
        }
        // The header did reach the server.
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0].headers["authorization"],
            format!("Key {sentinel}").as_str()
        );
    }

    #[tokio::test]
    async fn network_errors_never_contain_the_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/?q=SENTINEL-TEXT", listener.local_addr().unwrap());
        drop(listener);
        let error = get(&transport(0, false), &url).await.unwrap_err();
        assert!(matches!(error, Error::Network(_)), "{error:?}");
        assert!(!error.to_string().contains("SENTINEL"), "{error}");
    }

    // --- Options ---------------------------------------------------------------------

    #[test]
    fn options_override_the_defaults() {
        let options = ProviderOptions::new()
            .with("max_retries", "0")
            .with("timeout_secs", " 30 ");
        let transport = HttpTransport::builder(defaults(2, true))
            .options(&options)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(transport.max_retries, 0);
        assert_eq!(transport.timeout, Duration::from_secs(30));

        let untouched = HttpTransport::builder(defaults(2, true))
            .options(&ProviderOptions::new())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(untouched.max_retries, 2);
        assert_eq!(untouched.timeout, Duration::from_secs(5));
    }

    #[test]
    fn invalid_transport_options_are_rejected() {
        for (key, value) in [
            ("max_retries", "-1"),
            ("max_retries", "two"),
            ("max_retries", "1.5"),
            ("timeout_secs", "0"),
            ("timeout_secs", "-5"),
            ("timeout_secs", "ten"),
            ("timeout_secs", ""),
        ] {
            let options = ProviderOptions::new().with(key, value);
            let result = HttpTransport::builder(defaults(1, true)).options(&options);
            assert!(
                matches!(result, Err(Error::InvalidOptions(_))),
                "{key} = {value:?}"
            );
        }
    }
}
