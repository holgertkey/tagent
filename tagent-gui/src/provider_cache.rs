//! The Tokio runtime translations run on, and the translation/dictionary providers they
//! reuse.
//!
//! Both live for the whole process so a translation reuses the previous one's HTTP
//! connection: a provider owns its HTTP client, and that client's pooled connections
//! live on the runtime that opened them. Building a fresh provider on a fresh runtime
//! for every translation (as `spawn_translation` did before) paid a new TLS handshake
//! on every request -- about 150 ms per Google request instead of about 60 ms. reqwest
//! closes a connection idle for 90 s, so this helps translations made in a row, not
//! the first one after a pause.
//!
//! A cached provider is rebuilt only when its [`ProviderChoice`] (profile name and
//! effective options) changes, so a provider picked in the menu or edited in the config
//! file still applies to the next translation. A failed build is never cached.

use std::sync::{Arc, Mutex, OnceLock};

use tagent::error::Error;
use tagent::providers::{self, DictionaryProvider, TranslationProvider};
use tokio::runtime::Runtime;

use crate::config::ProviderChoice;

/// The process-wide runtime translations run on (two worker threads: the work is a
/// couple of concurrent HTTP requests at a time).
pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("tagent-net")
            .enable_all()
            .build()
            .expect("Failed to start Tokio runtime")
    })
}

/// The translation provider for `choice`, reused from the previous call when the
/// choice is unchanged.
pub fn translate_provider(choice: &ProviderChoice) -> Result<Arc<dyn TranslationProvider>, Error> {
    static SLOT: Mutex<Slot<dyn TranslationProvider>> = Mutex::new(Slot::new());
    SLOT.lock().unwrap().get_or_build(choice, || {
        providers::create_provider_with(&choice.name, &choice.options)
    })
}

/// The dictionary provider for `choice`, reused from the previous call when the choice
/// is unchanged.
pub fn dictionary_provider(choice: &ProviderChoice) -> Result<Arc<dyn DictionaryProvider>, Error> {
    static SLOT: Mutex<Slot<dyn DictionaryProvider>> = Mutex::new(Slot::new());
    SLOT.lock().unwrap().get_or_build(choice, || {
        providers::create_dictionary_provider_with(&choice.name, &choice.options)
    })
}

/// One cached provider and the choice it was built for.
struct Slot<P: ?Sized> {
    entry: Option<(ProviderChoice, Arc<P>)>,
}

impl<P: ?Sized> Slot<P> {
    const fn new() -> Self {
        Self { entry: None }
    }

    /// The cached provider if it was built for `choice`, otherwise a new one from
    /// `build`, which then replaces it. Building makes no network call, so it is fine
    /// under the slot's lock; the returned `Arc` is used after the lock is released.
    fn get_or_build(
        &mut self,
        choice: &ProviderChoice,
        build: impl FnOnce() -> Result<Box<P>, Error>,
    ) -> Result<Arc<P>, Error> {
        if let Some((cached_choice, provider)) = &self.entry {
            if cached_choice == choice {
                return Ok(provider.clone());
            }
        }
        let provider: Arc<P> = Arc::from(build()?);
        self.entry = Some((choice.clone(), provider.clone()));
        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use tagent::providers::ProviderOptions;

    fn choice(name: &str, options: ProviderOptions) -> ProviderChoice {
        ProviderChoice {
            name: name.to_string(),
            options,
        }
    }

    #[test]
    fn slot_reuses_the_provider_while_the_choice_is_unchanged() {
        let builds = Cell::new(0);
        let build = || {
            builds.set(builds.get() + 1);
            Ok(Box::<str>::from("provider"))
        };
        let mut slot = Slot::<str>::new();
        let google = choice("google", ProviderOptions::new());

        let first = slot.get_or_build(&google, build).unwrap();
        let second = slot.get_or_build(&google, build).unwrap();

        assert_eq!(builds.get(), 1);
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn slot_rebuilds_when_the_name_or_options_change() {
        let builds = Cell::new(0);
        let build = || {
            builds.set(builds.get() + 1);
            Ok(Box::<str>::from("provider"))
        };
        let mut slot = Slot::<str>::new();

        slot.get_or_build(&choice("google", ProviderOptions::new()), build)
            .unwrap();
        slot.get_or_build(&choice("work", ProviderOptions::new()), build)
            .unwrap();
        slot.get_or_build(
            &choice("work", ProviderOptions::new().with("timeout_secs", "5")),
            build,
        )
        .unwrap();

        assert_eq!(builds.get(), 3);
    }

    #[test]
    fn slot_never_caches_a_failed_build() {
        let mut slot = Slot::<str>::new();
        let google = choice("google", ProviderOptions::new());

        let failed = slot.get_or_build(&google, || {
            Err(Error::InvalidOptions("bad option".to_string()))
        });
        assert!(failed.is_err());

        let builds = Cell::new(0);
        slot.get_or_build(&google, || {
            builds.set(builds.get() + 1);
            Ok(Box::<str>::from("provider"))
        })
        .unwrap();
        assert_eq!(builds.get(), 1);
    }
}
