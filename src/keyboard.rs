use crate::translator::Translator;
use crate::config::{ConfigManager, HotkeyType, HotkeyParser};
use crate::speech::SpeechManager;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::{
    Win32::Foundation::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
};

static TRANSLATOR: OnceLock<Arc<Translator>> = OnceLock::new();
static IS_PROCESSING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_EXIT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static TRANSLATE_HOTKEY_CONFIG: OnceLock<Arc<Mutex<Option<HotkeyType>>>> = OnceLock::new();
static MODIFIER_STATE: OnceLock<Arc<Mutex<HashMap<u32, bool>>>> = OnceLock::new();
static LAST_KEY_TIME: OnceLock<Arc<Mutex<Option<Instant>>>> = OnceLock::new();

// Speech hotkey variables
static SPEECH_HOTKEY_CONFIG: OnceLock<Arc<Mutex<Option<HotkeyType>>>> = OnceLock::new();
static SPEECH_HOTKEY_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static SPEECH_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
// Note: Speech hotkeys use shared MODIFIER_STATE (declared above with alternative hotkey vars)
static SPEECH_LAST_KEY_TIME: OnceLock<Arc<Mutex<Option<Instant>>>> = OnceLock::new();
static IS_SPEAKING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_STOP_SPEECH: OnceLock<Arc<AtomicBool>> = OnceLock::new();

pub struct KeyboardHook;

