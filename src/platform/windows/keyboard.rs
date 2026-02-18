use crate::config::{self, ConfigManager, HotkeyParser, HotkeyType};
use crate::speech::SpeechManager;
use crate::translator::Translator;
use super::keycodes::normalize_vk_code;
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::{
    Win32::Foundation::*, Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
};

static TRANSLATOR: OnceLock<Arc<Translator>> = OnceLock::new();
static IS_PROCESSING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_EXIT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static MODIFIER_STATE: OnceLock<Arc<Mutex<HashMap<u32, bool>>>> = OnceLock::new();

// Hotkey state instances for translate and speech
static TRANSLATE_HOTKEY: OnceLock<HotkeyState> = OnceLock::new();
static SPEECH_HOTKEY: OnceLock<HotkeyState> = OnceLock::new();

// Speech-specific state
static SPEECH_HOTKEY_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static SPEECH_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static IS_SPEAKING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_STOP_SPEECH: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static CONFIG_MANAGER: OnceLock<Arc<ConfigManager>> = OnceLock::new();

/// Encapsulates hotkey detection state for a single hotkey.
/// Eliminates duplication between translate and speech hotkey handlers.
struct HotkeyState {
    config: Arc<Mutex<Option<HotkeyType>>>,
    last_key_time: Arc<Mutex<Option<Instant>>>,
    last_key_pressed: Arc<Mutex<bool>>,
    last_key_interrupted: Arc<Mutex<bool>>,
}

impl HotkeyState {
    fn new(hotkey: Option<HotkeyType>) -> Self {
        Self {
            config: Arc::new(Mutex::new(hotkey)),
            last_key_time: Arc::new(Mutex::new(None)),
            last_key_pressed: Arc::new(Mutex::new(false)),
            last_key_interrupted: Arc::new(Mutex::new(false)),
        }
    }

    /// Handle hotkey detection for key events.
    /// Returns true if the event was consumed (should be blocked).
    /// Calls `trigger_fn` when the hotkey combination is activated.
    fn handle(&self, vk_code: u32, is_key_down: bool, trigger_fn: unsafe fn()) -> bool {
        let hotkey_opt = match self.config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return false,
        };

        let hotkey = match hotkey_opt.as_ref() {
            Some(h) => h,
            None => return false,
        };

        match hotkey {
            HotkeyType::SingleKey { vk_code: target_vk } => {
                if is_key_down && vk_code == *target_vk {
                    unsafe { trigger_fn() };
                    return true;
                }
            }

            HotkeyType::ModifierCombo { modifiers, key } => {
                if let Some(modifier_state) = MODIFIER_STATE.get() {
                    if let Ok(mut state) = modifier_state.lock() {
                        let normalized_vk = normalize_vk_code(vk_code);

                        // Update modifier state and block modifier events
                        if modifiers.contains(&normalized_vk) {
                            state.insert(normalized_vk, is_key_down);
                            return true;
                        }

                        // Check if all modifiers are pressed and the key is pressed
                        if is_key_down && vk_code == *key {
                            let all_modifiers_pressed = modifiers
                                .iter()
                                .all(|m| state.get(m).copied().unwrap_or(false));

                            if all_modifiers_pressed {
                                unsafe { trigger_fn() };
                                return true;
                            }
                        }

                        // Clean up state on key up
                        if !is_key_down {
                            state.insert(normalized_vk, false);
                        }
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
                        // Check if this is a key repeat (auto-repeat from holding key down)
                        if let Ok(mut is_pressed) = self.last_key_pressed.lock() {
                            if *is_pressed {
                                return false;
                            }
                            *is_pressed = true;
                        }

                        if let Ok(mut last_time) = self.last_key_time.lock() {
                            let now = Instant::now();

                            match *last_time {
                                Some(last) => {
                                    let elapsed = now.duration_since(last);

                                    // Check if sequence was interrupted
                                    let was_interrupted = self
                                        .last_key_interrupted
                                        .lock()
                                        .ok()
                                        .is_some_and(|f| *f);

                                    if !was_interrupted
                                        && elapsed >= Duration::from_millis(*min_interval_ms)
                                        && elapsed < Duration::from_millis(*max_interval_ms)
                                    {
                                        unsafe { trigger_fn() };
                                        *last_time = None;
                                        return true;
                                    } else if elapsed >= Duration::from_millis(*max_interval_ms)
                                        || was_interrupted
                                    {
                                        // Start new sequence
                                        *last_time = Some(now);
                                        if let Ok(mut flag) = self.last_key_interrupted.lock() {
                                            *flag = false;
                                        }
                                    }
                                }
                                None => {
                                    // First press - start new sequence
                                    *last_time = Some(now);
                                    if let Ok(mut flag) = self.last_key_interrupted.lock() {
                                        *flag = false;
                                    }
                                }
                            }
                        }
                    } else {
                        // Key up event - mark key as not pressed
                        if let Ok(mut is_pressed) = self.last_key_pressed.lock() {
                            *is_pressed = false;
                        }
                    }
                }
            }
        }

