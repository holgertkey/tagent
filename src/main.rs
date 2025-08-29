mod translator;
mod clipboard;
mod keyboard;

use translator::Translator;
use keyboard::KeyboardHook;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Переводчик текста ===");
    println!("Инструкции по использованию:");
    println!("1. Выделите английский текст в любом приложении");
    println!("2. Дважды быстро нажмите Ctrl (Ctrl + Ctrl)");
    println!("3. Текст автоматически скопируется, переведется и сохранится в буфер обмена");
    println!("4. Вставьте перевод в нужное место с помощью Ctrl+V");
    println!();
    println!("Программа работает в фоновом режиме. Нажмите Ctrl+C для выхода.");
    println!("=====================================");
    
    // Устанавливаем обработчик Ctrl+C для корректного завершения
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        println!("\nПолучен сигнал завершения. Закрываем программу...");
        r.store(false, Ordering::SeqCst);
        
        // Отправляем WM_QUIT в очередь сообщений для корректного завершения цикла
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{PostQuitMessage};
            PostQuitMessage(0);
        }
    })?;
    
    let translator = Translator::new();
    let mut keyboard_hook = KeyboardHook::new(translator)?;
    
    // Создаем runtime для async операций
    let rt = tokio::runtime::Runtime::new()?;
    
    match rt.block_on(keyboard_hook.start()) {
        Ok(_) => {
            println!("Программа завершена успешно.");
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("WM_QUIT") {
                println!("Программа завершена пользователем.");
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}
