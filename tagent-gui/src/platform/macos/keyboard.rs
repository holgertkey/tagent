use crate::config::HotkeyType;

/// Global hotkey listener for macOS. Currently a stub: hotkeys are not detected.
pub struct KeyboardHook;

impl KeyboardHook {
    /// Always a no-op on macOS: not yet implemented. Every parameter is accepted for
    /// API parity with Linux/Windows (Stage 10 follow-up added the speech hotkey and
    /// Escape callback) but unused.
    pub fn spawn(
        _translate_hotkey: HotkeyType,
        _speech_hotkey: Option<HotkeyType>,
        _on_translate_trigger: impl Fn() + Send + Sync + 'static,
        _on_speech_trigger: impl Fn() + Send + Sync + 'static,
        _on_escape: impl Fn() + Send + Sync + 'static,
    ) {
        eprintln!("Global hotkeys not yet implemented for macOS.");
    }
}
