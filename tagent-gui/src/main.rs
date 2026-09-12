use slint::{ComponentHandle, Model};
use std::sync::{Arc, Mutex};
use tagent::{languages, providers};

mod config;

use config::GuiConfigManager;

slint::include_modules!();

fn scroll_transcript_to_bottom(window: &AppWindow) {
    let overflow = window.get_transcript_viewport_height() - window.get_transcript_visible_height();
    window.set_transcript_viewport_y(if overflow > 0.0 { -overflow } else { 0.0 });
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let window = AppWindow::new()?;

    let config_manager = Arc::new(Mutex::new(GuiConfigManager::new()));

    window.invoke_apply_theme(config_manager.lock().unwrap().config().theme.clone().into());

    let config_manager_for_settings = config_manager.clone();
    let weak = window.as_weak();
    window.on_translate_requested(move |text, from_lang, to_lang| {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let from = languages::name_to_code(&from_lang).to_string();
        let to = languages::name_to_code(&to_lang).to_string();
        if to == "auto" {
            if let Some(window) = weak.upgrade() {
                let entry = format!(
                    "[{from_lang}]: {text}\nError: \"Auto\" is not a valid target language\n\n"
                );
                let transcript = window.get_transcript();
                window.set_transcript(format!("{transcript}{entry}").into());
                scroll_transcript_to_bottom(&window);
            }
            return;
        }

        let translate_provider = {
            let mut manager = config_manager.lock().unwrap();
            manager.check_and_reload();
            manager.config().translate_provider.clone()
        };

        let weak = weak.clone();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
            let request_text = text.clone();
            let result = runtime.block_on(async move {
                let provider = providers::create_provider(&translate_provider)?;
                provider.translate_text(&request_text, &from, &to).await
            });

            slint::invoke_from_event_loop(move || {
                if let Some(window) = weak.upgrade() {
                    let entry = match result {
                        Ok(translated) => {
                            format!("[{from_lang}]: {text}\n[{to_lang}]: {translated}\n\n")
                        }
                        Err(err) => format!("[{from_lang}]: {text}\nError: {err}\n\n"),
                    };
                    let transcript = window.get_transcript();
                    window.set_transcript(format!("{transcript}{entry}").into());
                    scroll_transcript_to_bottom(&window);
                }
            })
            .ok();
        });
    });

    let weak = window.as_weak();
    window.on_swap_requested(move || {
        if let Some(window) = weak.upgrade() {
            let source = window.get_source_language_index();
            let target = window.get_target_language_index();
            window.set_source_language_index(target);
            // "Auto" (index 0) is only valid as a source language; fall back to
            // "English" (index 1) rather than making it the new target.
            window.set_target_language_index(if source == 0 { 1 } else { source });
        }
    });

    let window_weak_for_settings = window.as_weak();
    window.on_settings_requested(move || {
        let dialog = SettingsDialog::new().unwrap();
        dialog.set_app_version(env!("CARGO_PKG_VERSION").into());

        let current_config = config_manager_for_settings.lock().unwrap().config().clone();

        let providers = dialog.get_providers();
        let provider_index = providers
            .iter()
            .position(|p| p.as_str() == current_config.translate_provider)
            .unwrap_or(0);
        dialog.set_provider_index(provider_index as i32);

        let themes = dialog.get_themes();
        let theme_index = themes
            .iter()
            .position(|t| t.as_str().to_lowercase() == current_config.theme)
            .unwrap_or(0);
        dialog.set_theme_index(theme_index as i32);
        // The dialog gets its own Palette instance (globals aren't shared
        // between windows) — apply the currently-active theme to it too, or
        // it would render in the system default regardless of what's saved.
        dialog.invoke_apply_theme(current_config.theme.clone().into());

        let dialog_weak = dialog.as_weak();
        let config_manager_for_save = config_manager_for_settings.clone();
        let window_weak_for_save = window_weak_for_settings.clone();
        dialog.on_save_requested(move |provider, theme| {
            let theme = theme.to_lowercase();
            let new_config = config::GuiConfig {
                translate_provider: provider.to_string(),
                theme: theme.clone(),
            };
            if let Err(err) = config_manager_for_save.lock().unwrap().update(new_config) {
                eprintln!("Warning: failed to save tagent-gui.json: {err}");
            }
            if let Some(window) = window_weak_for_save.upgrade() {
                window.invoke_apply_theme(theme.clone().into());
            }
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.hide().ok();
            }
        });

        dialog.show().unwrap();
    });

    window.run()?;
    Ok(())
}
