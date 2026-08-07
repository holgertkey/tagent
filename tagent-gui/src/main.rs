use slint::ComponentHandle;
use tagent::{languages, providers};

slint::include_modules!();

fn scroll_transcript_to_bottom(window: &AppWindow) {
    let overflow = window.get_transcript_viewport_height() - window.get_transcript_visible_height();
    window.set_transcript_viewport_y(if overflow > 0.0 { -overflow } else { 0.0 });
}

/// Reads `TranslateProvider` from `[Provider]` in `tagent.conf`, defaulting to `"google"`.
///
/// This is a small, deliberate exception to reusing tagent-cli's `ConfigManager`: depending
/// on tagent-cli here would pull in rustyline, rdev, x11, arboard, and the whole platform/
/// tree just to read one string. It is not a full INI parser — just this one key.
fn read_translate_provider() -> String {
    const DEFAULT_PROVIDER: &str = "google";

    let Some(config_dir) = dirs::config_dir() else {
        return DEFAULT_PROVIDER.to_string();
    };
    let path = config_dir.join("Tagent").join("tagent.conf");

    let Ok(content) = std::fs::read_to_string(&path) else {
        return DEFAULT_PROVIDER.to_string();
    };

    let mut in_provider_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_provider_section = &line[1..line.len() - 1] == "Provider";
            continue;
        }
        if in_provider_section {
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();
                if key == "TranslateProvider" && !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }

    DEFAULT_PROVIDER.to_string()
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let window = AppWindow::new()?;

    let translate_provider = read_translate_provider();

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

        let weak = weak.clone();
        let translate_provider = translate_provider.clone();

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

    window.run()?;
    Ok(())
}
