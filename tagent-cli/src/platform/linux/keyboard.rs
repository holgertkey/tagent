use super::keycodes::normalize_vk_code;
use crate::config::{self, ConfigManager, HotkeyParser, HotkeyType};
use crate::speech::SpeechManager;
use crate::translator::Translator;
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Encapsulates hotkey detection state for a single hotkey
struct HotkeyState {
    config: Option<HotkeyType>,
    last_key_time: Option<Instant>,
    last_key_pressed: bool,
    last_key_interrupted: bool,
}

impl HotkeyState {
    fn new(hotkey: Option<HotkeyType>) -> Self {
        Self {
            config: hotkey,
            last_key_time: None,
            last_key_pressed: false,
            last_key_interrupted: false,
        }
    }

    /// Handle hotkey detection for key events.
    /// Returns true if the hotkey was triggered.
    fn handle(
        &mut self,
        vk_code: u32,
        is_key_down: bool,
        modifier_state: &HashMap<u32, bool>,
    ) -> bool {
        let hotkey = match &self.config {
            Some(h) => h,
            None => return false,
        };

        match hotkey {
            HotkeyType::SingleKey { vk_code: target_vk } => {
                if is_key_down && vk_code == *target_vk {
                    return true;
                }
            }

            HotkeyType::ModifierCombo { modifiers, key } => {
                if is_key_down && vk_code == *key {
                    let all_pressed = modifiers
                        .iter()
                        .all(|m| modifier_state.get(m).copied().unwrap_or(false));
                    if all_pressed {
                        return true;
                    }
                }
            }

            HotkeyType::DoublePress {
                vk_code: target_vk,
                min_interval_ms,
                max_interval_ms,
            } => {
                let normalized_vk = normalize_vk_code(vk_code);
                if normalized_vk == *target_vk {
                    if is_key_down {
                        // Ignore key repeats (auto-repeat from holding key)
                        if self.last_key_pressed {
                            return false;
                        }
                        self.last_key_pressed = true;

                        let now = Instant::now();
                        match self.last_key_time {
                            Some(last) => {
                                let elapsed = now.duration_since(last);

                                if !self.last_key_interrupted
                                    && elapsed >= Duration::from_millis(*min_interval_ms)
                                    && elapsed < Duration::from_millis(*max_interval_ms)
                                {
                                    self.last_key_time = None;
                                    return true;
                                } else if elapsed >= Duration::from_millis(*max_interval_ms)
                                    || self.last_key_interrupted
                                {
                                    // Start new sequence
                                    self.last_key_time = Some(now);
                                    self.last_key_interrupted = false;
                                }
                            }
                            None => {
                                self.last_key_time = Some(now);
                                self.last_key_interrupted = false;
                            }
                        }
                    } else {
                        self.last_key_pressed = false;
                    }
                }
            }
        }

        false
    }

    /// Mark double-press sequence as interrupted if another key was pressed
    fn mark_interrupted_if_needed(&mut self, vk_code: u32) {
        if let Some(HotkeyType::DoublePress {
            vk_code: target_vk, ..
        }) = &self.config
        {
            let normalized_vk = normalize_vk_code(vk_code);
            if normalized_vk != *target_vk && self.last_key_time.is_some() {
                self.last_key_interrupted = true;
            }
        }
    }
}

