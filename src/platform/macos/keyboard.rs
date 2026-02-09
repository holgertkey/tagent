use crate::config::ConfigManager;
use crate::translator::Translator;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct KeyboardHook {
    should_exit: Arc<AtomicBool>,
}

impl KeyboardHook {
    pub fn new(
        _translator: Translator,
        should_exit: Arc<AtomicBool>,
        _config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self { should_exit })
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
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
