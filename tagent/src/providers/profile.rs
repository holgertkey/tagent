//! Profile resolution for the `*_with` factories, and the wrapper that puts a profile's
//! name into its provider's display name.

use super::options::{is_valid_profile_name, ProviderOptions, TYPE_KEY};
use super::{
    DictionaryEntry, DictionaryProvider, SpeechProvider, TranslationProvider, DICTIONARY_PROVIDERS,
    SPEECH_PROVIDERS, TRANSLATION_PROVIDERS,
};
use crate::error::Error;
use async_trait::async_trait;

/// A profile resolved to a provider kind.
#[derive(Debug)]
pub(crate) struct Resolved {
    /// The provider kind to build, lowercase (e.g. `"google"`).
    pub kind: String,
    /// The profile name, lowercase, when it differs from the kind (it then goes into the
    /// display name); `None` for a plain built-in name.
    pub label: Option<String>,
    /// The options for the provider, without the reserved `type` key.
    pub options: ProviderOptions,
}

/// Whether `name` (lowercase) is a built-in provider kind on any axis. Those names are
/// reserved: a profile may only use one for the kind of the same name.
fn is_builtin_kind(name: &str) -> bool {
    [
        TRANSLATION_PROVIDERS,
        DICTIONARY_PROVIDERS,
        SPEECH_PROVIDERS,
    ]
    .iter()
    .any(|list| list.contains(&name))
}

/// Resolves profile `name` with `options` to a kind among `axis_kinds` (the kinds that
/// implement the requested axis).
///
/// Without a `type` option, `name` is the kind, exactly as the name-only factories have
/// always treated it (an unknown or malformed name is `UnknownProvider`). With a `type`,
/// `name` is a real profile: it must be a valid profile name and may not be a built-in
/// kind other than its `type`.
pub(crate) fn resolve(
    name: &str,
    options: &ProviderOptions,
    axis_kinds: &[&str],
) -> Result<Resolved, Error> {
    let mut options = options.clone();
    let Some(kind_option) = options.remove(TYPE_KEY) else {
        let kind = name.to_lowercase();
        if !axis_kinds.contains(&kind.as_str()) {
            return Err(Error::UnknownProvider(name.to_string()));
        }
        return Ok(Resolved {
            kind,
            label: None,
            options,
        });
    };

    let kind = kind_option.trim().to_lowercase();
    if kind.is_empty() {
        return Err(Error::InvalidOptions(format!(
            "profile `{name}`: option `type` is empty"
        )));
    }
    if !is_valid_profile_name(name) {
        return Err(Error::InvalidOptions(format!(
            "invalid profile name `{name}`: use only letters a-z, digits, `_` and `-`"
        )));
    }
    let profile = name.to_lowercase();
    if is_builtin_kind(&profile) && profile != kind {
        return Err(Error::InvalidOptions(format!(
            "profile `{profile}` is named after a built-in provider, so its `type` can only be `{profile}`, not `{kind}`"
        )));
    }
    if !axis_kinds.contains(&kind.as_str()) {
        return Err(Error::UnknownProvider(kind_option.trim().to_string()));
    }
    let label = (profile != kind).then_some(profile);
    Ok(Resolved {
        kind,
        label,
        options,
    })
}

/// A provider built for a named profile: delegates everything to `inner` and reports
/// `"<inner name> (<profile>)"` as its name.
///
/// Every trait method must be forwarded here, **including methods with a default
/// implementation**: one left out would silently answer with the default instead of the
/// inner provider's own implementation.
pub(crate) struct Profiled<P: ?Sized> {
    inner: Box<P>,
    name: String,
}

impl<P: ?Sized> Profiled<P> {
    fn new(inner: Box<P>, inner_name: &str, profile: &str) -> Self {
        Self {
            name: format!("{inner_name} ({profile})"),
            inner,
        }
    }
}

/// Wraps `provider` in a [`Profiled`] when `label` is set.
pub(crate) fn label_translation(
    provider: Box<dyn TranslationProvider>,
    label: Option<String>,
) -> Box<dyn TranslationProvider> {
    match label {
        Some(profile) => {
            let inner_name = provider.name().to_string();
            Box::new(Profiled::new(provider, &inner_name, &profile))
        }
        None => provider,
    }
}

