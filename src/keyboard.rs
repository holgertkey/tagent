use crate::translator::Translator;
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

pub struct KeyboardHook;

impl KeyboardHook {
    pub fn new(translator: Translator, should_exit: Arc<AtomicBool>) -> Result<Self, Box<dyn Error>> {
        TRANSLATOR.set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;
        
        let last_ctrl_time = Arc::new(Mutex::new(None));
        let is_processing = Arc::new(Mutex::new(false));
        
        LAST_CTRL_TIME.set(last_ctrl_time)
            .map_err(|_| "LastCtrlTime already initialized")?;
        IS_PROCESSING.set(is_processing)
            .map_err(|_| "IsProcessing already initialized")?;
        SHOULD_EXIT.set(should_exit)
            .map_err(|_| "ShouldExit already initialized")?;
        
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

            loop {
                // Check if we should exit
                if let Some(should_exit) = SHOULD_EXIT.get() {
                    if should_exit.load(Ordering::Relaxed) {
                        println!("Exit signal detected, breaking message loop");
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

            println!("Unhooking keyboard hook");
            UnhookWindowsHookEx(hook)?;
        }

        Ok(())
    }
}

unsafe extern "system" fn keyboard_hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 {
        if w_param.0 as u32 == WM_KEYDOWN {
            let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
            
            if kbd_struct.vkCode == VK_LCONTROL.0 as u32 || kbd_struct.vkCode == VK_RCONTROL.0 as u32 {
                let now = Instant::now();
                
                if let (Some(translator), Some(last_ctrl_time), Some(is_processing)) = 
                    (TRANSLATOR.get(), LAST_CTRL_TIME.get(), IS_PROCESSING.get()) {
                    
                    if let (Ok(mut last_time), Ok(mut processing)) = 
                        (last_ctrl_time.lock(), is_processing.lock()) {
                        
                        if *processing {
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }

                        match *last_time {
                            Some(last) => {
                                if now.duration_since(last) < Duration::from_millis(500) {
                                    *processing = true;
                                    *last_time = None;
                                    
                                    let translator_clone = translator.clone();
                                    let processing_clone = is_processing.clone();
                                    
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            if let Err(e) = translator_clone.translate_clipboard().await {
                                                println!("Translation error: {}", e);
                                            }
                                            if let Ok(mut proc) = processing_clone.lock() {
                                                *proc = false;
                                            }
                                        });
                                    });
                                } else {
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

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}