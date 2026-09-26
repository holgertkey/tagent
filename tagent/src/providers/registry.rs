//! Static descriptions of the built-in providers: names, display names, the options each
//! accepts and their transport defaults.

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
}

/// The generic transport options every HTTP provider accepts (handled by the shared
/// transport, not by the provider's own code).
const TRANSPORT_OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "timeout_secs",
        required: false,
        secret: false,
        description: "Time budget for one call in whole seconds, retries included",
    },
    OptionSpec {
        key: "max_retries",
        required: false,
        secret: false,
        description: "How often a failed request is retried; 0 disables retries",
    },
];

/// Google's unofficial free endpoint: one retry, but never on a 429, since insisting
/// risks captchas or IP blocks.
pub(crate) const GOOGLE_TRANSPORT: TransportDefaults = TransportDefaults {
    max_retries: 1,
    timeout: GOOGLE_TIMEOUT,
    retry_on_rate_limit: false,
};

static TRANSLATION: &[ProviderDescriptor] = &[ProviderDescriptor {
    name: "google",
    display_name: "Google Translate",
    options: TRANSPORT_OPTIONS,
    transport: GOOGLE_TRANSPORT,
}];

static DICTIONARY: &[ProviderDescriptor] = &[ProviderDescriptor {
    name: "google",
    display_name: "Google Dictionary",
    options: TRANSPORT_OPTIONS,
    transport: GOOGLE_TRANSPORT,
}];

static SPEECH: &[ProviderDescriptor] = &[ProviderDescriptor {
    name: "google",
    display_name: "Google TTS",
    options: TRANSPORT_OPTIONS,
    transport: GOOGLE_TRANSPORT,
}];

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

#[cfg(test)]
mod tests {
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
        let options = ProviderOptions::new();
        for d in translation_providers() {
            assert_eq!(
                create_provider_with(d.name, &options).unwrap().name(),
                d.display_name
            );
        }
        for d in dictionary_providers() {
            let provider = create_dictionary_provider_with(d.name, &options).unwrap();
            assert_eq!(provider.name(), d.display_name);
        }
        for d in speech_providers() {
            let provider = create_speech_provider_with(d.name, &options).unwrap();
            assert_eq!(provider.name(), d.display_name);
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
        assert!(!translation_providers()[0].transport.retry_on_rate_limit);
    }

    #[test]
    fn declared_option_keys_of_unknown_kind_is_empty() {
        assert!(declared_option_keys("no-such-kind").is_empty());
        assert_eq!(
            declared_option_keys("google"),
            [MAX_RETRIES_KEY, TIMEOUT_SECS_KEY]
        );
    }
}