impl KeyboardHook {
    pub fn new(translator: Translator, should_exit: Arc<AtomicBool>) -> Result<Self, Box<dyn Error>> {
        TRANSLATOR.set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;

        let is_processing = Arc::new(Mutex::new(false));

        IS_PROCESSING.set(is_processing)
            .map_err(|_| "IsProcessing already initialized")?;
        SHOULD_EXIT.set(should_exit)
            .map_err(|_| "ShouldExit already initialized")?;

        // Initialize translation hotkey configuration
        let config_manager = ConfigManager::new(
            &ConfigManager::get_default_config_path()?.to_string_lossy()
        )?;
        let config = config_manager.get_config();

        let translate_hotkey = match HotkeyParser::parse(&config.translate_hotkey) {
            Ok(hotkey) => {
                match HotkeyParser::validate_hotkey(&hotkey) {
                    Ok(_) => Some(hotkey),
                    Err(e) => {
                        eprintln!("Warning: Hotkey validation failed for '{}': {}", config.translate_hotkey, e);
                        eprintln!("Translation hotkey disabled.");
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse hotkey '{}': {}", config.translate_hotkey, e);
                eprintln!("Translation hotkey disabled.");
                None
            }
        };

        TRANSLATE_HOTKEY_CONFIG.set(Arc::new(Mutex::new(translate_hotkey)))
            .map_err(|_| "TranslateHotkeyConfig already initialized")?;

        MODIFIER_STATE.set(Arc::new(Mutex::new(HashMap::new())))
            .map_err(|_| "ModifierState already initialized")?;

        LAST_KEY_TIME.set(Arc::new(Mutex::new(None)))
            .map_err(|_| "LastKeyTime already initialized")?;

        // Initialize speech hotkey configuration
        let speech_hotkey = if config.enable_speech_hotkey && config.enable_text_to_speech {
            match HotkeyParser::parse(&config.speech_hotkey) {
                Ok(hotkey) => {
                    match HotkeyParser::validate_hotkey(&hotkey) {
                        Ok(_) => Some(hotkey),
                        Err(e) => {
                            eprintln!("Warning: Speech hotkey validation failed for '{}': {}", config.speech_hotkey, e);
                            eprintln!("Speech hotkey disabled.");
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse speech hotkey '{}': {}", config.speech_hotkey, e);
                    eprintln!("Speech hotkey disabled.");
                    None
                }
            }
        } else {
            None
        };

        SPEECH_HOTKEY_CONFIG.set(Arc::new(Mutex::new(speech_hotkey)))
            .map_err(|_| "SpeechHotkeyConfig already initialized")?;

        SPEECH_HOTKEY_ENABLED.set(Arc::new(AtomicBool::new(config.enable_speech_hotkey)))
            .map_err(|_| "SpeechHotkeyEnabled already initialized")?;

        SPEECH_ENABLED.set(Arc::new(AtomicBool::new(config.enable_text_to_speech)))
            .map_err(|_| "SpeechEnabled already initialized")?;

        // Note: Speech hotkeys use shared MODIFIER_STATE, no need for separate state

        SPEECH_LAST_KEY_TIME.set(Arc::new(Mutex::new(None)))
            .map_err(|_| "SpeechLastKeyTime already initialized")?;

        IS_SPEAKING.set(Arc::new(Mutex::new(false)))
            .map_err(|_| "IsSpeaking already initialized")?;

        SHOULD_STOP_SPEECH.set(Arc::new(AtomicBool::new(false)))
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
                        // println!("Exit signal detected, breaking message loop");
                        break;
                    }
                }

                let mut msg = MSG::default();
                
                // Use PeekMessage instead of GetMessage to avoid blocking
                let has_message = PeekMessageW(&mut msg, HWND::default(), 0, 0, PEEK_MESSAGE_REMOVE_TYPE(1u32));
                
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

            // println!("Unhooking keyboard hook");
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
                return; // Already processing
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
                return; // Already speaking
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
async fn speak_clipboard(_translator: &Translator, stop_flag: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    use crate::clipboard::ClipboardManager;
    use crate::window::WindowManager;
    use std::io::{self, Write};
    use colored::Colorize;

    // Create clipboard manager
    let clipboard = ClipboardManager::new();

    // Copy selected text to clipboard
    clipboard.copy_selected_text()?;

    // Read text from clipboard
    let text = clipboard.get_text()?;
    if text.trim().is_empty() {
        // Get config for prompt color
        let config_manager = ConfigManager::new(
            &ConfigManager::get_default_config_path()?.to_string_lossy()
        )?;
        let config = config_manager.get_config();

        // Clear current line and print error message
        print!("\r");
        io::stdout().flush().ok();
        println!("Clipboard is empty, nothing to speak");

        // Show [Auto]: prompt on new line
        println!();
        let auto_prompt = "[Auto]: ";
        if let Some(color) = ConfigManager::parse_color(&config.auto_prompt_color) {
            print!("{}", auto_prompt.color(color));
        } else {
            print!("{}", auto_prompt);
        }
        io::stdout().flush().ok();

        return Ok(());
    }

    // Get language code from config
    let config_manager = ConfigManager::new(
        &ConfigManager::get_default_config_path()?.to_string_lossy()
    )?;

    let config = config_manager.get_config();

    // Show terminal window if configured
    if config.show_terminal_on_translate {
        match WindowManager::new() {
            Ok(window_manager) => {
                if let Err(e) = window_manager.show_terminal() {
                    println!("Failed to show terminal: {}", e);
                }
            }
            Err(e) => {
                println!("Failed to create window manager: {}", e);
            }
        }
    }

    // Detect or use source language
    let (source_code, _target_code) = config_manager.get_language_codes();

    // Use auto-detected language or source language for speech
    let lang_code = if source_code == "auto" {
        // Try to detect language from text
        // For now, default to English if auto
        "en"
    } else {
        &source_code
    };

    // Clear any existing prompt and print speech info
    print!("\r");
    io::stdout().flush().ok();

    // Show speech label
    let speech_label = "[Speech]: ";
    if let Some(color) = ConfigManager::parse_color(&config.translation_prompt_color) {
        print!("{}", speech_label.color(color));
    } else {
        print!("{}", speech_label);
    }
    println!("{}", text);

    // Call speech directly (blocking until completion or cancellation)
    let speech_manager = SpeechManager::new();
    match speech_manager.speak_text_with_cancel(&text, lang_code, stop_flag).await {
        Ok(_) => {
            // Speech completed successfully
        }
        Err(e) => {
            eprintln!("Speech error: {}", e);
        }
    }

    // Show [Auto]: prompt after speech completes
    println!(); // Add empty line after speech
    let auto_prompt = "[Auto]: ";
    if let Some(color) = ConfigManager::parse_color(&config.auto_prompt_color) {
        print!("{}", auto_prompt.color(color));
    } else {
        print!("{}", auto_prompt);
    }
    io::stdout().flush().ok();

    Ok(())
}

/// Normalize virtual key code (convert specific L/R codes to generic codes)
fn normalize_vk_code(vk_code: u32) -> u32 {
    match vk_code {
        162 | 163 => 17,  // VK_LCONTROL/VK_RCONTROL -> VK_CONTROL
        164 | 165 => 18,  // VK_LMENU/VK_RMENU -> VK_MENU
        160 | 161 => 16,  // VK_LSHIFT/VK_RSHIFT -> VK_SHIFT
        _ => vk_code,
    }
}

/// Handle speech hotkey detection
unsafe fn handle_speech_hotkey(vk_code: u32, is_key_down: bool) -> bool {
    if let Some(hotkey_config) = SPEECH_HOTKEY_CONFIG.get() {
        if let Ok(hotkey_opt) = hotkey_config.lock() {
            if let Some(hotkey) = hotkey_opt.as_ref() {
                match hotkey {
                    HotkeyType::SingleKey { vk_code: target_vk } => {
                        if is_key_down && vk_code == *target_vk {
                            trigger_speech();
                            return true;
                        }
                    }

                    HotkeyType::ModifierCombo { modifiers, key } => {
                        // Use shared MODIFIER_STATE instead of separate state
                        if let Some(modifier_state) = MODIFIER_STATE.get() {
                            if let Ok(mut state) = modifier_state.lock() {
                                let normalized_vk = normalize_vk_code(vk_code);

                                // Update modifier state and block modifier events
                                if modifiers.contains(&normalized_vk) {
                                    state.insert(normalized_vk, is_key_down);
                                    // Block modifier to prevent system sounds and menu activation
                                    return true;
                                }

                                // Check if all modifiers are pressed and the key is pressed
                                if is_key_down && vk_code == *key {
                                    let all_modifiers_pressed = modifiers.iter()
                                        .all(|m| state.get(m).copied().unwrap_or(false));

                                    if all_modifiers_pressed {
                                        trigger_speech();
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

                    HotkeyType::DoublePress { vk_code: target_vk, min_interval_ms, max_interval_ms } => {
                        let normalized_vk = normalize_vk_code(vk_code);
                        if is_key_down && normalized_vk == *target_vk {
                            if let Some(last_key_time) = SPEECH_LAST_KEY_TIME.get() {
                                if let Ok(mut last_time) = last_key_time.lock() {
                                    let now = Instant::now();

                                    match *last_time {
                                        Some(last) => {
                                            let elapsed = now.duration_since(last);
                                            if elapsed >= Duration::from_millis(*min_interval_ms) &&
                                               elapsed < Duration::from_millis(*max_interval_ms) {
                                                trigger_speech();
                                                *last_time = None;
                                                return true;
                                            } else if elapsed >= Duration::from_millis(*max_interval_ms) {
                                                *last_time = Some(now);
                                            }
                                        }
                                        None => {
                                            *last_time = Some(now);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Handle translation hotkey detection
unsafe fn handle_translate_hotkey(vk_code: u32, is_key_down: bool) -> bool {
    if let Some(hotkey_config) = TRANSLATE_HOTKEY_CONFIG.get() {
        if let Ok(hotkey_opt) = hotkey_config.lock() {
            if let Some(hotkey) = hotkey_opt.as_ref() {
                match hotkey {
                    HotkeyType::SingleKey { vk_code: target_vk } => {
                        if is_key_down && vk_code == *target_vk {
                            trigger_translation();
                            return true;
                        }
                    }

                    HotkeyType::ModifierCombo { modifiers, key } => {
                        if let Some(modifier_state) = MODIFIER_STATE.get() {
                            if let Ok(mut state) = modifier_state.lock() {
                                let normalized_vk = normalize_vk_code(vk_code);

                                // Update modifier state and block modifier events to prevent system sounds
                                if modifiers.contains(&normalized_vk) {
                                    state.insert(normalized_vk, is_key_down);
                                    // Block modifier key events (especially Alt) to prevent menu activation
                                    // and system sounds
                                    return true;
                                }

                                // Check if all modifiers are pressed and the key is pressed
                                if is_key_down && vk_code == *key {
                                    let all_modifiers_pressed = modifiers.iter()
                                        .all(|m| state.get(m).copied().unwrap_or(false));

                                    if all_modifiers_pressed {
                                        trigger_translation();
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

                    HotkeyType::DoublePress { vk_code: target_vk, min_interval_ms, max_interval_ms } => {
                        let normalized_vk = normalize_vk_code(vk_code);
                        if is_key_down && normalized_vk == *target_vk {
                            if let Some(last_key_time) = LAST_KEY_TIME.get() {
                                if let Ok(mut last_time) = last_key_time.lock() {
                                    let now = Instant::now();

                                    match *last_time {
                                        Some(last) => {
                                            let elapsed = now.duration_since(last);
                                            if elapsed >= Duration::from_millis(*min_interval_ms) &&
                                               elapsed < Duration::from_millis(*max_interval_ms) {
                                                trigger_translation();
                                                *last_time = None;
                                                return true;
                                            } else if elapsed >= Duration::from_millis(*max_interval_ms) {
                                                *last_time = Some(now);
                                            }
                                        }
                                        None => {
                                            *last_time = Some(now);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

unsafe extern "system" fn keyboard_hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 {
        let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);

        // Ignore injected events (simulated by keybd_event, SendInput, etc.)
        // This allows our copy_selected_text() Ctrl+C simulation to work
        const LLKHF_INJECTED: u32 = 0x10;
        if (kbd_struct.flags.0 & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        if w_param.0 as u32 == WM_KEYDOWN || w_param.0 as u32 == WM_SYSKEYDOWN {
            // Handle Esc key to stop speech
            if kbd_struct.vkCode == VK_ESCAPE.0 as u32 {
                if let Some(is_speaking) = IS_SPEAKING.get() {
                    if let Ok(speaking) = is_speaking.lock() {
                        if *speaking {
                            // Stop speech playback
                            if let Some(stop_flag) = SHOULD_STOP_SPEECH.get() {
                                stop_flag.store(true, Ordering::Relaxed);
                                println!("Speech cancelled by user (Esc)");
                            }
                            return LRESULT(1); // Block Esc to prevent other actions
                        }
                    }
                }
            }

            // Handle translation hotkey
            if handle_translate_hotkey(kbd_struct.vkCode, true) {
                // Block the event - don't pass it to other applications
                return LRESULT(1);
            }

            // Handle speech hotkey if enabled
            if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                if speech_enabled.load(Ordering::Relaxed) {
                    if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                        if tts_enabled.load(Ordering::Relaxed) {
                            if handle_speech_hotkey(kbd_struct.vkCode, true) {
                                // Block the event - don't pass it to other applications
                                return LRESULT(1);
                            }
                        }
                    }
                }
            }
        } else if w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP {
            // Handle translation hotkey key up (for modifier state tracking)
            if handle_translate_hotkey(kbd_struct.vkCode, false) {
                // Block the key up event to match the blocked key down event
                return LRESULT(1);
            }

            // Handle speech hotkey key up (for modifier state tracking)
            if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                if speech_enabled.load(Ordering::Relaxed) {
                    if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                        if tts_enabled.load(Ordering::Relaxed) {
                            if handle_speech_hotkey(kbd_struct.vkCode, false) {
                                // Block the key up event to match the blocked key down event
                                return LRESULT(1);
                            }
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}