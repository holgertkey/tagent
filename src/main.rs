mod translator;
mod clipboard;
mod keyboard;
mod config;
mod window;

use translator::Translator;
use keyboard::KeyboardHook;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use windows::Win32::System::Console::{SetConsoleCtrlHandler};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Отключаем стандартную обработку Ctrl+C в консоли Windows
    unsafe {
        SetConsoleCtrlHandler(None, true)?;
    }
    
    println!("=== Text Translator ===");
    println!("Usage instructions:");
    println!("1. Select text in any application");
    println!("2. Quickly double-press Ctrl (Ctrl + Ctrl)");
    println!("3. Text will automatically copy, translate, and save to clipboard");
    println!("4. Paste translation where needed with Ctrl+V");
    println!();
    println!("Configuration:");
    println!("- Edit 'translator.conf' to change translation languages");
    println!("- Set 'ShowTerminalOnTranslate = true' to show terminal window during translation");
    println!("- Set 'AutoHideTerminalSeconds = N' to auto-hide terminal after N seconds (0 = no auto-hide)");
    println!("- Changes take effect immediately (no restart required)");
    println!();
    println!("Program runs in background. Press Ctrl+Q to exit.");
    println!("=====================================");
    
    let should_exit = Arc::new(AtomicBool::new(false));
    
    let translator = match Translator::new() {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to initialize translator: {}", e);
            return Err(e);
        }
    };
    
    let mut keyboard_hook = KeyboardHook::new(translator, should_exit)?;
    
    let rt = tokio::runtime::Runtime::new()?;
    
    rt.block_on(keyboard_hook.start())?;
    println!("Program terminated successfully.");
    
    Ok(())
}