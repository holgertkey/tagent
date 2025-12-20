use crate::translator::Translator;
use crate::config::{ConfigManager, HotkeyType, HotkeyParser};
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
static LAST_CTRL_TIME: OnceLock<Arc<Mutex<Option<Instant>>>> = OnceLock::new();
static IS_PROCESSING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_EXIT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static CTRL_IS_PRESSED: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static ALT_HOTKEY_CONFIG: OnceLock<Arc<Mutex<Option<HotkeyType>>>> = OnceLock::new();
static ALT_HOTKEY_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static MODIFIER_STATE: OnceLock<Arc<Mutex<HashMap<u32, bool>>>> = OnceLock::new();
static LAST_KEY_TIME: OnceLock<Arc<Mutex<Option<Instant>>>> = OnceLock::new();

pub struct KeyboardHook;

impl KeyboardHook {
    pub fn new(translator: Translator, should_exit: Arc<AtomicBool>) -> Result<Self, Box<dyn Error>> {
        TRANSLATOR.set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;
        
        let last_ctrl_time = Arc::new(Mutex::new(None));
        let is_processing = Arc::new(Mutex::new(false));
        let ctrl_is_pressed = Arc::new(Mutex::new(false));
        
        LAST_CTRL_TIME.set(last_ctrl_time)
            .map_err(|_| "LastCtrlTime already initialized")?;
        IS_PROCESSING.set(is_processing)
            .map_err(|_| "IsProcessing already initialized")?;
        SHOULD_EXIT.set(should_exit)
            .map_err(|_| "ShouldExit already initialized")?;
        CTRL_IS_PRESSED.set(ctrl_is_pressed)
            .map_err(|_| "CtrlIsPressed already initialized")?;

        // Initialize alternative hotkey configuration
        let config_manager = ConfigManager::new(
            &ConfigManager::get_default_config_path()?.to_string_lossy()
        )?;
        let config = config_manager.get_config();

        let alt_hotkey = if config.enable_alternative_hotkey {
            match HotkeyParser::parse(&config.alternative_hotkey) {
                Ok(hotkey) => {
                    match HotkeyParser::validate_hotkey(&hotkey) {
                        Ok(_) => Some(hotkey),
                        Err(e) => {
                            eprintln!("Warning: Hotkey validation failed for '{}': {}", config.alternative_hotkey, e);
                            eprintln!("Alternative hotkey disabled. Using Ctrl+Ctrl only.");
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse hotkey '{}': {}", config.alternative_hotkey, e);
                    eprintln!("Alternative hotkey disabled. Using Ctrl+Ctrl only.");
                    None
                }
            }
        } else {
            None
        };

        ALT_HOTKEY_CONFIG.set(Arc::new(Mutex::new(alt_hotkey)))
            .map_err(|_| "AltHotkeyConfig already initialized")?;

        ALT_HOTKEY_ENABLED.set(Arc::new(AtomicBool::new(config.enable_alternative_hotkey)))
            .map_err(|_| "AltHotkeyEnabled already initialized")?;

        MODIFIER_STATE.set(Arc::new(Mutex::new(HashMap::new())))
            .map_err(|_| "ModifierState already initialized")?;

        LAST_KEY_TIME.set(Arc::new(Mutex::new(None)))
            .map_err(|_| "LastKeyTime already initialized")?;

        Ok(Self)
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;

            if hook.0 == 0 {
                return Err("Failed to set keyboard hook".into());
            }

            println!("Keyboard hook set successfully");
            println!();

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

/// Normalize virtual key code (convert specific L/R codes to generic codes)
fn normalize_vk_code(vk_code: u32) -> u32 {
    match vk_code {
        162 | 163 => 17,  // VK_LCONTROL/VK_RCONTROL -> VK_CONTROL
        164 | 165 => 18,  // VK_LMENU/VK_RMENU -> VK_MENU
        160 | 161 => 16,  // VK_LSHIFT/VK_RSHIFT -> VK_SHIFT
        _ => vk_code,
    }
}

/// Handle alternative hotkey detection
unsafe fn handle_alternative_hotkey(vk_code: u32, is_key_down: bool) -> bool {
    if let Some(hotkey_config) = ALT_HOTKEY_CONFIG.get() {
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

                                // Update modifier state
                                if modifiers.contains(&normalized_vk) {
                                    state.insert(normalized_vk, is_key_down);
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
            // Handle Ctrl key for double-press detection
            if kbd_struct.vkCode == VK_LCONTROL.0 as u32 || kbd_struct.vkCode == VK_RCONTROL.0 as u32 {
                if let (Some(_translator), Some(last_ctrl_time), Some(is_processing), Some(ctrl_is_pressed)) =
                    (TRANSLATOR.get(), LAST_CTRL_TIME.get(), IS_PROCESSING.get(), CTRL_IS_PRESSED.get()) {

                    if let (Ok(mut last_time), Ok(processing), Ok(mut is_pressed)) =
                        (last_ctrl_time.lock(), is_processing.lock(), ctrl_is_pressed.lock()) {
                        
                        // If Ctrl is already pressed, this is a key repeat event - ignore it
                        if *is_pressed {
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }
                        
                        // Mark Ctrl as pressed
                        *is_pressed = true;
                        
                        // If already processing a translation, ignore
                        if *processing {
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }

                        let now = Instant::now();
                        
                        match *last_time {
                            Some(last) => {
                                let time_since_last = now.duration_since(last);
                                
                                if time_since_last >= Duration::from_millis(50) &&
                                   time_since_last < Duration::from_millis(500) {

                                    // Double Ctrl - trigger translation
                                    *last_time = None;
                                    drop(last_time); // Release lock before trigger_translation
                                    drop(processing);
                                    drop(is_pressed);

                                    // println!("Double Ctrl detected ({}ms apart)", time_since_last.as_millis());
                                    trigger_translation();
                                    // Block the event - don't pass it to other applications
                                    return LRESULT(1);
                                } else if time_since_last < Duration::from_millis(50) {
                                    println!("Ctrl press too fast ({}ms) - ignoring contact bounce", time_since_last.as_millis());
                                } else {
                                    // Too slow - treat as new first press
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

            // Handle alternative hotkey if enabled
            if let Some(alt_enabled) = ALT_HOTKEY_ENABLED.get() {
                if alt_enabled.load(Ordering::Relaxed) {
                    if handle_alternative_hotkey(kbd_struct.vkCode, true) {
                        // Block the event - don't pass it to other applications
                        return LRESULT(1);
                    }
                }
            }
        } else if w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP {
            // Handle Ctrl key up - mark as not pressed
            if kbd_struct.vkCode == VK_LCONTROL.0 as u32 || kbd_struct.vkCode == VK_RCONTROL.0 as u32 {
                if let Some(ctrl_is_pressed) = CTRL_IS_PRESSED.get() {
                    if let Ok(mut is_pressed) = ctrl_is_pressed.lock() {
                        *is_pressed = false;
                    }
                }
            }

            // Handle alternative hotkey key up (for modifier state tracking)
            if let Some(alt_enabled) = ALT_HOTKEY_ENABLED.get() {
                if alt_enabled.load(Ordering::Relaxed) {
                    handle_alternative_hotkey(kbd_struct.vkCode, false);
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}