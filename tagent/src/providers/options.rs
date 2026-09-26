//! Provider options: credentials, endpoints, models and other settings passed into the
//! `*_with` factories, and the shared environment-variable naming rule.

use crate::error::Error;
use std::collections::BTreeMap;
use std::fmt;

/// The reserved option key that selects a profile's provider kind; see
/// [`create_provider_with`](super::create_provider_with).
pub(crate) const TYPE_KEY: &str = "type";

/// Options for constructing a provider: credentials, endpoint, model, timeout, ...
///
/// A string map rather than a typed struct per provider, so an application can forward
/// the `key = value` pairs of a config-file section without knowing which provider they
/// are for. Keys are case-insensitive (stored lowercase); values are kept as given.
///
/// The key `type` is reserved: it picks the provider kind of a named profile and is never
/// passed on to the provider itself (see [`create_provider_with`](super::create_provider_with)).
///
/// `Debug` output redacts the values of secret-looking keys (`api_key`, `token`, ...), so
/// options can be logged without leaking credentials.
///
/// # Examples
///
/// ```
/// use tagent::providers::ProviderOptions;
///
/// let options = ProviderOptions::new()
///     .with("API_KEY", "abc123")
///     .with("endpoint", "https://api.example.com");
/// assert_eq!(options.get("api_key"), Some("abc123"));
/// assert!(options.require("model").is_err());
/// assert!(!format!("{options:?}").contains("abc123"));
///
/// // Or collect them from a config section's key/value pairs.
/// let options: ProviderOptions = [("model", "gpt-4o-mini")].into_iter().collect();
/// assert_eq!(options.get("model"), Some("gpt-4o-mini"));
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ProviderOptions {
    values: BTreeMap<String, String>,
}

impl ProviderOptions {
    /// Creates an empty set of options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns these options with `key` set to `value` (builder style).
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.insert(key, value);
        self
    }

    /// Sets `key` to `value`, replacing any previous value.
    pub fn insert(&mut self, key: &str, value: impl Into<String>) {
        self.values.insert(key.to_lowercase(), value.into());
    }

    /// Removes `key` and returns its value, if it was set.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.values.remove(&key.to_lowercase())
    }

    /// Returns the value of `key`, if set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(&key.to_lowercase()).map(String::as_str)
    }

    /// Returns the value of a required option.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidOptions`] if `key` is unset, empty or only whitespace. The
    /// message names the key, never a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use tagent::error::Error;
    /// use tagent::providers::ProviderOptions;
    ///
    /// let options = ProviderOptions::new().with("api_key", "  ");
    /// assert!(matches!(options.require("api_key"), Err(Error::InvalidOptions(_))));
    /// ```
    pub fn require(&self, key: &str) -> Result<&str, Error> {
        match self.get(key) {
            Some(value) if !value.trim().is_empty() => Ok(value),
            _ => Err(Error::InvalidOptions(format!(
                "missing required option `{}`",
                key.to_lowercase()
            ))),
        }
    }

    /// Returns `true` if no option is set.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over the options as `(key, value)` pairs, keys in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns these options with values overridden from `TAGENT_<PROVIDER>_<KEY>`
    /// environment variables (named by [`env_var_name`]).
    ///
    /// `provider` is the profile name the options belong to, as passed to the `*_with`
    /// factories. The keys looked up are the ones already set, `api_key`, and every option
    /// the profile's provider kind declares (its [`OptionSpec`](super::OptionSpec)s; the
    /// kind is the `type` option, or `provider` itself without one). So a key can come from
    /// the environment alone, the usual way to keep it out of a config file. The reserved
    /// `type` key is never overridden: environment variables supply
    /// credentials and settings, not the choice of provider. A variable that is unset,
    /// empty or not valid Unicode is ignored, so `TAGENT_X_API_KEY=` does not blank a key
    /// from the config file.
    ///
    /// # Examples
    ///
    /// ```
    /// use tagent::providers::ProviderOptions;
    ///
    /// // (Setting variables is only for the example; normally they come from the shell.)
    /// std::env::set_var("TAGENT_DOC_EXAMPLE_API_KEY", "from-env");
    /// let options = ProviderOptions::new()
    ///     .with("api_key", "from-file")
    ///     .with_env_overrides("doc-example");
    /// assert_eq!(options.get("api_key"), Some("from-env"));
    /// ```
    pub fn with_env_overrides(self, provider: &str) -> Self {
        let kind = match self.get(TYPE_KEY) {
            Some(kind) => kind.trim().to_lowercase(),
            None => provider.to_lowercase(),
        };
        let declared = super::registry::declared_option_keys(&kind);
        self.with_overrides_from(provider, &declared, |name| std::env::var(name).ok())
    }

    /// [`with_env_overrides`](Self::with_env_overrides) with the kind's `declared` option
    /// keys and the variable lookup injected, so the rule can be tested without touching
    /// the process environment.
    pub(crate) fn with_overrides_from(
        mut self,
        provider: &str,
        declared: &[&str],
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let mut keys: Vec<String> = self.values.keys().cloned().collect();
        keys.push("api_key".to_string());
        keys.extend(declared.iter().map(|key| key.to_string()));
        keys.sort_unstable();
        keys.dedup();
        for key in keys.into_iter().filter(|key| key != TYPE_KEY) {
            if let Some(value) = lookup(&env_var_name(provider, &key)) {
                if !value.is_empty() {
                    self.values.insert(key, value);
                }
            }
        }
        self
    }
}

