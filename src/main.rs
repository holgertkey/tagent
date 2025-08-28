mod translator;
mod clipboard;
mod keyboard;

use translator::Translator;
use keyboard::KeyboardHook;

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
    
    let translator = Translator::new();
    let mut keyboard_hook = KeyboardHook::new(translator);
    
    // Создаем runtime для async операций
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(keyboard_hook.start())?;
    
    Ok(())
}
