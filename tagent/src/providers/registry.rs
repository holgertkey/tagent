//! Static descriptions of the built-in providers: names, display names, the options each
//! accepts and their transport defaults.

#[cfg(feature = "google")]
use super::google::GOOGLE_TIMEOUT;
use std::time::Duration;

/// Static description of one built-in provider on one axis.
///
/// Returned by [`translation_providers`], [`dictionary_providers`] and
/// [`speech_providers`], so an application can build a picker or a settings form (one
/// field per [`OptionSpec`]) without hardcoding what each provider needs.
///
/// # Examples
///
/// ```
/// use tagent::providers::translation_providers;
///
/// for provider in translation_providers() {
///     println!("{} ({})", provider.display_name, provider.name);
///     for option in provider.options {
///         let mark = if option.required { " (required)" } else { "" };
///         println!("  {}{mark}: {}", option.key, option.description);
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProviderDescriptor {
    /// The canonical lowercase name, as passed to the factories (e.g. `"google"`). Built-in
    /// names are reserved: a profile may only use one with the same `type`.
    pub name: &'static str,
    /// The name the provider reports through `name()`, e.g. `"Google Translate"`.
    pub display_name: &'static str,
    /// The options the provider accepts, beyond the reserved `type`.
    pub options: &'static [OptionSpec],
    /// Timeout and retry defaults for the provider's requests.
    pub transport: TransportDefaults,
}

/// Timeout and retry defaults a provider ships with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TransportDefaults {
    /// How many times a failed request is retried (`0`: never).
    pub max_retries: u32,
    /// The time budget for one call, retries included.
    pub timeout: Duration,
    /// Whether an HTTP 429 answer with a short `Retry-After` is retried.
    pub retry_on_rate_limit: bool,
}

/// One option a provider accepts, e.g. an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct OptionSpec {
    /// The option key, lowercase (e.g. `"api_key"`), as used in
    /// [`ProviderOptions`](super::ProviderOptions).
    pub key: &'static str,
    /// Whether the provider can't be built without it.
    pub required: bool,
    /// Whether the value is a secret: a UI should mask it, and `Debug` output of
    /// [`ProviderOptions`](super::ProviderOptions) redacts it.
    pub secret: bool,
    /// A one-line description for a settings form or help text.
    pub description: &'static str,
    /// The value the provider uses when the option is unset, for a UI to show or start
    /// editing from (e.g. the built-in prompt of
    /// [`openai::DEFAULT_TRANSLATE_PROMPT`](super::openai::DEFAULT_TRANSLATE_PROMPT)).
    /// `None` when there is nothing useful to show; the transport options
    /// (`timeout_secs`, `max_retries`) take theirs from the descriptor's
    /// [`TransportDefaults`] instead.
    pub default: Option<&'static str>,
    /// Whether the value is usually several lines long (a prompt), a hint for a UI to offer
    /// a multi-line editor.
    pub multiline: bool,
}

/// The generic time-budget option (handled by the shared transport, not by the
/// provider's own code).
#[cfg(any(feature = "google", feature = "deepl", feature = "openai"))]
const TIMEOUT_SECS: OptionSpec = OptionSpec {
    key: "timeout_secs",
    required: false,
    secret: false,
    description: "Time budget for one call in whole seconds, retries included",
    default: None,
    multiline: false,
};

/// The generic retry-count option (handled by the shared transport).
#[cfg(any(feature = "google", feature = "deepl", feature = "openai"))]
const MAX_RETRIES: OptionSpec = OptionSpec {
    key: "max_retries",
    required: false,
    secret: false,
    description: "How often a failed request is retried; 0 disables retries",
    default: None,
    multiline: false,
};

/// The generic transport options every HTTP provider accepts.
#[cfg(feature = "google")]
const TRANSPORT_OPTIONS: &[OptionSpec] = &[TIMEOUT_SECS, MAX_RETRIES];

/// DeepL's options: its key, an endpoint override and the transport options.
#[cfg(feature = "deepl")]
const DEEPL_OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "api_key",
        required: true,
        secret: true,
        description: "DeepL authentication key (a Free key ends in `:fx`)",
        default: None,
        multiline: false,
    },
    OptionSpec {
        key: "endpoint",
        required: false,
        secret: false,
        description:
            "API base URL; default by key: https://api-free.deepl.com or https://api.deepl.com",
        default: None,
        multiline: false,
    },
    TIMEOUT_SECS,
    MAX_RETRIES,
];