/// Wraps `provider` in a [`Profiled`] when `label` is set.
pub(crate) fn label_dictionary(
    provider: Box<dyn DictionaryProvider>,
    label: Option<String>,
) -> Box<dyn DictionaryProvider> {
    match label {
        Some(profile) => {
            let inner_name = provider.name().to_string();
            Box::new(Profiled::new(provider, &inner_name, &profile))
        }
        None => provider,
    }
}

/// Wraps `provider` in a [`Profiled`] when `label` is set.
pub(crate) fn label_speech(
    provider: Box<dyn SpeechProvider>,
    label: Option<String>,
) -> Box<dyn SpeechProvider> {
    match label {
        Some(profile) => {
            let inner_name = provider.name().to_string();
            Box::new(Profiled::new(provider, &inner_name, &profile))
        }
        None => provider,
    }
}

#[async_trait]
impl TranslationProvider for Profiled<dyn TranslationProvider> {
    async fn translate_text(&self, text: &str, from: &str, to: &str) -> Result<String, Error> {
        self.inner.translate_text(text, from, to).await
    }

    async fn detect_language(&self, text: &str) -> Result<String, Error> {
        self.inner.detect_language(text).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl DictionaryProvider for Profiled<dyn DictionaryProvider> {
    async fn lookup(
        &self,
        word: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<DictionaryEntry>, Error> {
        self.inner.lookup(word, from, to).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl SpeechProvider for Profiled<dyn SpeechProvider> {
    fn split_for_speech(&self, text: &str) -> Vec<String> {
        self.inner.split_for_speech(text)
    }

    async fn speak_chunk(&self, text: &str, lang: &str) -> Result<Vec<u8>, Error> {
        self.inner.speak_chunk(text, lang).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(kind: &str) -> ProviderOptions {
        ProviderOptions::new()
            .with("type", kind)
            .with("api_key", "k")
    }

    #[test]
    fn without_type_the_name_is_the_kind() {
        let resolved = resolve("Google", &ProviderOptions::new(), &["google"]).unwrap();
        assert_eq!(resolved.kind, "google");
        assert_eq!(resolved.label, None);
    }

    #[test]
    fn without_type_an_unknown_or_malformed_name_is_unknown_provider() {
        for name in ["deepl", "bad name!", "a:b", ""] {
            match resolve(name, &ProviderOptions::new(), &["google"]) {
                Err(Error::UnknownProvider(got)) => assert_eq!(got, name),
                other => panic!("{name}: expected UnknownProvider, got {other:?}"),
            }
        }
    }

    #[test]
    fn type_wins_and_is_not_forwarded() {
        let resolved = resolve("Work", &typed("Google"), &["google"]).unwrap();
        assert_eq!(resolved.kind, "google");
        assert_eq!(resolved.label.as_deref(), Some("work"));
        assert_eq!(resolved.options.get("type"), None);
        assert_eq!(resolved.options.get("api_key"), Some("k"));
    }

    #[test]
    fn builtin_name_with_its_own_type_is_fine() {
        let resolved = resolve("google", &typed("GOOGLE"), &["google"]).unwrap();
        assert_eq!(resolved.kind, "google");
        assert_eq!(resolved.label, None);
    }

    #[test]
    fn builtin_name_with_a_foreign_type_is_invalid() {
        assert!(matches!(
            resolve("google", &typed("deepl"), &["google", "deepl"]),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[test]
    fn invalid_profile_name_with_type_is_invalid() {
        for name in ["bad name!", "a:b", "work.deepl", ""] {
            assert!(
                matches!(
                    resolve(name, &typed("google"), &["google"]),
                    Err(Error::InvalidOptions(_))
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn empty_type_is_invalid() {
        assert!(matches!(
            resolve("work", &typed("  "), &["google"]),
            Err(Error::InvalidOptions(_))
        ));
    }

    #[test]
    fn unknown_type_is_unknown_provider() {
        match resolve("work", &typed(" Nope "), &["google"]) {
            Err(Error::UnknownProvider(kind)) => assert_eq!(kind, "Nope"),
            other => panic!("expected UnknownProvider, got {other:?}"),
        }
    }

    #[test]
    fn kind_lacking_the_axis_is_unknown_provider() {
        // "google" is a built-in kind, but not among this (hypothetical) axis's kinds.
        assert!(matches!(
            resolve("work", &typed("google"), &[]),
            Err(Error::UnknownProvider(_))
        ));
        assert!(matches!(
            resolve("google", &ProviderOptions::new(), &[]),
            Err(Error::UnknownProvider(_))
        ));
    }
}
