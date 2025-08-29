mod translator;
mod clipboard;
mod keyboard;

use translator::Translator;
use keyboard::KeyboardHook;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Text Translator ===");
    println!("Usage instructions:");
    println!("1. Select English text in any application");
    println!("2. Quickly double-press Ctrl (Ctrl + Ctrl)");
    println!("3. Text will automatically copy, translate, and save to clipboard");
    println!("4. Paste translation where needed with Ctrl+V");
    println!();
    println!("Program runs in background. Press Ctrl+C to exit.");
    println!("=====================================");
    
    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_clone = should_exit.clone();
    
    ctrlc::set_handler(move || {
        println!("\nShutdown signal received. Closing program...");
        should_exit_clone.store(true, Ordering::SeqCst);
        
        // Also post WM_QUIT to break out of the message loop
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
            PostQuitMessage(0);
        }
    })?;
    
    let translator = Translator::new();
    let mut keyboard_hook = KeyboardHook::new(translator, should_exit)?;
    
    let rt = tokio::runtime::Runtime::new()?;
    
    rt.block_on(keyboard_hook.start())?;
    println!("Program terminated successfully.");
    
    Ok(())
}
