use crate::translator::Translator;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::{
    Win32::Foundation::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
};

// Используем OnceLock вместо мutable static для безопасности
static TRANSLATOR: OnceLock<Arc<Translator>> = OnceLock::new();
static LAST_CTRL_TIME: OnceLock<Arc<Mutex<Option<Instant>>>> = OnceLock::new();
static IS_PROCESSING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();

pub struct KeyboardHook;

impl KeyboardHook {
    pub fn new(translator: Translator) -> Result<Self, Box<dyn Error>> {
        // Инициализируем глобальные переменные один раз
        TRANSLATOR.set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;
        
        let last_ctrl_time = Arc::new(Mutex::new(None));
        let is_processing = Arc::new(Mutex::new(false));
        
        LAST_CTRL_TIME.set(last_ctrl_time)
            .map_err(|_| "LastCtrlTime already initialized")?;
        IS_PROCESSING.set(is_processing)
            .map_err(|_| "IsProcessing already initialized")?;
        
        Ok(Self)
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;

            if hook.0 == 0 {
                return Err("Не удалось установить хук клавиатуры".into());
            }

            println!("Хук клавиатуры установлен успешно");

            // Главный цикл обработки сообщений (синхронный)
            loop {
                let mut msg = MSG::default();
                let bret = GetMessageW(&mut msg, HWND::default(), 0, 0);
                
                match bret.0 {
                    0 => break, // WM_QUIT
                    -1 => {
                        println!("Ошибка в GetMessageW");
                        break;
                    }
                    _ => {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }

            UnhookWindowsHookEx(hook)?;
        }

        Ok(())
    }
}

// Процедура хука клавиатуры
unsafe extern "system" fn keyboard_hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 {
        // Проверяем, что это событие нажатия клавиши
        if w_param.0 as u32 == WM_KEYDOWN {
            let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
            
            // Проверяем, что нажат Ctrl (VK_LCONTROL или VK_RCONTROL)
            if kbd_struct.vkCode == VK_LCONTROL.0 as u32 || kbd_struct.vkCode == VK_RCONTROL.0 as u32 {
                let now = Instant::now();
                
                // Безопасный доступ к глобальным переменным через OnceLock
                if let (Some(translator), Some(last_ctrl_time), Some(is_processing)) = 
                    (TRANSLATOR.get(), LAST_CTRL_TIME.get(), IS_PROCESSING.get()) {
                    
                    if let (Ok(mut last_time), Ok(mut processing)) = 
                        (last_ctrl_time.lock(), is_processing.lock()) {
                        
                        // Если уже обрабатываем предыдущий запрос, игнорируем
                        if *processing {
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }

                        match *last_time {
                            Some(last) => {
                                // Проверяем, прошло ли менее 500 мс с последнего нажатия Ctrl
                                if now.duration_since(last) < Duration::from_millis(500) {
                                    *processing = true;
                                    *last_time = None;
                                    
                                    // Запускаем перевод в отдельной задаче
                                    let translator_clone = translator.clone();
                                    let processing_clone = is_processing.clone();
                                    
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            if let Err(e) = translator_clone.translate_clipboard().await {
                                                println!("Ошибка при переводе: {}", e);
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