/// The OpenAI-compatible server's base URL (both axes).
#[cfg(feature = "openai")]
const ENDPOINT: OptionSpec = OptionSpec {
    key: "endpoint",
    required: true,
    secret: false,
    description: "API base URL including /v1, e.g. http://localhost:11434/v1 (Ollama) or https://api.openai.com/v1",
    default: None,
    multiline: false,
};

/// The OpenAI-compatible model name (both axes).
#[cfg(feature = "openai")]
const MODEL: OptionSpec = OptionSpec {
    key: "model",
    required: true,
    secret: false,
    description: "Model name, e.g. qwen3:8b or gpt-4o-mini",
    default: None,
    multiline: false,
};

/// The OpenAI-compatible API key (both axes).
#[cfg(feature = "openai")]
const API_KEY: OptionSpec = OptionSpec {
    key: "api_key",
    required: false,
    secret: true,
    description: "API key, sent as a Bearer token; not needed for a local server",
    default: None,
    multiline: false,
};

/// The OpenAI-compatible sampling temperature (both axes).
#[cfg(feature = "openai")]
const TEMPERATURE: OptionSpec = OptionSpec {
    key: "temperature",
    required: false,
    secret: false,
    description: "Sampling temperature from 0 to 2; unset: the model's default",
    default: None,
    multiline: false,
};

/// The OpenAI-compatible chat options for translation: the server, the model, an optional
/// key, sampling, the prompt and the transport options.
#[cfg(feature = "openai")]
const OPENAI_TRANSLATION_OPTIONS: &[OptionSpec] = &[
    ENDPOINT,
    MODEL,
    API_KEY,
    TEMPERATURE,
    OptionSpec {
        key: "translate_prompt",
        required: false,
        secret: false,
        description: "System prompt for translations; {from} and {to} become language names",
        default: Some(super::openai::DEFAULT_TRANSLATE_PROMPT),
        multiline: true,
    },
    TIMEOUT_SECS,
    MAX_RETRIES,
];

/// The OpenAI-compatible chat options for dictionary lookups: as for translation, plus the
/// structured-output mode and the dictionary prompt.
#[cfg(feature = "openai")]
const OPENAI_DICTIONARY_OPTIONS: &[OptionSpec] = &[
    ENDPOINT,
    MODEL,
    API_KEY,
    TEMPERATURE,
    OptionSpec {
        key: "response_format",
        required: false,
        secret: false,
        description: "Dictionary answers: json_schema or json_object for structured output, if the server supports it; unset: not sent",
        default: None,
        multiline: false,
    },
    OptionSpec {
        key: "dictionary_prompt",
        required: false,
        secret: false,
        description: "System prompt for dictionary lookups; must keep asking for the same JSON answer shape; {from} and {to} become language names",
        default: Some(super::openai::DEFAULT_DICTIONARY_PROMPT),
        multiline: true,
    },
    TIMEOUT_SECS,
    MAX_RETRIES,
];

/// Google's unofficial free endpoint: one retry, but never on a 429, since insisting
/// risks captchas or IP blocks.
#[cfg(feature = "google")]
pub(crate) const GOOGLE_TRANSPORT: TransportDefaults = TransportDefaults {
    max_retries: 1,
    timeout: GOOGLE_TIMEOUT,
    retry_on_rate_limit: false,
};

/// DeepL's official API: two retries, including a 429/529 with a short `Retry-After`.
#[cfg(feature = "deepl")]
pub(crate) const DEEPL_TRANSPORT: TransportDefaults = TransportDefaults {
    max_retries: 2,
    timeout: Duration::from_secs(10),
    retry_on_rate_limit: true,
};

/// OpenAI-compatible chat servers: a model can take a while, so a long budget; one retry,
/// including a 429 with a short `Retry-After`.
#[cfg(feature = "openai")]
pub(crate) const OPENAI_TRANSPORT: TransportDefaults = TransportDefaults {
    max_retries: 1,
    timeout: Duration::from_secs(60),
    retry_on_rate_limit: true,
};

