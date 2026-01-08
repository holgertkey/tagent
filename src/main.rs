mod cli;
mod clipboard;
mod config;
mod interactive;
mod keyboard;
mod speech;
mod translator;
mod window;

use cli::CliHandler;
use interactive::InteractiveMode;
use keyboard::KeyboardHook;
use std::env;
use translator::Translator;
use windows::Win32::System::Console::SetConsoleCtrlHandler;

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
    use config::ConfigManager;

    println!("Text Translator v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // Load config to show active hotkeys
    if let Ok(config_path) = ConfigManager::get_default_config_path() {
        if let Ok(config_manager) = ConfigManager::new(config_path.to_string_lossy().as_ref()) {
            let config = config_manager.get_config();

            println!("Active Hotkeys:");
            println!("  Translation: {}", config.translate_hotkey);
            if config.enable_speech_hotkey && config.enable_text_to_speech {
                println!("  Speech: {}", config.speech_hotkey);
            }
            println!();
        }
    }

    println!(
        "Commands: /h (help), /c (config), /v (version), /s (speech), /q (quit), /cls (clear)"
    );
    println!();
}
