use super::keycodes::normalize_vk_code;
use crate::config::{self, ConfigManager, HotkeyParser, HotkeyType};
use crate::speech::SpeechManager;
use crate::translator::Translator;
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

/// Ids passed to `RegisterHotKey`/delivered back in `WM_HOTKEY`'s `wParam`.
const TRANSLATE_HOTKEY_ID: i32 = 1;
const SPEECH_HOTKEY_ID: i32 = 2;

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
    /// Set once a `ModifierCombo` config has been handed off to `RegisterHotKey`
    /// (see `KeyboardHook::start`). While set, the low-level-hook-based combo
    /// detection in `handle` below is skipped so the combo isn't triggered twice
    /// — once by `WM_HOTKEY` and once by the hook.
    registered_with_os: AtomicBool,
}

impl HotkeyState {
    fn new(hotkey: Option<HotkeyType>) -> Self {
        Self {
            config: Arc::new(Mutex::new(hotkey)),
            last_key_time: Arc::new(Mutex::new(None)),
            last_key_pressed: Arc::new(Mutex::new(false)),
            last_key_interrupted: Arc::new(Mutex::new(false)),
            registered_with_os: AtomicBool::new(false),
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
                // RegisterHotKey (see `KeyboardHook::start`) already owns this combo end to
                // end via WM_HOTKEY; detecting it again here would fire the translation twice.
                if self.registered_with_os.load(Ordering::Relaxed) {
                    return false;
                }

                if let Some(modifier_state) = MODIFIER_STATE.get() {
                    if let Ok(mut state) = modifier_state.lock() {
                        let normalized_vk = normalize_vk_code(vk_code);

                        if modifiers.contains(&normalized_vk) {
                            // Track state only; modifiers must still reach other apps (Alt-Tab, menus).
                            state.insert(normalized_vk, is_key_down);
                        } else if is_key_down && vk_code == *key {
                            let all_modifiers_pressed = modifiers
                                .iter()
                                .all(|m| state.get(m).copied().unwrap_or(false));

                            if all_modifiers_pressed {
                                unsafe { trigger_fn() };
                                return true;
                            }
                        } else if !is_key_down {
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
                                    let was_interrupted =
                                        self.last_key_interrupted.lock().ok().is_some_and(|f| *f);

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

    /// If this state holds a `ModifierCombo`, attempt to register it with the OS via
    /// `RegisterHotKey` and, on success, mark it so `handle`'s hook-based detection steps
    /// aside. Returns whether registration happened (i.e. whether the caller must
    /// eventually call `UnregisterHotKey` with `id`). No-op for other hotkey types.
    unsafe fn try_register_with_os(&self, id: i32) -> bool {
        let hotkey_opt = match self.config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return false,
        };

        let (modifiers, key) = match hotkey_opt {
            Some(HotkeyType::ModifierCombo { modifiers, key }) => (modifiers, key),
            _ => return false,
        };

        let flags = modifiers_to_hotkey_flags(&modifiers);
        match RegisterHotKey(HWND::default(), id, flags, key) {
            Ok(()) => {
                self.registered_with_os.store(true, Ordering::Relaxed);
                true
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to register hotkey with the system ({}), falling back to key-hook detection",
                    e
                );
                false
            }
        }
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

/// Global hotkey listener for Windows, driven by a low-level `WH_KEYBOARD_LL` hook and
/// a Win32 message loop. All state is process-global (`OnceLock` statics) because the
/// hook callback is a plain `extern "system" fn` with no user-data pointer.
pub struct KeyboardHook;

impl KeyboardHook {
    /// Initialize global hook state (translator, hotkey configs for translate and
    /// speech, exit flag) from `config_manager`. Does not install the hook itself;
    /// call [`KeyboardHook::start`] for that. Must only be called once per process —
    /// returns an error if any of the underlying `OnceLock`s are already set.
    pub fn new(
        translator: Translator,
        should_exit: Arc<AtomicBool>,
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
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

    /// Install the low-level keyboard hook and run the Win32 message loop until
    /// `should_exit` is set or `WM_QUIT` is received, then uninstall the hook.
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;

            if hook.0 == 0 {
                return Err("Failed to set keyboard hook".into());
            }

            // Register modifier-combo hotkeys (e.g. Alt+Q) with the OS instead of only
            // detecting them in the low-level hook above. RegisterHotKey delivers WM_HOTKEY
            // straight to this thread and guarantees the triggering keystroke (Q) is not
            // also delivered to the foreground window as WM_SYSKEYDOWN/WM_SYSCHAR — that's
            // what produced the "no matching menu accelerator" system beep. It does NOT
            // guarantee anything about the preceding modifier keydown (Alt) itself; if the
            // foreground app still enters menu-mode focus on bare Alt-down and that ends up
            // swallowing the simulated Ctrl+C in `ClipboardManager::copy_selected_text`,
            // that needs a separate, verified-on-hardware fix in copy_selected_text (e.g.
            // waiting for the physical Alt release via GetAsyncKeyState instead of sending a
            // synthetic Alt-up). Registered here (not in `new`) because RegisterHotKey with
            // a NULL hwnd is thread-affine to whichever thread later pumps messages for it,
            // i.e. this one.
            let registered_ids = Self::register_modifier_combo_hotkeys();

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
                        WM_HOTKEY => {
                            handle_registered_hotkey(msg.wParam.0 as i32);
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

            for id in registered_ids {
                let _ = UnregisterHotKey(HWND::default(), id);
            }

            UnhookWindowsHookEx(hook)?;
        }

        Ok(())
    }

    /// Register `RegisterHotKey`-eligible (`ModifierCombo`) hotkeys with the OS and mark
    /// the corresponding `HotkeyState` so the low-level hook stops also detecting them.
    /// Falls back to hook-only detection (leaving the state unmarked) for a given hotkey
    /// if `RegisterHotKey` fails, e.g. because another application already owns that combo.
    unsafe fn register_modifier_combo_hotkeys() -> Vec<i32> {
        let mut registered_ids = Vec::new();

        if let Some(state) = TRANSLATE_HOTKEY.get() {
            if state.try_register_with_os(TRANSLATE_HOTKEY_ID) {
                registered_ids.push(TRANSLATE_HOTKEY_ID);
            }
        }

        if let Some(state) = SPEECH_HOTKEY.get() {
            if state.try_register_with_os(SPEECH_HOTKEY_ID) {
                registered_ids.push(SPEECH_HOTKEY_ID);
            }
        }

        registered_ids
    }
}

/// Convert a parsed `ModifierCombo`'s modifier vk codes into `RegisterHotKey`'s
/// `HOT_KEY_MODIFIERS` flags. Unrecognized codes are silently ignored, matching the
/// existing validation happening earlier in `HotkeyParser`.
fn modifiers_to_hotkey_flags(modifiers: &[u32]) -> HOT_KEY_MODIFIERS {
    let mut flags = MOD_NOREPEAT;
    for &m in modifiers {
        flags |= if m == super::keycodes::KEY_CONTROL {
            MOD_CONTROL
        } else if m == super::keycodes::KEY_ALT {
            MOD_ALT
        } else if m == super::keycodes::KEY_SHIFT {
            MOD_SHIFT
        } else if m == super::keycodes::KEY_LWIN || m == super::keycodes::KEY_RWIN {
            MOD_WIN
        } else {
            HOT_KEY_MODIFIERS(0)
        };
    }
    flags
}

/// Dispatch a `WM_HOTKEY` message (`wParam` is the id passed to `RegisterHotKey`) to the
/// matching trigger, applying the same speech-enabled gating as the hook-based path.
fn handle_registered_hotkey(id: i32) {
    if id == TRANSLATE_HOTKEY_ID {
        unsafe { trigger_translation() };
    } else if id == SPEECH_HOTKEY_ID {
        if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
            if speech_enabled.load(Ordering::Relaxed) {
                if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                    if tts_enabled.load(Ordering::Relaxed) {
                        unsafe { trigger_speech() };
                    }
                }
            }
        }
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
) -> Result<(), Box<dyn Error + Send + Sync>> {
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
    let provider = tagent::providers::create_provider(&config.translate_provider)?;
    let lang_code =
        tagent::providers::resolve_source_language(provider.as_ref(), &text, &source_code).await;

    // Clear any existing prompt and print speech info
    print!("\r");
    io::stdout().flush().ok();

    // Show speech label and speak
    SpeechManager::print_speech_label(&text, Some(&config.target_prompt_color));

    if let Err(e) = speech_manager
        .speak_text_with_cancel(provider.as_ref(), &text, &lang_code, stop_flag)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

    // NOTE: written but unverified locally -- this module only compiles under
    // `#[cfg(target_os = "windows")]`, so it cannot be run on the Linux dev machine.
    // Needs a run on Windows CI/hardware before this test is trusted.

    static TEST_TRIGGERED: AtomicBool = AtomicBool::new(false);

    // TEST_TRIGGERED and MODIFIER_STATE are process-global, and Rust runs tests in the same
    // binary on parallel threads by default, so every test in this module must serialize on
    // this lock before touching either one.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe fn test_trigger_fn() {
        TEST_TRIGGERED.store(true, AtomicOrdering::SeqCst);
    }

    #[test]
    fn test_hotkey_state_triggers_on_lr_specific_modifier() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Regression test for a parsed "LAlt+Q" config: modifier state is tracked keyed by
        // the normalized code (see `normalize_vk_code(vk_code)` in the ModifierCombo arm of
        // `HotkeyState::handle`), so `HotkeyParser::parse` must normalize the configured
        // modifier too, or this never triggers.
        MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("LAlt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        // Physical left Alt down - tracked, not yet triggered.
        state.handle(super::super::keycodes::KEY_LALT, true, test_trigger_fn);
        assert!(!TEST_TRIGGERED.load(AtomicOrdering::SeqCst));

        // Q down while left Alt held - combo should trigger.
        state.handle('Q' as u32, true, test_trigger_fn);
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    // Regression tests for item 1.3: a bare modifier press/release must never be blocked
    // (must return `false`), only the combo's target key blocks, and only once the full
    // combo actually fires. MODIFIER_STATE is a shared global, so each test resets the
    // specific keys it touches before asserting to avoid cross-test interference.

    #[test]
    fn test_modifier_press_alone_is_not_blocked() {
        let _guard = TEST_LOCK.lock().unwrap();
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state
            .lock()
            .unwrap()
            .insert(super::super::keycodes::KEY_ALT, false);
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        let blocked = state.handle(super::super::keycodes::KEY_ALT, true, test_trigger_fn);
        assert!(!blocked, "a bare modifier press must not be blocked");
        assert!(!TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_modifier_release_alone_is_not_blocked() {
        let _guard = TEST_LOCK.lock().unwrap();
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state
            .lock()
            .unwrap()
            .insert(super::super::keycodes::KEY_ALT, false);
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        state.handle(super::super::keycodes::KEY_ALT, true, test_trigger_fn);
        let blocked_up = state.handle(super::super::keycodes::KEY_ALT, false, test_trigger_fn);
        assert!(!blocked_up, "a bare modifier release must not be blocked");
        assert!(!TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_modifier_state_is_still_tracked_without_blocking() {
        let _guard = TEST_LOCK.lock().unwrap();
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state
            .lock()
            .unwrap()
            .insert(super::super::keycodes::KEY_ALT, false);
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        let blocked_down = state.handle(super::super::keycodes::KEY_ALT, true, test_trigger_fn);
        assert!(!blocked_down);

        // Even though the modifier press itself wasn't blocked, its state must still be
        // tracked so the combo can complete correctly.
        let triggered = state.handle('Q' as u32, true, test_trigger_fn);
        assert!(
            triggered,
            "combo must still fire after an unblocked modifier press"
        );
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_combo_completion_still_blocks_target_key() {
        let _guard = TEST_LOCK.lock().unwrap();
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state
            .lock()
            .unwrap()
            .insert(super::super::keycodes::KEY_ALT, false);
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        state.handle(super::super::keycodes::KEY_ALT, true, test_trigger_fn);
        let blocked = state.handle('Q' as u32, true, test_trigger_fn);
        assert!(
            blocked,
            "the target key must still be blocked once the combo fires"
        );
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    // Regression tests for the RegisterHotKey handoff: once a ModifierCombo has been
    // registered with the OS (see `KeyboardHook::register_modifier_combo_hotkeys`), the
    // hook-based `handle` path must stay out of the way entirely, or WM_HOTKEY and the
    // hook would both fire the same combo.

    #[test]
    fn test_registered_combo_is_never_detected_by_the_hook() {
        let _guard = TEST_LOCK.lock().unwrap();
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state
            .lock()
            .unwrap()
            .insert(super::super::keycodes::KEY_ALT, false);
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));
        state.registered_with_os.store(true, AtomicOrdering::SeqCst);

        // Same sequence that fires the combo in `test_combo_completion_still_blocks_target_key`
        // above -- but since RegisterHotKey now owns this combo, the hook must not trigger it
        // (that would double-fire the translation: once via WM_HOTKEY, once via the hook).
        let blocked_alt = state.handle(super::super::keycodes::KEY_ALT, true, test_trigger_fn);
        let blocked_q = state.handle('Q' as u32, true, test_trigger_fn);
        assert!(!blocked_alt);
        assert!(
            !blocked_q,
            "a combo handed off to RegisterHotKey must not also be detected by the hook"
        );
        assert!(!TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_modifiers_to_hotkey_flags_maps_each_modifier() {
        assert_eq!(
            modifiers_to_hotkey_flags(&[super::super::keycodes::KEY_ALT]),
            MOD_ALT | MOD_NOREPEAT
        );
        assert_eq!(
            modifiers_to_hotkey_flags(&[super::super::keycodes::KEY_CONTROL]),
            MOD_CONTROL | MOD_NOREPEAT
        );
        assert_eq!(
            modifiers_to_hotkey_flags(&[super::super::keycodes::KEY_SHIFT]),
            MOD_SHIFT | MOD_NOREPEAT
        );
        assert_eq!(
            modifiers_to_hotkey_flags(&[super::super::keycodes::KEY_LWIN]),
            MOD_WIN | MOD_NOREPEAT
        );
        assert_eq!(
            modifiers_to_hotkey_flags(&[
                super::super::keycodes::KEY_CONTROL,
                super::super::keycodes::KEY_SHIFT
            ]),
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT
        );
    }
}
