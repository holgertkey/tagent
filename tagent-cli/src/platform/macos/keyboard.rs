use crate::config::ConfigManager;
use crate::translator::Translator;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Global hotkey listener for macOS. Currently a stub: hotkeys are not detected,
/// so only interactive/CLI mode works — [`KeyboardHook::start`] just waits for exit.
pub struct KeyboardHook {
    should_exit: Arc<AtomicBool>,
}

impl KeyboardHook {
    /// Create a new hook. The translator and config manager are accepted for API parity
    /// with other platforms but currently unused, since hotkeys are not implemented.
    pub fn new(
        _translator: Translator,
        should_exit: Arc<AtomicBool>,
        _config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self { should_exit })
    }

    /// Print a "not yet implemented" notice and block until `should_exit` is set.
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        eprintln!("Global hotkeys not yet implemented for macOS. Use interactive mode.");
        loop {
            if self.should_exit.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }
}

/// What the banner's "Active Hotkeys" shows: always the configured hotkeys here (the
/// desktop owns them only on Wayland).
pub fn hotkey_banner() -> crate::platform::HotkeyBanner {
    crate::platform::HotkeyBanner::Configured
}

/// [`hotkey_banner`], with nothing to wait for here.
pub async fn wait_for_hotkey_banner(
    _timeout: std::time::Duration,
) -> crate::platform::HotkeyBanner {
    hotkey_banner()
}