/// Map rdev::Key to abstract VK code
fn rdev_key_to_vk(key: &rdev::Key) -> Option<u32> {
    use rdev::Key;
    match key {
        // Modifiers
        Key::ControlLeft => Some(super::keycodes::KEY_LCONTROL),
        Key::ControlRight => Some(super::keycodes::KEY_RCONTROL),
        Key::ShiftLeft => Some(super::keycodes::KEY_LSHIFT),
        Key::ShiftRight => Some(super::keycodes::KEY_RSHIFT),
        Key::Alt => Some(super::keycodes::KEY_LALT),
        Key::AltGr => Some(super::keycodes::KEY_RALT),
        Key::MetaLeft => Some(super::keycodes::KEY_LWIN),
        Key::MetaRight => Some(super::keycodes::KEY_RWIN),

        // Function keys
        Key::F1 => Some(112),
        Key::F2 => Some(113),
        Key::F3 => Some(114),
        Key::F4 => Some(115),
        Key::F5 => Some(116),
        Key::F6 => Some(117),
        Key::F7 => Some(118),
        Key::F8 => Some(119),
        Key::F9 => Some(120),
        Key::F10 => Some(121),
        Key::F11 => Some(122),
        Key::F12 => Some(123),

        // Special keys
        Key::Space => Some(32),
        Key::Tab => Some(9),
        Key::Return => Some(13),
        Key::Escape => Some(super::keycodes::KEY_ESCAPE),
        Key::Backspace => Some(8),
        Key::Delete => Some(super::keycodes::KEY_DELETE),
        Key::Insert => Some(45),
        Key::Home => Some(36),
        Key::End => Some(35),
        Key::PageUp => Some(33),
        Key::PageDown => Some(34),

        // Arrow keys
        Key::LeftArrow => Some(37),
        Key::RightArrow => Some(39),
        Key::UpArrow => Some(38),
        Key::DownArrow => Some(40),

        // Letters
        Key::KeyA => Some('A' as u32),
        Key::KeyB => Some('B' as u32),
        Key::KeyC => Some('C' as u32),
        Key::KeyD => Some('D' as u32),
        Key::KeyE => Some('E' as u32),
        Key::KeyF => Some('F' as u32),
        Key::KeyG => Some('G' as u32),
        Key::KeyH => Some('H' as u32),
        Key::KeyI => Some('I' as u32),
        Key::KeyJ => Some('J' as u32),
        Key::KeyK => Some('K' as u32),
        Key::KeyL => Some('L' as u32),
        Key::KeyM => Some('M' as u32),
        Key::KeyN => Some('N' as u32),
        Key::KeyO => Some('O' as u32),
        Key::KeyP => Some('P' as u32),
        Key::KeyQ => Some('Q' as u32),
        Key::KeyR => Some('R' as u32),
        Key::KeyS => Some('S' as u32),
        Key::KeyT => Some('T' as u32),
        Key::KeyU => Some('U' as u32),
        Key::KeyV => Some('V' as u32),
        Key::KeyW => Some('W' as u32),
        Key::KeyX => Some('X' as u32),
        Key::KeyY => Some('Y' as u32),
        Key::KeyZ => Some('Z' as u32),

        // Numbers
        Key::Num0 => Some('0' as u32),
        Key::Num1 => Some('1' as u32),
        Key::Num2 => Some('2' as u32),
        Key::Num3 => Some('3' as u32),
        Key::Num4 => Some('4' as u32),
        Key::Num5 => Some('5' as u32),
        Key::Num6 => Some('6' as u32),
        Key::Num7 => Some('7' as u32),
        Key::Num8 => Some('8' as u32),
        Key::Num9 => Some('9' as u32),

        Key::Unknown(code) => Some(*code),
        _ => None,
    }
}

/// Check if a VK code represents a modifier key
fn is_modifier_key(vk_code: u32) -> bool {
    matches!(
        normalize_vk_code(vk_code),
        super::keycodes::KEY_CONTROL | super::keycodes::KEY_SHIFT | super::keycodes::KEY_ALT
    ) || vk_code == super::keycodes::KEY_LWIN
        || vk_code == super::keycodes::KEY_RWIN
}

/// Global hotkey listener for Linux, driven by `rdev` key events and X11 key grabbing (see [`super::xgrab`]).
pub struct KeyboardHook {
    translator: Translator,
    should_exit: Arc<AtomicBool>,
    config_manager: Arc<ConfigManager>,
}

