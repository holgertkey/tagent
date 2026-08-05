use slint::ComponentHandle;
use tagent::{config::ConfigManager, providers};

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;

    let weak = window.as_weak();
    window.on_translate_requested(move |text, from_lang, to_lang| {
        let weak = weak.clone();
        let from = ConfigManager::language_to_code(&from_lang).to_string();
        let to = ConfigManager::language_to_code(&to_lang).to_string();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
            let result = runtime.block_on(async move {
                let provider = providers::create_provider("google")?;
                provider.translate_text(text.trim(), &from, &to).await
            });

            let display = match result {
                Ok(translated) => translated,
                Err(err) => format!("Error: {err}"),
            };

            slint::invoke_from_event_loop(move || {
                if let Some(window) = weak.upgrade() {
                    window.set_translated_text(display.into());
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
            window.set_target_language_index(source);
        }
    });

    window.run()?;
    Ok(())
}
