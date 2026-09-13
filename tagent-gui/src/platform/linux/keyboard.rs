use super::keycodes::normalize_vk_code;
use super::xgrab::XGrabManager;
use crate::config::HotkeyType;
use std::collections::HashMap;

/// Encapsulates hotkey detection state for the one configured hotkey.
///
/// Unlike `tagent-cli` (translate + speech hotkeys), `tagent-gui` only has one
/// hotkey today, so this holds a single, non-shared instance rather than a map.
struct HotkeyState {
    config: HotkeyType,
    last_key_time: Option<std::time::Instant>,
    last_key_pressed: bool,
    last_key_interrupted: bool,
}

impl HotkeyState {
    fn new(hotkey: HotkeyType) -> Self {
        Self {
            config: hotkey,
            last_key_time: None,
            last_key_pressed: false,
            last_key_interrupted: false,
        }
    }

    /// Handle hotkey detection for one key event. Returns true if the hotkey was triggered.
    fn handle(
        &mut self,
        vk_code: u32,
        is_key_down: bool,
        modifier_state: &HashMap<u32, bool>,
    ) -> bool {
        use std::time::{Duration, Instant};

        match &self.config {
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
        if let HotkeyType::DoublePress {
            vk_code: target_vk, ..
        } = &self.config
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

/// One key event sent from the `rdev` listener thread to the processing thread.
struct KeyEvent {
    vk_code: u32,
    is_key_down: bool,
}

/// Global hotkey listener for Linux, driven by `rdev` key events and X11 key
/// grabbing (see [`super::xgrab`]).
///
/// Fully app-agnostic: it knows nothing about `tagent`, providers, or Slint —
/// it only calls `on_trigger()` when the configured hotkey fires. The caller
/// (`main.rs`) is responsible for guarding against overlapping triggers,
/// reading clipboard/UI state, and doing the actual translation.
pub struct KeyboardHook;

impl KeyboardHook {
    /// Parse-and-validate the hotkey beforehand (see `config::HotkeyParser`) and
    /// start listening for it in a background thread. Returns immediately; the
    /// listener runs for the lifetime of the process (no explicit shutdown —
    /// `tagent-gui` has no competing mode to coordinate with, unlike
    /// `tagent-cli`'s unified hotkey+interactive-terminal setup).
    ///
    /// Behavior depends on the detected display server: full X11/XWayland
    /// grabbing when `DISPLAY` is set, a disabled-hotkeys no-op on pure
    /// Wayland or when no display server is detected at all.
    pub fn spawn(hotkey: HotkeyType, on_trigger: impl Fn() + Send + Sync + 'static) {
        let has_x11 = std::env::var("DISPLAY").is_ok();
        let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

        if !has_x11 && !has_wayland {
            eprintln!("No display server detected. Global hotkey disabled.");
            return;
        }

        if has_wayland && !has_x11 {
            eprintln!(
                "Wayland detected without X11. Global hotkeys not yet supported on pure Wayland."
            );
            return;
        }

        std::thread::spawn(move || Self::run_x11(hotkey, on_trigger));
    }

    fn run_x11(hotkey: HotkeyType, on_trigger: impl Fn() + Send + Sync + 'static) {
        // Grab the hotkey via X11 to prevent it from reaching other applications.
        // _xgrab lives until the end of this function; Drop releases the grab.
        let mut _xgrab = XGrabManager::new();
        if let Some(ref mut xgrab) = _xgrab {
            xgrab.grab_hotkey(&hotkey);
        }

        let mut state = HotkeyState::new(hotkey);
        let mut modifier_state: HashMap<u32, bool> = HashMap::new();

        let (tx, rx) = std::sync::mpsc::channel::<KeyEvent>();

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
                eprintln!("Keyboard listener error: {:?}", e);
            }
        });

        for event in rx {
            if is_modifier_key(event.vk_code) {
                let normalized = normalize_vk_code(event.vk_code);
                modifier_state.insert(normalized, event.is_key_down);
            }

            if event.is_key_down {
                state.mark_interrupted_if_needed(event.vk_code);

                if state.handle(event.vk_code, true, &modifier_state) {
                    // Clear all modifier state: the clipboard copy that follows
                    // releases them via its own `xdotool keyup`, and rdev will
                    // see synthetic release events anyway.
                    modifier_state.clear();
                    on_trigger();
                    continue;
                }
            } else {
                state.handle(event.vk_code, false, &modifier_state);
            }
        }
    }
}
