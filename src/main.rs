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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
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
    
    let should_exit = Arc::new(AtomicBool::new(false));
    
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
    
    // Запускаем горячие клавиши в отдельном потоке
    let should_exit_clone = should_exit.clone();
    let mut keyboard_hook = KeyboardHook::new(translator, should_exit_clone)?;
    
    let keyboard_task = tokio::spawn(async move {
        if let Err(e) = keyboard_hook.start().await {
            println!("Keyboard hook error: {}", e);
        }
    });
    
    // Запускаем интерактивный режим в основном потоке
    let interactive_task = tokio::spawn(async move {
        if let Err(e) = interactive_mode.start().await {
            println!("Interactive mode error: {}", e);
        }
    });
    
    // Ждем завершения любой из задач
    tokio::select! {
        _ = keyboard_task => {
            println!("Keyboard hook terminated");
        }
        _ = interactive_task => {
            println!("Interactive mode terminated");
        }
    }
    
    // Устанавливаем флаг выхода для завершения всех потоков
    should_exit.store(true, std::sync::atomic::Ordering::SeqCst);
    
    println!("Program terminated successfully.");
    Ok(())
}

/// Display unified mode information
fn show_unified_mode_info() {
    println!("=== Text Translator v0.7.0 ===");
    println!();
    println!("Translation Methods:");
    println!();
    println!("1. Hotkeys (GUI Mode):");
    println!("   - Select text in any application");
    println!("   - Quickly double-press Ctrl (Ctrl + Ctrl)");
    println!("   - Translation will be copied to clipboard");
    println!();
    println!("2. Interactive Mode (Current Terminal):");
    println!("   - Type text directly below and press Enter");
    println!("   - Commands: help, config, clear, exit");
    println!("   - Single words show dictionary entries");
    println!("   - Phrases show translations");
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
    println!("- Changes take effect immediately (no restart required)");
    println!();
    println!("New in v0.7.0:");
    println!("- Unified interface: Both hotkeys AND interactive prompt work simultaneously");
    println!("- Type text below for translation or use Ctrl+Ctrl hotkeys");
    println!("- Interactive commands available in terminal");
    println!();
    println!("Exit: Press F12 or type 'exit' below");
    println!("=====================================");
    println!();
}