mod translator;
mod clipboard;
mod keyboard;

use translator::Translator;
use keyboard::KeyboardHook;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Переводчик запущен. Дважды нажмите Ctrl для перевода текста из буфера обмена.");
    println!("Нажмите Ctrl+C для выхода из программы.");
    
    let translator = Translator::new();
    let mut keyboard_hook = KeyboardHook::new(translator);
    
    // Создаем runtime для async операций
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(keyboard_hook.start())?;
    
    Ok(())
}
