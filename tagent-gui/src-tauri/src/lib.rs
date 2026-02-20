mod commands;

use commands::AppState;
use std::sync::Arc;
use tagent::{config::ConfigManager, speech::SpeechManager};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize shared state
            let config_path = ConfigManager::get_default_config_path()
                .expect("Failed to get config path");
            let config_manager = Arc::new(
                ConfigManager::new(config_path.to_string_lossy().as_ref())
                    .expect("Failed to load config"),
            );
            let speech_manager = Arc::new(SpeechManager::new());

            app.manage(AppState {
                config_manager,
                speech_manager,
            });

            // Build system tray
            build_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate,
            commands::get_config,
            commands::save_config,
            commands::get_languages,
            commands::speak,
            commands::swap_languages,
            commands::copy_to_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("Error running tagent-gui");
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("Tagent v{}", version);

    let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let about_i = MenuItem::with_id(app, "about", &title, false, None::<&str>)?;
    let sep2 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_i, &sep1, &about_i, &sep2, &quit_i])?;

    let handle = app.handle().clone();
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Tagent — Text Translator")
        .menu(&menu)
        .on_menu_event(move |_tray, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = handle.get_webview_window("main") {
                    win.show().ok();
                    win.set_focus().ok();
                }
            }
            "quit" => {
                handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    if win.is_visible().unwrap_or(false) {
                        win.hide().ok();
                    } else {
                        win.show().ok();
                        win.set_focus().ok();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