        false
    }

    /// Mark double-press sequence as interrupted if another key was pressed
    fn mark_interrupted_if_needed(&self, vk_code: u32) {
        let hotkey_opt = match self.config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return,
        };

        if let Some(HotkeyType::DoublePress {
            vk_code: target_vk, ..
        }) = hotkey_opt.as_ref()
        {
            let normalized_vk = normalize_vk_code(vk_code);
            if normalized_vk != *target_vk {
                if let Ok(time) = self.last_key_time.lock() {
                    if time.is_some() {
                        if let Ok(mut flag) = self.last_key_interrupted.lock() {
                            *flag = true;
                        }
                    }
                }
            }
        }
    }
}

pub struct KeyboardHook;

impl KeyboardHook {
    pub fn new(
        translator: Translator,
        should_exit: Arc<AtomicBool>,
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error>> {
        TRANSLATOR
            .set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;

        CONFIG_MANAGER
            .set(config_manager.clone())
            .map_err(|_| "ConfigManager already initialized")?;

        IS_PROCESSING
            .set(Arc::new(Mutex::new(false)))
            .map_err(|_| "IsProcessing already initialized")?;
        SHOULD_EXIT
            .set(should_exit)
            .map_err(|_| "ShouldExit already initialized")?;

        MODIFIER_STATE
            .set(Arc::new(Mutex::new(HashMap::new())))
            .map_err(|_| "ModifierState already initialized")?;

        // Initialize translation hotkey
        let config = config_manager.get_config();

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

        TRANSLATE_HOTKEY
            .set(HotkeyState::new(translate_hotkey))
            .map_err(|_| "TranslateHotkey already initialized")?;

        // Initialize speech hotkey
        let speech_hotkey = if config.enable_speech_hotkey && config.enable_text_to_speech {
            match HotkeyParser::parse(&config.speech_hotkey) {
                Ok(hotkey) => match HotkeyParser::validate_hotkey(&hotkey) {
                    Ok(_) => Some(hotkey),
                    Err(e) => {
                        eprintln!(
                            "Warning: Speech hotkey validation failed for '{}': {}",
                            config.speech_hotkey, e
                        );
                        eprintln!("Speech hotkey disabled.");
                        None
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse speech hotkey '{}': {}",
                        config.speech_hotkey, e
                    );
                    eprintln!("Speech hotkey disabled.");
                    None
                }
            }
        } else {
            None
        };

        SPEECH_HOTKEY
            .set(HotkeyState::new(speech_hotkey))
            .map_err(|_| "SpeechHotkey already initialized")?;

        SPEECH_HOTKEY_ENABLED
            .set(Arc::new(AtomicBool::new(config.enable_speech_hotkey)))
            .map_err(|_| "SpeechHotkeyEnabled already initialized")?;

        SPEECH_ENABLED
            .set(Arc::new(AtomicBool::new(config.enable_text_to_speech)))
            .map_err(|_| "SpeechEnabled already initialized")?;

        IS_SPEAKING
            .set(Arc::new(Mutex::new(false)))
            .map_err(|_| "IsSpeaking already initialized")?;

        SHOULD_STOP_SPEECH
            .set(Arc::new(AtomicBool::new(false)))
            .map_err(|_| "ShouldStopSpeech already initialized")?;

        Ok(Self)
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;

            if hook.0 == 0 {
                return Err("Failed to set keyboard hook".into());
            }

            loop {
                // Check if we should exit
                if let Some(should_exit) = SHOULD_EXIT.get() {
                    if should_exit.load(Ordering::Relaxed) {
                        break;
                    }
                }

                let mut msg = MSG::default();

                // Use PeekMessage instead of GetMessage to avoid blocking
                let has_message = PeekMessageW(
                    &mut msg,
                    HWND::default(),
                    0,
                    0,
                    PEEK_MESSAGE_REMOVE_TYPE(1u32),
                );

                if has_message.as_bool() {
                    match msg.message {
                        WM_QUIT => {
                            println!("WM_QUIT received, exiting");
                            break;
                        }
                        _ => {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    // No message available, sleep briefly to avoid busy waiting
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }

            UnhookWindowsHookEx(hook)?;
        }

        Ok(())
    }
}

/// Trigger translation in a separate thread
unsafe fn trigger_translation() {
    if let Some(is_processing) = IS_PROCESSING.get() {
        if let Ok(mut processing) = is_processing.lock() {
            if *processing {
                return;
            }
            *processing = true;
        }
    }

    if let Some(translator) = TRANSLATOR.get() {
        let translator_clone = translator.clone();
        let processing_clone = IS_PROCESSING.get().unwrap().clone();

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
}

/// Trigger text-to-speech in a separate thread
unsafe fn trigger_speech() {
    // Check if speech is enabled
    if let Some(speech_enabled) = SPEECH_ENABLED.get() {
        if !speech_enabled.load(Ordering::Relaxed) {
            println!("Text-to-speech is disabled in configuration");
            return;
        }
    }

    if let Some(is_speaking) = IS_SPEAKING.get() {
        if let Ok(mut speaking) = is_speaking.lock() {
            if *speaking {
                println!("Already speaking, ignoring request");
                return;
            }
            *speaking = true;
        }
    }

    // Reset stop flag
    if let Some(stop_flag) = SHOULD_STOP_SPEECH.get() {
        stop_flag.store(false, Ordering::Relaxed);
    }

    if let Some(translator) = TRANSLATOR.get() {
        let translator_clone = translator.clone();
        let speaking_clone = IS_SPEAKING.get().unwrap().clone();
        let stop_flag_clone = SHOULD_STOP_SPEECH.get().unwrap().clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = speak_clipboard(&translator_clone, stop_flag_clone).await {
                    eprintln!("Speech error: {}", e);
                }
                if let Ok(mut speaking) = speaking_clone.lock() {
                    *speaking = false;
                }
            });
        });
    }
}

