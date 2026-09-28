mod cli;
mod config;
mod interactive;
mod platform;
mod speech;
mod translator;

use cli::CliHandler;
use config::ConfigManager;
use interactive::InteractiveMode;
use platform::KeyboardHook;
use std::env;
use std::sync::Arc;
use translator::Translator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set up platform-specific signal handling
    platform::signals::setup()?;

    // Get command-line arguments
    let args: Vec<String> = env::args().collect();

    // If arguments are provided, run in CLI mode
    if args.len() > 1 {
        let cli_handler = match CliHandler::new() {
            Ok(handler) => handler,
            Err(e) => {
                // Printed with `Display` and exited here: returning the error from `main`
                // would print it again with `Debug`, which mangles a multi-line TOML error.
                eprintln!("Failed to initialize CLI handler: {}", e);
                std::process::exit(1);
            }
        };

        return cli_handler.process_args(args).await;
    }

    // No arguments - start unified GUI+Interactive mode

    // Create shared ConfigManager
    let config_path = ConfigManager::get_default_config_path()?;
    let config_manager = match ConfigManager::new(config_path.to_string_lossy().as_ref()) {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let translator = match Translator::new_with_config(config_manager.clone()) {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to initialize translator: {}", e);
            return Err(e);
        }
    };

    // Banner: language pair, providers and active hotkeys. After the translator is built,
    // so it names the providers actually in use.
    let config = config_manager.get_config();
    ConfigManager::display_banner(&config, &translator.active_providers(&config));

    // Create interactive mode with shared translator
    let interactive_mode =
        InteractiveMode::with_translator(translator.clone(), config_manager.clone());

    // Get shared exit flag
    let should_exit = interactive_mode.get_exit_flag();

    // Start keyboard hook in a separate thread
    let should_exit_clone = should_exit.clone();
    let config_manager_clone = config_manager.clone();
    let keyboard_task = tokio::spawn(async move {
        let mut keyboard_hook =
            match KeyboardHook::new(translator, should_exit_clone, config_manager_clone) {
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

    // Start interactive mode in the main thread
    let interactive_result = interactive_mode.start().await;

    // Set the exit flag to terminate the keyboard hook
    should_exit.store(true, std::sync::atomic::Ordering::SeqCst);

    // Wait for keyboard task to finish
    let _ = keyboard_task.await;

    if let Err(e) = interactive_result {
        println!("Interactive mode error: {}", e);
    }

    Ok(())
}
