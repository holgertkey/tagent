use crate::translator::Translator;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::{
    Win32::Foundation::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::UI::WindowsAndMessaging::*,
};

// Глобальные переменные для хранения состояния
static mut TRANSLATOR: Option<Arc<Translator>> = None;
static mut LAST_CTRL_TIME: Option<Arc<Mutex<Option<Instant>>>> = None;
static mut IS_PROCESSING: Option<Arc<Mutex<bool>>> = None;

pub struct KeyboardHook {
    translator: Translator,
    last_ctrl_time: Arc<Mutex<Option<Instant>>>,
    is_processing: Arc<Mutex<bool>>,
}

impl KeyboardHook {
    pub fn new(translator: Translator) -> Self {
        Self {
            translator,
            last_ctrl_time: Arc::new(Mutex::new(None)),
            is_processing: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn start(&mut self) -> std::result::Result<(), Box<dyn Error>> {
        // Инициализируем глобальные переменные
        unsafe {
            TRANSLATOR = Some(Arc::new(Translator::new()));
            LAST_CTRL_TIME = Some(self.last_ctrl_time.clone());
            IS_PROCESSING = Some(self.is_processing.clone());
        }

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
                
                if let (Some(translator), Some(last_ctrl_time), Some(is_processing)) = 
                    (&TRANSLATOR, &LAST_CTRL_TIME, &IS_PROCESSING) {
                    
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
                                            *processing_clone.lock().unwrap() = false;
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