impl<K: AsRef<str>, V: Into<String>> FromIterator<(K, V)> for ProviderOptions {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut options = Self::new();
        for (key, value) in iter {
            options.insert(key.as_ref(), value);
        }
        options
    }
}

impl fmt::Debug for ProviderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (key, value) in &self.values {
            if is_secret_key(key) {
                map.entry(key, &"<redacted>");
            } else {
                map.entry(key, value);
            }
        }
        map.finish()
    }
}

/// Whether an option key looks like it holds a secret, for redaction.
///
/// A name heuristic, since `Debug` doesn't know which provider kind the options are for; a
/// test checks that it catches every option a descriptor declares `secret`.
pub(crate) fn is_secret_key(key: &str) -> bool {
    ["key", "secret", "token", "password", "auth", "credential"]
        .iter()
        .any(|part| key.contains(part))
}

/// Returns the environment variable that overrides option `key` of profile `provider`.
///
/// The format is `TAGENT_<PROVIDER>_<KEY>`, uppercase, with every character that isn't an
/// ASCII letter or digit replaced by `_`. The `TAGENT_` prefix is shared by all
/// applications built on this library, so one variable serves all of them.
///
/// # Examples
///
/// ```
/// use tagent::providers::env_var_name;
///
/// assert_eq!(env_var_name("deepl", "api_key"), "TAGENT_DEEPL_API_KEY");
/// assert_eq!(env_var_name("work-deepl", "api_key"), "TAGENT_WORK_DEEPL_API_KEY");
/// ```
pub fn env_var_name(provider: &str, key: &str) -> String {
    let normalize = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("TAGENT_{}_{}", normalize(provider), normalize(key))
}