/// Speak text from clipboard
async fn speak_clipboard(
    _translator: &Translator,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    use crate::platform::ClipboardManager;
    use crate::platform::WindowManager;
    use std::io::{self, Write};

    let clipboard = ClipboardManager::new();
    clipboard.copy_selected_text()?;
    let text = clipboard.get_text()?;

    // Get config from shared ConfigManager
    let config_manager = CONFIG_MANAGER
        .get()
        .ok_or("ConfigManager not initialized")?;
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

    // Show terminal window if configured
    if config.show_terminal_on_translate {
        if let Ok(window_manager) = WindowManager::new() {
            let _ = window_manager.show_terminal();
        }
    }

    // Detect language
    let (source_code, _) = config_manager.get_language_codes();
    let speech_manager = SpeechManager::new();
    let lang_code = speech_manager
        .detect_speech_language(&text, &source_code, &config.translate_provider)
        .await;

    // Clear any existing prompt and print speech info
    print!("\r");
    io::stdout().flush().ok();

    // Show speech label and speak
    SpeechManager::print_speech_label(&text, Some(&config.target_prompt_color));

    if let Err(e) = speech_manager
        .speak_text_with_cancel(&text, &lang_code, stop_flag)
        .await
    {
        eprintln!("Speech error: {}", e);
    }

    // Show source language prompt after speech completes
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

unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);

        // Ignore injected events (simulated by keybd_event, SendInput, etc.)
        const LLKHF_INJECTED: u32 = 0x10;
        if (kbd_struct.flags.0 & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        if w_param.0 as u32 == WM_KEYDOWN || w_param.0 as u32 == WM_SYSKEYDOWN {
            // Mark double-press sequences as interrupted if another key was pressed
            if let Some(state) = TRANSLATE_HOTKEY.get() {
                state.mark_interrupted_if_needed(kbd_struct.vkCode);
            }
            if let Some(state) = SPEECH_HOTKEY.get() {
                state.mark_interrupted_if_needed(kbd_struct.vkCode);
            }

            // Handle Esc key to stop speech
            if kbd_struct.vkCode == VK_ESCAPE.0 as u32 {
                if let Some(is_speaking) = IS_SPEAKING.get() {
                    if let Ok(speaking) = is_speaking.lock() {
                        if *speaking {
                            if let Some(stop_flag) = SHOULD_STOP_SPEECH.get() {
                                stop_flag.store(true, Ordering::Relaxed);
                                println!("Speech cancelled by user (Esc)");
                            }
                            return LRESULT(1);
                        }
                    }
                }
            }

            // Handle translation hotkey
            if let Some(state) = TRANSLATE_HOTKEY.get() {
                if state.handle(kbd_struct.vkCode, true, trigger_translation) {
                    return LRESULT(1);
                }
            }

            // Handle speech hotkey if enabled
            if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                if speech_enabled.load(Ordering::Relaxed) {
                    if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                        if tts_enabled.load(Ordering::Relaxed) {
                            if let Some(state) = SPEECH_HOTKEY.get() {
                                if state.handle(kbd_struct.vkCode, true, trigger_speech) {
                                    return LRESULT(1);
                                }
                            }
                        }
                    }
                }
            }
        } else if w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP {
            // Handle translation hotkey key up (for modifier state tracking)
            if let Some(state) = TRANSLATE_HOTKEY.get() {
                if state.handle(kbd_struct.vkCode, false, trigger_translation) {
                    return LRESULT(1);
                }
            }

            // Handle speech hotkey key up (for modifier state tracking)
            if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                if speech_enabled.load(Ordering::Relaxed) {
                    if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                        if tts_enabled.load(Ordering::Relaxed) {
                            if let Some(state) = SPEECH_HOTKEY.get() {
                                if state.handle(kbd_struct.vkCode, false, trigger_speech) {
                                    return LRESULT(1);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}