static TRANSLATION: &[ProviderDescriptor] = &[
    #[cfg(feature = "google")]
    ProviderDescriptor {
        name: "google",
        display_name: "Google Translate",
        options: TRANSPORT_OPTIONS,
        transport: GOOGLE_TRANSPORT,
    },
    #[cfg(feature = "deepl")]
    ProviderDescriptor {
        name: "deepl",
        display_name: "DeepL",
        options: DEEPL_OPTIONS,
        transport: DEEPL_TRANSPORT,
    },
    #[cfg(feature = "openai")]
    ProviderDescriptor {
        name: "openai",
        display_name: "OpenAI-compatible",
        options: OPENAI_TRANSLATION_OPTIONS,
        transport: OPENAI_TRANSPORT,
    },
];

static DICTIONARY: &[ProviderDescriptor] = &[
    #[cfg(feature = "google")]
    ProviderDescriptor {
        name: "google",
        display_name: "Google Dictionary",
        options: TRANSPORT_OPTIONS,
        transport: GOOGLE_TRANSPORT,
    },
    #[cfg(feature = "openai")]
    ProviderDescriptor {
        name: "openai",
        display_name: "OpenAI-compatible",
        options: OPENAI_DICTIONARY_OPTIONS,
        transport: OPENAI_TRANSPORT,
    },
];

static SPEECH: &[ProviderDescriptor] = &[
    #[cfg(feature = "google")]
    ProviderDescriptor {
        name: "google",
        display_name: "Google TTS",
        options: TRANSPORT_OPTIONS,
        transport: GOOGLE_TRANSPORT,
    },
];

/// The built-in translation providers, in the same order as
/// [`TRANSLATION_PROVIDERS`](super::TRANSLATION_PROVIDERS).
///
/// # Examples
///
/// ```
/// use tagent::providers::{translation_providers, TRANSLATION_PROVIDERS};
///
/// let names: Vec<_> = translation_providers().iter().map(|p| p.name).collect();
/// assert_eq!(names, TRANSLATION_PROVIDERS);
/// ```
pub fn translation_providers() -> &'static [ProviderDescriptor] {
    TRANSLATION
}

/// The built-in dictionary providers, in the same order as
/// [`DICTIONARY_PROVIDERS`](super::DICTIONARY_PROVIDERS).
pub fn dictionary_providers() -> &'static [ProviderDescriptor] {
    DICTIONARY
}

/// The built-in speech providers, in the same order as
/// [`SPEECH_PROVIDERS`](super::SPEECH_PROVIDERS).
pub fn speech_providers() -> &'static [ProviderDescriptor] {
    SPEECH
}

/// Whether option `key` of provider kind `kind` holds a secret that a UI should mask: the
/// kind declares it [`secret`](OptionSpec::secret) on any axis, or the key's name looks
/// like one (`api_key`, `token`, `password`, ...), the same rule `Debug` of
/// [`ProviderOptions`](super::ProviderOptions) uses. Both are case-insensitive.
///
/// # Examples
///
/// ```
/// use tagent::providers::is_secret_option;
///
/// assert!(is_secret_option("deepl", "api_key"));
/// assert!(is_secret_option("google", "API_KEY"));
/// assert!(!is_secret_option("google", "timeout_secs"));
/// ```
pub fn is_secret_option(kind: &str, key: &str) -> bool {
    let (kind, key) = (kind.to_lowercase(), key.to_lowercase());
    super::options::is_secret_key(&key)
        || [TRANSLATION, DICTIONARY, SPEECH]
            .iter()
            .flat_map(|list| list.iter())
            .filter(|descriptor| descriptor.name == kind)
            .flat_map(|descriptor| descriptor.options.iter())
            .any(|option| option.key == key && option.secret)
}

/// The option keys declared for provider kind `kind` (lowercase) on any axis.
pub(crate) fn declared_option_keys(kind: &str) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = [TRANSLATION, DICTIONARY, SPEECH]
        .iter()
        .flat_map(|list| list.iter())
        .filter(|descriptor| descriptor.name == kind)
        .flat_map(|descriptor| descriptor.options.iter().map(|option| option.key))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Options that satisfy every [`required`](OptionSpec::required) option of `descriptor`