/// Whether `name` is a valid profile name: non-empty, only `a-z`, `0-9`, `_` and `-`
/// once lowercased (in particular no `:`, so a `[Provider:<name>]` section parses
/// unambiguously).
pub(crate) fn is_valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .to_lowercase()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_case_insensitive_values_kept() {
        let mut options = ProviderOptions::new().with("Api_Key", "MiXeD");
        assert_eq!(options.get("API_KEY"), Some("MiXeD"));
        assert_eq!(options.iter().collect::<Vec<_>>(), [("api_key", "MiXeD")]);
        assert_eq!(options.remove("api_KEY").as_deref(), Some("MiXeD"));
        assert!(options.is_empty());
    }

    #[test]
    fn require_missing_or_blank_is_invalid_options() {
        let options = ProviderOptions::new()
            .with("model", "m")
            .with("api_key", " \t");
        assert_eq!(options.require("model").unwrap(), "m");
        for key in ["api_key", "endpoint"] {
            match options.require(key) {
                Err(Error::InvalidOptions(message)) => assert!(message.contains(key)),
                other => panic!("expected InvalidOptions for {key}, got {other:?}"),
            }
        }
    }

    #[test]
    fn debug_redacts_secret_values() {
        let sentinel = "SENTINEL-9f2c";
        let options = ProviderOptions::new()
            .with("api_key", sentinel)
            .with("access_token", sentinel)
            .with("client_secret", sentinel)
            .with("password", sentinel)
            .with("model", "visible-model");
        let debug = format!("{options:?}");
        assert!(!debug.contains(sentinel), "{debug}");
        assert!(debug.contains("visible-model"), "{debug}");
        assert!(debug.contains("api_key"), "{debug}");
    }

    #[test]
    fn from_iterator_lowercases_keys() {
        let map: BTreeMap<String, String> =
            [("Endpoint".to_string(), "http://x".to_string())].into();
        let options: ProviderOptions = map.into_iter().collect();
        assert_eq!(options.get("endpoint"), Some("http://x"));
    }

    #[test]
    fn env_var_name_normalization() {
        assert_eq!(env_var_name("deepl", "api_key"), "TAGENT_DEEPL_API_KEY");
        assert_eq!(
            env_var_name("Work-DeepL", "Api-Key"),
            "TAGENT_WORK_DEEPL_API_KEY"
        );
        assert_eq!(
            env_var_name("my_llm", "timeout_secs"),
            "TAGENT_MY_LLM_TIMEOUT_SECS"
        );
        assert_eq!(env_var_name("a.b c", "k"), "TAGENT_A_B_C_K");
    }

    fn fake_env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: BTreeMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| vars.get(name).cloned()
    }

    #[test]
    fn env_overrides_present_keys_and_api_key() {
        let options = ProviderOptions::new()
            .with("endpoint", "file-endpoint")
            .with("model", "file-model")
            .with_overrides_from(
                "ollama",
                &[],
                fake_env(&[
                    ("TAGENT_OLLAMA_ENDPOINT", "env-endpoint"),
                    ("TAGENT_OLLAMA_API_KEY", "env-key"),
                    // Not a present key and not api_key: ignored.
                    ("TAGENT_OLLAMA_TEMPERATURE", "0.9"),
                ]),
            );
        assert_eq!(options.get("endpoint"), Some("env-endpoint"));
        assert_eq!(options.get("model"), Some("file-model"));
        assert_eq!(options.get("api_key"), Some("env-key"));
        assert_eq!(options.get("temperature"), None);
    }

    #[test]
    fn env_override_unset_or_empty_keeps_file_value() {
        let options = ProviderOptions::new()
            .with("api_key", "file-key")
            .with_overrides_from("deepl", &[], fake_env(&[("TAGENT_DEEPL_API_KEY", "")]));
        assert_eq!(options.get("api_key"), Some("file-key"));

        let options = ProviderOptions::new()
            .with("api_key", "file-key")
            .with_overrides_from("deepl", &[], fake_env(&[]));
        assert_eq!(options.get("api_key"), Some("file-key"));
    }

    #[test]
    fn env_never_overrides_type() {
        let options = ProviderOptions::new()
            .with("type", "openai-compat")
            .with_overrides_from("ollama", &[], fake_env(&[("TAGENT_OLLAMA_TYPE", "deepl")]));
        assert_eq!(options.get("type"), Some("openai-compat"));
    }

    #[test]
    fn env_overrides_declared_keys() {
        let options = ProviderOptions::new().with_overrides_from(
            "ollama",
            &["endpoint", "model"],
            fake_env(&[
                ("TAGENT_OLLAMA_ENDPOINT", "env-endpoint"),
                ("TAGENT_OLLAMA_TEMPERATURE", "0.9"),
            ]),
        );
        assert_eq!(options.get("endpoint"), Some("env-endpoint"));
        assert_eq!(options.get("model"), None);
        assert_eq!(options.get("temperature"), None);
    }

    #[test]
    fn env_overrides_use_the_profile_name() {
        let options = ProviderOptions::new().with_overrides_from(
            "work-deepl",
            &[],
            fake_env(&[
                ("TAGENT_DEEPL_API_KEY", "wrong"),
                ("TAGENT_WORK_DEEPL_API_KEY", "right"),
            ]),
        );
        assert_eq!(options.get("api_key"), Some("right"));
    }

    #[test]
    fn profile_name_validation() {
        for ok in ["ollama", "work-deepl", "my_llm2", "Work"] {
            assert!(is_valid_profile_name(ok), "{ok}");
        }
        for bad in ["", "a:b", "bad name", "a.b", "ölm"] {
            assert!(!is_valid_profile_name(bad), "{bad}");
        }
    }
}