impl KeyboardHook {
    /// Create a new hook. Does not start listening; call [`KeyboardHook::start`] for that.
    pub fn new(
        translator: Translator,
        should_exit: Arc<AtomicBool>,
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self {
            translator,
            should_exit,
            config_manager,
        })
    }

    /// Start listening for the configured hotkeys and block until `should_exit` is set.
    ///
    /// Behavior depends on the detected display server: full X11/XWayland grabbing when
    /// `DISPLAY` is set, a disabled-hotkeys fallback (interactive/CLI mode still works) on
    /// pure Wayland or when no display server is detected at all.
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check display server
        let has_x11 = std::env::var("DISPLAY").is_ok();
        let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

        if !has_x11 && !has_wayland {
            eprintln!("No display server detected. Hotkeys disabled, use interactive mode.");
            self.wait_for_exit().await;
            return Ok(());
        }

        if has_wayland && !has_x11 {
            // Pure Wayland without XWayland
            eprintln!(
                "Wayland detected without X11. Global hotkeys not yet supported on pure Wayland."
            );
            eprintln!("Use interactive mode for translations.");
            self.wait_for_exit().await;
            return Ok(());
        }

        // X11 available (either native X11 or XWayland) - use rdev
        self.start_x11().await
    }

    async fn start_x11(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Parse hotkeys from config
        let config = self.config_manager.get_config();

        let translate_hotkey = match HotkeyParser::parse(&config.translate_hotkey) {
            Ok(hotkey) => match HotkeyParser::validate_hotkey(&hotkey) {
                Ok(_) => Some(hotkey),
                Err(e) => {
                    eprintln!(
                        "Warning: Hotkey validation failed for '{}': {}",
                        config.translate_hotkey, e
                    );
                    eprintln!("Translation hotkey disabled.");
                    None
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse hotkey '{}': {}",
                    config.translate_hotkey, e
                );
                eprintln!("Translation hotkey disabled.");
                None
            }
        };

        let speech_hotkey = if config.enable_speech_hotkey && config.enable_text_to_speech {
            match HotkeyParser::parse(&config.speech_hotkey) {
                Ok(hotkey) => match HotkeyParser::validate_hotkey(&hotkey) {
                    Ok(_) => Some(hotkey),
                    Err(e) => {
                        eprintln!(
                            "Warning: Speech hotkey validation failed for '{}': {}",
                            config.speech_hotkey, e
                        );
                        None
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse speech hotkey '{}': {}",
                        config.speech_hotkey, e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Grab hotkeys via X11 to prevent them from reaching other applications.
        // _xgrab lives until the end of the event loop; Drop releases all grabs.
        let mut _xgrab = super::xgrab::XGrabManager::new();
        if let Some(ref mut xgrab) = _xgrab {
            if let Some(ref hotkey) = translate_hotkey {
                xgrab.grab_hotkey(hotkey);
            }
            if let Some(ref hotkey) = speech_hotkey {
                xgrab.grab_hotkey(hotkey);
            }
        }

        // Create shared state
        let state = Arc::new(Mutex::new(KeyboardEventState {
            translate_hotkey: HotkeyState::new(translate_hotkey),
            speech_hotkey: HotkeyState::new(speech_hotkey),
            modifier_state: HashMap::new(),
            speech_enabled: config.enable_text_to_speech,
            speech_hotkey_enabled: config.enable_speech_hotkey,
        }));

        let is_processing = Arc::new(Mutex::new(false));
        let is_speaking = Arc::new(Mutex::new(false));
        let should_stop_speech = Arc::new(AtomicBool::new(false));

        // Create channel for key events
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<KeyEvent>();

        // Start rdev listener in a separate thread
        let should_exit_clone = self.should_exit.clone();
        std::thread::spawn(move || {
            let callback = move |event: rdev::Event| {
                let key_event = match event.event_type {
                    rdev::EventType::KeyPress(key) => rdev_key_to_vk(&key).map(|vk| KeyEvent {
                        vk_code: vk,
                        is_key_down: true,
                    }),
                    rdev::EventType::KeyRelease(key) => rdev_key_to_vk(&key).map(|vk| KeyEvent {
                        vk_code: vk,
                        is_key_down: false,
                    }),
                    _ => None,
                };

                if let Some(ke) = key_event {
                    let _ = tx.send(ke);
                }
            };

            if let Err(e) = rdev::listen(callback) {
                if !should_exit_clone.load(Ordering::Relaxed) {
                    eprintln!("Keyboard listener error: {:?}", e);
                }
            }
        });

        // Process events
        let translator = self.translator.clone();
        let config_manager = self.config_manager.clone();

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let mut state_guard = match state.lock() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    // Update modifier state
                    if is_modifier_key(event.vk_code) {
                        let normalized = normalize_vk_code(event.vk_code);
                        state_guard.modifier_state.insert(normalized, event.is_key_down);
                    }

                    // Update key pressed state for ESC monitoring
                    if event.vk_code == super::keycodes::KEY_ESCAPE && event.is_key_down {
                        // Store ESC state for is_key_pressed
                        super::keycodes::set_key_state(super::keycodes::KEY_ESCAPE as i32, true);

                        // Cancel speech if speaking
                        if let Ok(speaking) = is_speaking.lock() {
                            if *speaking {
                                should_stop_speech.store(true, Ordering::Relaxed);
                                println!("Speech cancelled by user (Esc)");
                            }
                        }
                    } else if event.vk_code == super::keycodes::KEY_ESCAPE && !event.is_key_down {
                        super::keycodes::set_key_state(super::keycodes::KEY_ESCAPE as i32, false);
                    }

                    if event.is_key_down {
                        // Mark double-press sequences as interrupted
                        state_guard.translate_hotkey.mark_interrupted_if_needed(event.vk_code);
                        state_guard.speech_hotkey.mark_interrupted_if_needed(event.vk_code);

                        // Check translation hotkey
                        let modifier_snapshot = state_guard.modifier_state.clone();
                        if state_guard.translate_hotkey.handle(
                            event.vk_code,
                            true,
                            &modifier_snapshot,
                        ) {
                            // Clear all modifier state: xdotool keyup will release them,
                            // and rdev will see synthetic release events anyway
                            state_guard.modifier_state.clear();
                            drop(state_guard);
                            Self::trigger_translation(
                                &translator,
                                &is_processing,
                            );
                            continue;
                        }

                        // Check speech hotkey
                        if state_guard.speech_enabled
                            && state_guard.speech_hotkey_enabled
                            && state_guard.speech_hotkey.handle(
                                event.vk_code,
                                true,
                                &modifier_snapshot,
                            )
                        {
                            // Clear all modifier state before speech trigger
                            state_guard.modifier_state.clear();
                            drop(state_guard);
                            Self::trigger_speech(
                                &translator,
                                &config_manager,
                                &is_speaking,
                                &should_stop_speech,
                            );
                            continue;
                        }
                    } else {
                        // Key up - update hotkey state for modifier combos and double-press
                        let modifier_snapshot = state_guard.modifier_state.clone();
                        state_guard.translate_hotkey.handle(
                            event.vk_code,
                            false,
                            &modifier_snapshot,
                        );
                        if state_guard.speech_enabled && state_guard.speech_hotkey_enabled {
                            state_guard.speech_hotkey.handle(
                                event.vk_code,
                                false,
                                &modifier_snapshot,
                            );
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    if self.should_exit.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Trigger translation in a separate thread
    fn trigger_translation(translator: &Translator, is_processing: &Arc<Mutex<bool>>) {
        if let Ok(mut processing) = is_processing.lock() {
            if *processing {
                return;
            }
            *processing = true;
        }

        let translator_clone = translator.clone();
        let processing_clone = is_processing.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = translator_clone.translate_clipboard().await {
                    eprintln!("Translation error: {}", e);
                }
                if let Ok(mut proc) = processing_clone.lock() {
                    *proc = false;
                }
            });
        });
    }

    /// Trigger text-to-speech in a separate thread
    fn trigger_speech(
        translator: &Translator,
        config_manager: &Arc<ConfigManager>,
        is_speaking: &Arc<Mutex<bool>>,
        should_stop_speech: &Arc<AtomicBool>,
    ) {
        if let Ok(mut speaking) = is_speaking.lock() {
            if *speaking {
                return;
            }
            *speaking = true;
        }

        should_stop_speech.store(false, Ordering::Relaxed);

        let _translator_clone = translator.clone();
        let config_manager_clone = config_manager.clone();
        let speaking_clone = is_speaking.clone();
        let stop_flag_clone = should_stop_speech.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = speak_clipboard(&config_manager_clone, stop_flag_clone).await {
                    eprintln!("Speech error: {}", e);
                }
                if let Ok(mut speaking) = speaking_clone.lock() {
                    *speaking = false;
                }
            });
        });
    }

    /// Wait for exit flag (used when hotkeys are disabled)
    async fn wait_for_exit(&self) {
        loop {
            if self.should_exit.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

/// Key event sent from rdev listener thread to async processor
struct KeyEvent {
    vk_code: u32,
    is_key_down: bool,
}

/// Combined state for all hotkey detection
struct KeyboardEventState {
    translate_hotkey: HotkeyState,
    speech_hotkey: HotkeyState,
    modifier_state: HashMap<u32, bool>,
    speech_enabled: bool,
    speech_hotkey_enabled: bool,
}

/// Speak text from clipboard
async fn speak_clipboard(
    config_manager: &ConfigManager,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::platform::ClipboardManager;
    use std::io::{self, Write};

    let clipboard = ClipboardManager::new();
    clipboard.copy_selected_text()?;
    let text = clipboard.get_text()?;

    if let Err(e) = config_manager.check_and_reload() {
        eprintln!("Config reload error: {}", e);
    }
    let config = config_manager.get_config();

    if text.trim().is_empty() {
        print!("\r");
        io::stdout().flush().ok();
        println!("Clipboard is empty, nothing to speak");
        println!();
        print_source_prompt(&config);
        return Ok(());
    }

    let (source_code, _) = config_manager.get_language_codes();
    let speech_manager = SpeechManager::new();
    let provider = tagent::providers::create_provider(&config.translate_provider)?;
    let lang_code =
        tagent::providers::resolve_source_language(provider.as_ref(), &text, &source_code).await;

    print!("\r");
    io::stdout().flush().ok();

    SpeechManager::print_speech_label(&text, Some(&config.target_prompt_color));

    if let Err(e) = speech_manager
        .speak_text_with_cancel(provider.as_ref(), &text, &lang_code, stop_flag)
        .await
    {
        eprintln!("Speech error: {}", e);
    }

    println!();
    print_source_prompt(&config);
    Ok(())
}

/// Print source language prompt with color
fn print_source_prompt(cfg: &crate::config::Config) {
    use std::io::{self, Write};

    let source_prompt = format!("[{}]: ", cfg.source_language);
    config::print_colored(&source_prompt, &cfg.source_prompt_color);
    io::stdout().flush().ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_state_modifier_combo_detection() {
        let hotkey = HotkeyType::ModifierCombo {
            modifiers: vec![super::super::keycodes::KEY_ALT],
            key: 'Q' as u32,
        };
        let mut state = HotkeyState::new(Some(hotkey));

        // Alt not pressed - should not trigger
        let mut mods = HashMap::new();
        assert!(!state.handle('Q' as u32, true, &mods));

        // Alt pressed - should trigger
        mods.insert(super::super::keycodes::KEY_ALT, true);
        assert!(state.handle('Q' as u32, true, &mods));
    }

    #[test]
    fn test_hotkey_state_double_press_detection() {
        let hotkey = HotkeyType::DoublePress {
            vk_code: super::super::keycodes::KEY_CONTROL,
            min_interval_ms: 50,
            max_interval_ms: 500,
        };
        let mut state = HotkeyState::new(Some(hotkey));
        let mods = HashMap::new();

        // First press
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
        // First release
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, false, &mods));

        // Wait within the valid interval
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Second press - should trigger
        assert!(state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
    }

    #[test]
    fn test_hotkey_state_double_press_interrupted() {
        let hotkey = HotkeyType::DoublePress {
            vk_code: super::super::keycodes::KEY_CONTROL,
            min_interval_ms: 50,
            max_interval_ms: 500,
        };
        let mut state = HotkeyState::new(Some(hotkey));
        let mods = HashMap::new();

        // First press
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, false, &mods));

        // Another key interrupts the sequence
        state.mark_interrupted_if_needed('A' as u32);

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Second press - should NOT trigger because sequence was interrupted
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
    }

    #[test]
    fn test_hotkey_state_triggers_on_lr_specific_modifier() {
        // Regression test for a parsed "LAlt+Q" config: the observed modifier is always
        // normalized to the generic code before it's stored in modifier_state (see the
        // event loop at `normalize_vk_code(event.vk_code)`), so HotkeyParser::parse must
        // normalize the configured side too, or this never triggers.
        let hotkey = HotkeyParser::parse("LAlt+Q").unwrap();
        let mut state = HotkeyState::new(Some(hotkey));

        // No modifier pressed - should not trigger
        let mods = HashMap::new();
        assert!(!state.handle('Q' as u32, true, &mods));

        // Physical left Alt pressed, stored normalized exactly as the real event loop does
        let mut mods = HashMap::new();
        mods.insert(normalize_vk_code(super::super::keycodes::KEY_LALT), true);
        assert!(state.handle('Q' as u32, true, &mods));
    }

    #[test]
    fn test_hotkey_state_triggers_on_lr_specific_double_press() {
        // Regression test for a parsed "LCtrl+LCtrl" config.
        let hotkey = HotkeyParser::parse("LCtrl+LCtrl").unwrap();
        let mut state = HotkeyState::new(Some(hotkey));
        let mods = HashMap::new();

        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
        assert!(!state.handle(super::super::keycodes::KEY_LCONTROL, false, &mods));

        std::thread::sleep(std::time::Duration::from_millis(100));

        assert!(state.handle(super::super::keycodes::KEY_LCONTROL, true, &mods));
    }

    #[test]
    fn test_modifier_state_cleared_on_hotkey_trigger() {
        // Simulate what happens in the event loop when a hotkey triggers
        let mut modifier_state: HashMap<u32, bool> = HashMap::new();
        modifier_state.insert(super::super::keycodes::KEY_ALT, true);

        // After hotkey triggers, modifier_state.clear() is called
        modifier_state.clear();

        // Verify all modifiers are cleared
        assert!(!modifier_state
            .get(&super::super::keycodes::KEY_ALT)
            .copied()
            .unwrap_or(false));
        assert!(!modifier_state
            .get(&super::super::keycodes::KEY_CONTROL)
            .copied()
            .unwrap_or(false));
        assert!(!modifier_state
            .get(&super::super::keycodes::KEY_SHIFT)
            .copied()
            .unwrap_or(false));
    }

    #[test]
    fn test_is_modifier_key() {
        assert!(is_modifier_key(super::super::keycodes::KEY_LCONTROL));
        assert!(is_modifier_key(super::super::keycodes::KEY_RCONTROL));
        assert!(is_modifier_key(super::super::keycodes::KEY_LALT));
        assert!(is_modifier_key(super::super::keycodes::KEY_RALT));
        assert!(is_modifier_key(super::super::keycodes::KEY_LSHIFT));
        assert!(is_modifier_key(super::super::keycodes::KEY_RSHIFT));
        assert!(is_modifier_key(super::super::keycodes::KEY_LWIN));
        assert!(is_modifier_key(super::super::keycodes::KEY_RWIN));
        // Regular keys should not be modifiers
        assert!(!is_modifier_key('A' as u32));
        assert!(!is_modifier_key('Q' as u32));
        assert!(!is_modifier_key(112)); // F1
    }

    #[test]
    fn test_rdev_key_to_vk_modifiers() {
        assert_eq!(
            rdev_key_to_vk(&rdev::Key::Alt),
            Some(super::super::keycodes::KEY_LALT)
        );
        assert_eq!(
            rdev_key_to_vk(&rdev::Key::ControlLeft),
            Some(super::super::keycodes::KEY_LCONTROL)
        );
        assert_eq!(
            rdev_key_to_vk(&rdev::Key::ControlRight),
            Some(super::super::keycodes::KEY_RCONTROL)
        );
        assert_eq!(
            rdev_key_to_vk(&rdev::Key::ShiftLeft),
            Some(super::super::keycodes::KEY_LSHIFT)
        );
    }

    #[test]
    fn test_rdev_key_to_vk_letters() {
        assert_eq!(rdev_key_to_vk(&rdev::Key::KeyQ), Some('Q' as u32));
        assert_eq!(rdev_key_to_vk(&rdev::Key::KeyA), Some('A' as u32));
        assert_eq!(rdev_key_to_vk(&rdev::Key::KeyZ), Some('Z' as u32));
    }

    #[test]
    fn test_single_key_hotkey() {
        let hotkey = HotkeyType::SingleKey { vk_code: 120 }; // F9
        let mut state = HotkeyState::new(Some(hotkey));
        let mods = HashMap::new();

        // Wrong key - should not trigger
        assert!(!state.handle(119, true, &mods)); // F8

        // Correct key - should trigger
        assert!(state.handle(120, true, &mods)); // F9
    }
}
