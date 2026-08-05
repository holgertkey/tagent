use slint::ComponentHandle;
use tagent::{config::ConfigManager, providers};

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = AppWindow::new()?;

    let weak = window.as_weak();
    window.on_translate_requested(move |text, from_lang, to_lang| {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let weak = weak.clone();
        let from = ConfigManager::language_to_code(&from_lang).to_string();
        let to = ConfigManager::language_to_code(&to_lang).to_string();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
            let request_text = text.clone();
            let result = runtime.block_on(async move {
                let provider = providers::create_provider("google")?;
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
