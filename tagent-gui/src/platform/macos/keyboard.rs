use crate::config::HotkeyType;

/// Global hotkey listener for macOS. Currently a stub: hotkeys are not detected.
pub struct KeyboardHook;

impl KeyboardHook {
    /// Always a no-op on macOS: not yet implemented. `hotkey` and `on_trigger`
    /// are accepted for API parity with Linux/Windows but unused.
    pub fn spawn(_hotkey: HotkeyType, _on_trigger: impl Fn() + Send + Sync + 'static) {
        eprintln!("Global hotkeys not yet implemented for macOS.");
    }
}
