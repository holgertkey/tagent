mod translator;
mod clipboard;
mod keyboard;
mod config;
mod window;
mod cli;
mod interactive;

use translator::Translator;
use keyboard::KeyboardHook;
use cli::CliHandler;
use interactive::InteractiveMode;
use std::env;
use windows::Win32::System::Console::{SetConsoleCtrlHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Отключаем стандартную обработку Ctrl+C в консоли Windows
    unsafe {
        SetConsoleCtrlHandler(None, true)?;
    }
    
    // Получаем аргументы командной строки
    let args: Vec<String> = env::args().collect();
    
    // Если есть аргументы, работаем в режиме CLI
    if args.len() > 1 {
        let cli_handler = match CliHandler::new() {
            Ok(handler) => handler,
            Err(e) => {
                println!("Failed to initialize CLI handler: {}", e);
                return Err(e);
            }
        };
        
        return cli_handler.process_args(args).await;
    }
    
    // Если аргументов нет, запускаем объединенный GUI+Interactive режим
    show_unified_mode_info();
    
    let translator = match Translator::new() {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to initialize translator: {}", e);
            return Err(e);
        }
    };
    
    // Создаем интерактивный режим
    let interactive_mode = match InteractiveMode::new() {
        Ok(mode) => mode,
        Err(e) => {
            println!("Failed to initialize interactive mode: {}", e);
            return Err(e);
        }
    };
    
    // Получаем общий флаг выхода
    let should_exit = interactive_mode.get_exit_flag();
    
    // Запускаем горячие клавиши в отдельном потоке
    let should_exit_clone = should_exit.clone();
    let keyboard_task = tokio::spawn(async move {
        let mut keyboard_hook = match KeyboardHook::new(translator, should_exit_clone) {
            Ok(hook) => hook,
            Err(e) => {
                println!("Failed to create keyboard hook: {}", e);
                return;
            }
        };
        
        if let Err(e) = keyboard_hook.start().await {
            println!("Keyboard hook error: {}", e);
        }
    });
    
    // Запускаем интерактивный режим в основном потоке
    let interactive_result = interactive_mode.start().await;
    
    // Устанавливаем флаг выхода для завершения keyboard hook
    should_exit.store(true, std::sync::atomic::Ordering::SeqCst);
    
    // Ждем завершения keyboard task
    let _ = keyboard_task.await;
    
    if let Err(e) = interactive_result {
        println!("Interactive mode error: {}", e);
    }
    
    // println!("Program terminated successfully.");
    Ok(())
}

/// Display unified mode information
fn show_unified_mode_info() {
    println!("=== Text Translator v{} ===", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Translation Methods:");
    println!();
    println!("1. Hotkeys (GUI Mode):");
    println!("   - Select text in any application");
    println!("   - Quickly double-press Ctrl (Ctrl + Ctrl) or press configured hotkey");
    println!("   - Default alternative hotkey: F9");
    println!("   - Configure custom hotkeys in tagent.conf [Hotkeys] section");
    println!("   - Translation will be copied to clipboard");
    println!("   - Prompt will automatically return after translation");
    println!();
    println!("2. Interactive Mode (Current Terminal):");
    println!("   - Type text directly below and press Enter");
    println!("   - Commands: -h (help), -c (config), -v (version), -q (quit)");
    println!("   - Single words show dictionary entries");
    println!("   - Phrases show translations");
    println!("   - Any text not recognized as command will be translated");
    println!();
    println!("CLI Mode:");
    println!("Run: tagent <text to translate>");
    println!("Examples:");
    println!("  tagent hello");
    println!("  tagent \"Hello world\"");
    println!("  tagent --help        (show detailed help)");
    println!("  tagent --config      (show current configuration)");
    println!("  tagent --version     (show version information)");
    println!();
    println!("Configuration:");
    println!("- Edit 'tagent.conf' to change translation languages");
    println!("- Set 'ShowDictionary = false' to disable dictionary lookup");
    println!("- Set 'CopyToClipboard = false' to display results only");
    println!("- Set 'SaveTranslationHistory = true' to log all translations");
    println!("- Changes take effect immediately (no restart required)");
    println!();
    println!("Exit: Type 'exit', 'quit', 'q', or '-q' below");
    println!("=====================================");
    println!();
}