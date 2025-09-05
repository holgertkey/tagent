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
        // Check for interactive mode flag
        if args.len() == 2 && (args[1] == "-i" || args[1] == "--interactive") {
            let interactive_mode = match InteractiveMode::new() {
                Ok(mode) => mode,
                Err(e) => {
                    println!("Failed to initialize interactive mode: {}", e);
                    return Err(e);
                }
            };
            
            return interactive_mode.start().await;
        }
        
        let cli_handler = match CliHandler::new() {
            Ok(handler) => handler,
            Err(e) => {
                println!("Failed to initialize CLI handler: {}", e);
                return Err(e);
            }
        };
        
        return cli_handler.process_args(args).await;
    }
    
    // Если аргументов нет, работаем в обычном режиме с горячими клавишами
    show_gui_mode_info();
    
    let should_exit = Arc::new(AtomicBool::new(false));
    
    let translator = match Translator::new() {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to initialize translator: {}", e);
            return Err(e);
        }
    };
    
    let mut keyboard_hook = KeyboardHook::new(translator, should_exit)?;
    
    keyboard_hook.start().await?;
    println!("Program terminated successfully.");
    
    Ok(())
}

/// Display GUI mode information
fn show_gui_mode_info() {
    println!("=== Text Translator v0.7.0 ===");
    println!();
    println!("Usage instructions:");
    println!();
    println!("GUI Mode (Current):");
    println!("1. Select text in any application");
    println!("2. Quickly double-press Ctrl (Ctrl + Ctrl)");
    println!("3. For single words: dictionary entry will be shown and copied to clipboard");
    println!("4. For phrases: translation will be copied to clipboard");
    println!("5. Paste result where needed with Ctrl+V");
    println!();
    println!("Interactive Mode:");
    println!("Run: tagent -i  or  tagent --interactive");
    println!("- Type text directly in terminal for translation");
    println!("- Same features as GUI mode, but with prompt interface");
    println!("- GUI hotkeys still work in background");
    println!("- Type 'help' for interactive commands");
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
    println!("- Set 'ShowDictionary = false' to disable dictionary lookup for single words");
    println!("- Set 'CopyToClipboard = false' to display results only (without copying to clipboard)");
    println!("- Set 'ShowTerminalOnTranslate = true' to show terminal window during translation");
    println!("- Set 'AutoHideTerminalSeconds = N' to auto-hide terminal after N seconds (0 = no auto-hide)");
    println!("- Changes take effect immediately (no restart required)");
    println!();
    println!("New in v0.7.0:");
    println!("- Interactive mode: Type text directly in terminal for translation");
    println!("- Run 'tagent -i' to start interactive prompt mode");
    println!("- Interactive commands: help, config, clear, exit");
    println!("- GUI hotkeys remain active in interactive mode");
    println!();
    println!("Features from v0.6.0:");
    println!("- Command-line interface for direct text translation");
    println!("- Enhanced help system with --help, --config, --version options");
    println!();
    println!("Features from v0.5.0:");
    println!("- Dictionary lookup for single words (definitions, part of speech, examples)");
    println!("- Compact format for easy reading");
    println!("- Automatic fallback to translation if dictionary lookup fails");
    println!("- Optional clipboard copying (can be disabled in config)");
    println!();
    println!("Program runs in background. Press F12 to exit.");
    println!("=====================================");
}