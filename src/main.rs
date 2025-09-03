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
    
    println!("=== Text Translator v0.5.0 ===");
    println!("Usage instructions:");
    println!("1. Select text in any application");
    println!("2. Quickly double-press Ctrl (Ctrl + Ctrl)");
    println!("3. For single words: dictionary entry will be shown and copied to clipboard");
    println!("4. For phrases: translation will be copied to clipboard");
    println!("5. Paste result where needed with Ctrl+V");
    println!();
    println!("Configuration:");
    println!("- Edit 'translator.conf' to change translation languages");
    println!("- Set 'ShowDictionary = false' to disable dictionary lookup for single words");
    println!("- Set 'ShowTerminalOnTranslate = true' to show terminal window during translation");
    println!("- Set 'AutoHideTerminalSeconds = N' to auto-hide terminal after N seconds (0 = no auto-hide)");
    println!("- Changes take effect immediately (no restart required)");
    println!();
    println!("New in v0.5.0:");
    println!("- Dictionary lookup for single words (definitions, part of speech, examples)");
    println!("- Compact format for easy reading");
    println!("- Automatic fallback to translation if dictionary lookup fails");
    println!();
    println!("Program runs in background. Press F12 to exit.");
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