/// with a dummy value (a URL for `endpoint`), so tests can build any listed provider
/// without a network call.
#[cfg(test)]
pub(crate) fn required_options(descriptor: &ProviderDescriptor) -> super::ProviderOptions {
    descriptor
        .options
        .iter()
        .filter(|option| option.required)
        .map(|option| match option.key {
            "endpoint" => (option.key, "http://localhost:1/v1"),
            _ => (option.key, "test-value"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "google", feature = "deepl", feature = "openai"))]
    use super::super::http::{MAX_RETRIES_KEY, TIMEOUT_SECS_KEY};
    use super::super::options::{is_secret_key, TYPE_KEY};
    use super::super::*;
    use super::*;

    fn axes() -> [(
        &'static str,
        &'static [ProviderDescriptor],
        &'static [&'static str],
    ); 3] {
        [
            ("translation", TRANSLATION, TRANSLATION_PROVIDERS),
            ("dictionary", DICTIONARY, DICTIONARY_PROVIDERS),
            ("speech", SPEECH, SPEECH_PROVIDERS),
        ]
    }

    /// Pickers are built from either the descriptors or the const lists, so both must
    /// agree, in the same order.
    #[test]
    fn descriptor_names_match_the_const_lists() {
        for (axis, descriptors, names) in axes() {
            let got: Vec<_> = descriptors.iter().map(|d| d.name).collect();
            assert_eq!(got, names, "{axis}");
        }
        assert_eq!(translation_providers(), TRANSLATION);
        assert_eq!(dictionary_providers(), DICTIONARY);
        assert_eq!(speech_providers(), SPEECH);
    }

    #[test]
    fn every_descriptor_builds_and_reports_its_display_name() {
        for d in translation_providers() {
            let options = required_options(d);
            assert_eq!(
                create_provider_with(d.name, &options).unwrap().name(),
                d.display_name
            );
        }
        for d in dictionary_providers() {
            let provider = create_dictionary_provider_with(d.name, &required_options(d)).unwrap();
            assert_eq!(provider.name(), d.display_name);
        }
        for d in speech_providers() {
            let provider = create_speech_provider_with(d.name, &required_options(d)).unwrap();
            assert_eq!(provider.name(), d.display_name);
        }
    }

    /// A provider with a required option can't be built without it.
    #[test]
    fn required_options_are_enforced() {
        for (axis, descriptors, _) in axes() {
            for d in descriptors
                .iter()
                .filter(|d| d.options.iter().any(|o| o.required))
            {
                let built = match axis {
                    "translation" => create_provider_with(d.name, &ProviderOptions::new()).err(),
                    "dictionary" => {
                        create_dictionary_provider_with(d.name, &ProviderOptions::new()).err()
                    }
                    _ => create_speech_provider_with(d.name, &ProviderOptions::new()).err(),
                };
                assert!(
                    matches!(built, Some(Error::InvalidOptions(_))),
                    "{axis}/{}: {built:?}",
                    d.name
                );
            }
        }
    }

    #[test]
    fn option_keys_are_lowercase_unique_and_not_reserved() {
        for (axis, descriptors, _) in axes() {
            for d in descriptors {
                for (i, option) in d.options.iter().enumerate() {
                    let at = format!("{axis}/{}/{}", d.name, option.key);
                    assert!(!option.key.is_empty(), "{at}");
                    assert_eq!(option.key, option.key.to_lowercase(), "{at}");
                    assert_ne!(option.key, TYPE_KEY, "{at}");
                    assert!(
                        !d.options[..i].iter().any(|o| o.key == option.key),
                        "{at}: duplicate"
                    );
                    assert!(!option.description.is_empty(), "{at}");
                    assert!(
                        option.default.is_none_or(|d| !d.trim().is_empty()),
                        "{at}: an empty default"
                    );
                    assert!(!option.secret || option.default.is_none(), "{at}");
                }
            }
        }
    }

    /// `ProviderOptions`' `Debug` redacts by key name; every declared secret must be
    /// caught by that rule.
    #[test]
    fn declared_secrets_are_redacted_by_debug() {
        for (axis, descriptors, _) in axes() {
            for d in descriptors {
                for option in d.options.iter().filter(|o| o.secret) {
                    assert!(
                        is_secret_key(option.key),
                        "{axis}/{}/{}: rename it or extend is_secret_key",
                        d.name,
                        option.key
                    );
                }
            }
        }
    }

    #[test]
    fn transport_defaults_are_sane() {
        for (axis, descriptors, _) in axes() {
            for d in descriptors {
                assert!(d.transport.timeout > Duration::ZERO, "{axis}/{}", d.name);
            }
        }
        // Google's free endpoint must never insist on a 429.
        #[cfg(feature = "google")]
        for d in [TRANSLATION, DICTIONARY, SPEECH].concat() {
            if d.name == "google" {
                assert!(!d.transport.retry_on_rate_limit);
            }
        }
    }

    #[test]
    fn declared_option_keys_of_unknown_kind_is_empty() {
        assert!(declared_option_keys("no-such-kind").is_empty());
        #[cfg(feature = "google")]
        assert_eq!(
            declared_option_keys("google"),
            [MAX_RETRIES_KEY, TIMEOUT_SECS_KEY]
        );
        #[cfg(feature = "deepl")]
        assert_eq!(
            declared_option_keys("deepl"),
            ["api_key", "endpoint", MAX_RETRIES_KEY, TIMEOUT_SECS_KEY]
        );
    }

    #[cfg(feature = "openai")]
    #[test]
    fn openai_needs_endpoint_and_model_and_offers_the_prompt() {
        let openai = translation_providers()
            .iter()
            .find(|d| d.name == "openai")
            .unwrap();
        assert_eq!(openai.display_name, "OpenAI-compatible");
        let option = |key: &str| openai.options.iter().find(|o| o.key == key).unwrap();
        assert!(option("endpoint").required && option("model").required);
        assert!(!option("api_key").required && option("api_key").secret);
        let prompt = option("translate_prompt");
        assert!(prompt.multiline && !prompt.required);
        assert_eq!(
            prompt.default,
            Some(super::super::openai::DEFAULT_TRANSLATE_PROMPT)
        );
        assert!(openai
            .options
            .iter()
            .all(|o| o.key == "translate_prompt" || (!o.multiline && o.default.is_none())));
        assert!(!openai.options.iter().any(|o| o.key == "dictionary_prompt"));
        assert_eq!(openai.transport.timeout, Duration::from_secs(60));
        assert_eq!(openai.transport.max_retries, 1);
        assert!(openai.transport.retry_on_rate_limit);
        assert_eq!(
            declared_option_keys("openai"),
            [
                "api_key",
                "dictionary_prompt",
                "endpoint",
                MAX_RETRIES_KEY,
                "model",
                "response_format",
                "temperature",
                TIMEOUT_SECS_KEY,
                "translate_prompt"
            ]
        );
        assert!(is_secret_option("openai", "api_key"));
        assert!(!is_secret_option("openai", "translate_prompt"));
    }

    /// The dictionary axis shares the server options and the transport, and has its own
    /// prompt and `response_format` instead of `translate_prompt`.
    #[cfg(feature = "openai")]
    #[test]
    fn openai_dictionary_offers_its_prompt_and_response_format() {
        let openai = dictionary_providers()
            .iter()
            .find(|d| d.name == "openai")
            .unwrap();
        assert_eq!(openai.display_name, "OpenAI-compatible");
        assert_eq!(openai.transport, OPENAI_TRANSPORT);
        let option = |key: &str| openai.options.iter().find(|o| o.key == key).unwrap();
        assert!(option("endpoint").required && option("model").required);
        assert!(option("api_key").secret && !option("api_key").required);
        let prompt = option("dictionary_prompt");
        assert!(prompt.multiline && !prompt.required && !prompt.secret);
        assert_eq!(
            prompt.default,
            Some(super::super::openai::DEFAULT_DICTIONARY_PROMPT)
        );
        let format = option("response_format");
        assert!(!format.required && !format.multiline && format.default.is_none());
        assert!(format.description.contains("json_schema"));
        assert!(format.description.contains("json_object"));
        assert!(!openai.options.iter().any(|o| o.key == "translate_prompt"));
        // The shared options are the same specs on both axes.
        let translation = translation_providers()
            .iter()
            .find(|d| d.name == "openai")
            .unwrap();
        for key in ["endpoint", "model", "api_key", "temperature"] {
            assert_eq!(
                translation.options.iter().find(|o| o.key == key),
                Some(option(key)),
                "{key}"
            );
        }
    }

    #[cfg(feature = "deepl")]
    #[test]
    fn deepl_key_is_a_required_secret() {
        let deepl = translation_providers()
            .iter()
            .find(|d| d.name == "deepl")
            .unwrap();
        let api_key = deepl.options.iter().find(|o| o.key == "api_key").unwrap();
        assert!(api_key.required && api_key.secret);
        assert!(deepl.transport.retry_on_rate_limit);
        assert!(is_secret_option("DeepL", "API_KEY"));
        assert!(!is_secret_option("deepl", "endpoint"));
    }
}
