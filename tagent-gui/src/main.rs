use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};
use std::sync::{Arc, Mutex};
use tagent::{languages, providers};

mod config;

use config::GuiConfigManager;

slint::include_modules!();

/// Font-family choices offered for the phrase/translation style pickers in
/// Settings > View, in the same order as `SettingsDialog.font-options`.
const FONT_FAMILIES: [&str; 3] = ["monospace", "sans-serif", "serif"];

fn font_index_for(family: &str) -> i32 {
    FONT_FAMILIES
        .iter()
        .position(|f| *f == family)
        .unwrap_or(0) as i32
}

/// One preset entry for the "Color scheme" picker in Settings > View: a
/// one-shot bulk-fill for background/phrase/translation colors (plus the
/// matching Theme), not a persisted setting of its own — see
/// `SettingsDialog.color-scheme-options` in app.slint.
struct ColorScheme {
    name: &'static str,
    dark: bool,
    background: &'static str,
    phrase_color: &'static str,
    phrase_background: &'static str,
    translation_color: &'static str,
    translation_background: &'static str,
}

const COLOR_SCHEMES: &[ColorScheme] = &[
    ColorScheme {
        name: "Solarized Dark",
        dark: true,
        background: "#002B36",
        phrase_color: "#839496",
        phrase_background: "#073642",
        translation_color: "#268BD2",
        translation_background: "#073642",
    },
    ColorScheme {
        name: "Solarized Light",
        dark: false,
        background: "#FDF6E3",
        phrase_color: "#657B83",
        phrase_background: "#EEE8D5",
        translation_color: "#268BD2",
        translation_background: "#EEE8D5",
    },
    ColorScheme {
        name: "Dracula",
        dark: true,
        background: "#282A36",
        phrase_color: "#F8F8F2",
        phrase_background: "#44475A",
        translation_color: "#BD93F9",
        translation_background: "#44475A",
    },
    ColorScheme {
        name: "Nord",
        dark: true,
        background: "#2E3440",
        phrase_color: "#D8DEE9",
        phrase_background: "#3B4252",
        translation_color: "#88C0D0",
        translation_background: "#3B4252",
    },
    ColorScheme {
        name: "Gruvbox Dark",
        dark: true,
        background: "#282828",
        phrase_color: "#EBDBB2",
        phrase_background: "#3C3836",
        translation_color: "#FE8019",
        translation_background: "#3C3836",
    },
    ColorScheme {
        name: "Monokai",
        dark: true,
        background: "#272822",
        phrase_color: "#F8F8F2",
        phrase_background: "#3E3D32",
        translation_color: "#A6E22E",
        translation_background: "#3E3D32",
    },
    ColorScheme {
        name: "One Dark",
        dark: true,
        background: "#282C34",
        phrase_color: "#ABB2BF",
        phrase_background: "#2C313C",
        translation_color: "#61AFEF",
        translation_background: "#2C313C",
    },
];

/// Parses a `"#RRGGBB"` (or `"RRGGBB"`) string into 0-255 components.
fn parse_hex_color(text: &str) -> Option<(u8, u8, u8)> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&text[0..2], 16).ok()?;
    let g = u8::from_str_radix(&text[2..4], 16).ok()?;
    let b = u8::from_str_radix(&text[4..6], 16).ok()?;
    Some((r, g, b))
}

fn format_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn resolve_color(hex: &str, default: Color) -> Color {
    match parse_hex_color(hex) {
        Some((r, g, b)) => Color::from_rgb_u8(r, g, b),
        None => default,
    }
}

/// Reads a color-picker field's dialog state back into a `"#RRGGBB"` config
/// value, or `""` when the field is left at "Theme default".
fn color_field_hex(use_default: bool, r: f32, g: f32, b: f32) -> String {
    if use_default {
        String::new()
    } else {
        format_hex(r.round() as u8, g.round() as u8, b.round() as u8)
    }
}

/// Applies the theme and the phrase/translation display style from `config`
/// to `window`. Colors left at "theme default" (empty string in config) are
/// resolved against the window's own theme-driven `panel-foreground`/
/// `panel-background`, read back *after* the theme is applied so an "auto"
/// theme resolves to whatever the system's current dark/light preference is.
///
/// `panel-background` itself is resolved first (against the raw theme value,
/// `panel-background-theme-default`) since it's now also user-customizable —
/// phrase-background/translation-background's own "theme default" then
/// follows whatever `panel-background` ends up being, custom or not, so the
/// three stay visually consistent.
fn apply_style(window: &AppWindow, config: &config::GuiConfig) {
    window.invoke_apply_theme(config.theme.clone().into());

    let panel_background_theme_default = window.get_panel_background_theme_default().color();
    window.set_panel_background(resolve_color(
        &config.background_color,
        panel_background_theme_default,
    ));

    let default_fg = window.get_panel_foreground().color();
    let default_bg = window.get_panel_background();

    window.set_phrase_font(config.phrase_font.clone().into());
    window.set_phrase_size(config.phrase_size);
    window.set_phrase_color(resolve_color(&config.phrase_color, default_fg));
    window.set_phrase_background(resolve_color(&config.phrase_background, default_bg));

    window.set_translation_font(config.translation_font.clone().into());
    window.set_translation_size(config.translation_size);
    window.set_translation_color(resolve_color(&config.translation_color, default_fg));
    window.set_translation_background(resolve_color(&config.translation_background, default_bg));

    window.set_block_spacing_px(config.block_spacing_px);
    window.set_phrases_spacing_px(config.phrases_spacing_px);
}

/// Formats one transcript line, with or without its "[Auto]:"-style prompt.
///
/// The prompt is baked directly into the string (rather than kept as a
/// separately styled element) because Slint's plain `Text`/`TextInput` can't
/// mix two styles within one wrapped paragraph — an earlier attempt at a
/// separately colored/sized prompt element made the text wrap flush under the
/// prompt (hanging indent) instead of flush from the row's left margin like a
/// normal paragraph, which didn't match the desired look.
fn format_line(show_prompt: bool, lang: &str, text: &str) -> String {
    if show_prompt {
        format!("[{lang}]: {text}")
    } else {
        text.to_string()
    }
}

/// Populates one `ColorPickerField`'s dialog-side state from a `"#RRGGBB"` (or
/// empty, for "theme default") config value. Used five times (the shared
/// panel background, plus phrase/translation × text/background) — see the
/// matching macro call sites below.
macro_rules! init_color_field {
    ($dialog:expr, $hex:expr, $set_default:ident, $set_r:ident, $set_g:ident, $set_b:ident, $set_hex:ident) => {{
        let hex_value = $hex;
        match parse_hex_color(hex_value) {
            Some((r, g, b)) => {
                $dialog.$set_default(false);
                $dialog.$set_r(r as f32);
                $dialog.$set_g(g as f32);
                $dialog.$set_b(b as f32);
                $dialog.$set_hex(hex_value.to_string().into());
            }
            None => {
                $dialog.$set_default(true);
            }
        }
    }};
}

/// Wires one `ColorPickerField`'s `hex-committed` callback: parses the typed
/// hex text and reflects it into the field's RGB sliders. Used five times.
macro_rules! wire_hex_committed {
    ($dialog:expr, $on_committed:ident, $set_r:ident, $set_g:ident, $set_b:ident, $set_default:ident) => {{
        let dialog_weak = $dialog.as_weak();
        $dialog.$on_committed(move |text| {
            if let (Some(dialog), Some((r, g, b))) =
                (dialog_weak.upgrade(), parse_hex_color(text.as_str()))
            {
                dialog.$set_r(r as f32);
                dialog.$set_g(g as f32);
                dialog.$set_b(b as f32);
                dialog.$set_default(false);
            }
        });
    }};
}

/// Wires one `ColorPickerField`'s `rgb-changed` callback (fired on every
/// slider drag): reformats the field's current red/green/blue into
/// `"#RRGGBB"` and writes it back into the hex text, so the hex field doesn't
/// go stale while dragging sliders. Used five times.
macro_rules! wire_rgb_changed {
    ($dialog:expr, $on_changed:ident, $get_r:ident, $get_g:ident, $get_b:ident, $set_hex:ident) => {{
        let dialog_weak = $dialog.as_weak();
        $dialog.$on_changed(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                let hex = format_hex(
                    dialog.$get_r().round() as u8,
                    dialog.$get_g().round() as u8,
                    dialog.$get_b().round() as u8,
                );
                dialog.$set_hex(hex.into());
            }
        });
    }};
}

fn scroll_transcript_to_bottom(window: &AppWindow) {
    let overflow = window.get_transcript_viewport_height() - window.get_transcript_visible_height();
    window.set_transcript_viewport_y(if overflow > 0.0 { -overflow } else { 0.0 });
}

/// Appends one entry to the transcript and scrolls to show it.
///
/// Rebuilds the backing model rather than mutating a shared `VecModel`
/// because the new entry is produced on a background translation thread and
/// handed back via `slint::invoke_from_event_loop`, whose closure must be
/// `Send` — an `Rc<VecModel<_>>` captured in that closure wouldn't be, but
/// `AppWindow::as_weak()` is, so entries are read back from the window itself
/// instead of a separately shared model.
fn push_transcript_entry(window: &AppWindow, entry: TranscriptEntry) {
    let mut entries: Vec<TranscriptEntry> = window.get_transcript_entries().iter().collect();
    entries.push(entry);
    window.set_transcript_entries(ModelRc::new(VecModel::from(entries)));
    scroll_transcript_to_bottom(window);
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let window = AppWindow::new()?;

    let config_manager = Arc::new(Mutex::new(GuiConfigManager::new()));

    apply_style(&window, config_manager.lock().unwrap().config());

    let config_manager_for_settings = config_manager.clone();
    let weak = window.as_weak();
    window.on_translate_requested(move |text, from_lang, to_lang| {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let from = languages::name_to_code(&from_lang).to_string();
        let to = languages::name_to_code(&to_lang).to_string();

        let (translate_provider, show_prompt) = {
            let mut manager = config_manager.lock().unwrap();
            manager.check_and_reload();
            let cfg = manager.config();
            (cfg.translate_provider.clone(), cfg.show_prompt)
        };

        if to == "auto" {
            if let Some(window) = weak.upgrade() {
                push_transcript_entry(
                    &window,
                    TranscriptEntry {
                        phrase: format_line(show_prompt, &from_lang, &text).into(),
                        translation: "Error: \"Auto\" is not a valid target language".into(),
                    },
                );
            }
            return;
        }

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
                        Ok(translated) => TranscriptEntry {
                            phrase: format_line(show_prompt, &from_lang, &text).into(),
                            translation: format_line(show_prompt, &to_lang, &translated).into(),
                        },
                        Err(err) => TranscriptEntry {
                            phrase: format_line(show_prompt, &from_lang, &text).into(),
                            translation: format!("Error: {err}").into(),
                        },
                    };
                    push_transcript_entry(&window, entry);
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

        dialog.set_block_spacing_px(current_config.block_spacing_px);
        dialog.set_phrases_spacing_px(current_config.phrases_spacing_px);

        dialog.set_phrase_font_index(font_index_for(&current_config.phrase_font));
        dialog.set_phrase_size(current_config.phrase_size);
        dialog.set_translation_font_index(font_index_for(&current_config.translation_font));
        dialog.set_translation_size(current_config.translation_size);

        dialog.set_show_prompt(current_config.show_prompt);

        init_color_field!(
            dialog,
            current_config.background_color.as_str(),
            set_background_use_default,
            set_background_red,
            set_background_green,
            set_background_blue,
            set_background_hex
        );
        init_color_field!(
            dialog,
            current_config.phrase_color.as_str(),
            set_phrase_color_use_default,
            set_phrase_color_red,
            set_phrase_color_green,
            set_phrase_color_blue,
            set_phrase_color_hex
        );
        init_color_field!(
            dialog,
            current_config.phrase_background.as_str(),
            set_phrase_bg_use_default,
            set_phrase_bg_red,
            set_phrase_bg_green,
            set_phrase_bg_blue,
            set_phrase_bg_hex
        );
        init_color_field!(
            dialog,
            current_config.translation_color.as_str(),
            set_translation_color_use_default,
            set_translation_color_red,
            set_translation_color_green,
            set_translation_color_blue,
            set_translation_color_hex
        );
        init_color_field!(
            dialog,
            current_config.translation_background.as_str(),
            set_translation_bg_use_default,
            set_translation_bg_red,
            set_translation_bg_green,
            set_translation_bg_blue,
            set_translation_bg_hex
        );
        wire_hex_committed!(
            dialog,
            on_background_hex_committed,
            set_background_red,
            set_background_green,
            set_background_blue,
            set_background_use_default
        );
        wire_hex_committed!(
            dialog,
            on_phrase_color_hex_committed,
            set_phrase_color_red,
            set_phrase_color_green,
            set_phrase_color_blue,
            set_phrase_color_use_default
        );
        wire_hex_committed!(
            dialog,
            on_phrase_bg_hex_committed,
            set_phrase_bg_red,
            set_phrase_bg_green,
            set_phrase_bg_blue,
            set_phrase_bg_use_default
        );
        wire_hex_committed!(
            dialog,
            on_translation_color_hex_committed,
            set_translation_color_red,
            set_translation_color_green,
            set_translation_color_blue,
            set_translation_color_use_default
        );
        wire_hex_committed!(
            dialog,
            on_translation_bg_hex_committed,
            set_translation_bg_red,
            set_translation_bg_green,
            set_translation_bg_blue,
            set_translation_bg_use_default
        );
        wire_rgb_changed!(
            dialog,
            on_background_rgb_changed,
            get_background_red,
            get_background_green,
            get_background_blue,
            set_background_hex
        );
        wire_rgb_changed!(
            dialog,
            on_phrase_color_rgb_changed,
            get_phrase_color_red,
            get_phrase_color_green,
            get_phrase_color_blue,
            set_phrase_color_hex
        );
        wire_rgb_changed!(
            dialog,
            on_phrase_bg_rgb_changed,
            get_phrase_bg_red,
            get_phrase_bg_green,
            get_phrase_bg_blue,
            set_phrase_bg_hex
        );
        wire_rgb_changed!(
            dialog,
            on_translation_color_rgb_changed,
            get_translation_color_red,
            get_translation_color_green,
            get_translation_color_blue,
            set_translation_color_hex
        );
        wire_rgb_changed!(
            dialog,
            on_translation_bg_rgb_changed,
            get_translation_bg_red,
            get_translation_bg_green,
            get_translation_bg_blue,
            set_translation_bg_hex
        );

        let dialog_weak = dialog.as_weak();
        dialog.on_color_scheme_selected(move |name| {
            let Some(dialog) = dialog_weak.upgrade() else {
                return;
            };
            let Some(scheme) = COLOR_SCHEMES.iter().find(|s| s.name == name.as_str()) else {
                return;
            };

            init_color_field!(
                dialog,
                scheme.background,
                set_background_use_default,
                set_background_red,
                set_background_green,
                set_background_blue,
                set_background_hex
            );
            init_color_field!(
                dialog,
                scheme.phrase_color,
                set_phrase_color_use_default,
                set_phrase_color_red,
                set_phrase_color_green,
                set_phrase_color_blue,
                set_phrase_color_hex
            );
            init_color_field!(
                dialog,
                scheme.phrase_background,
                set_phrase_bg_use_default,
                set_phrase_bg_red,
                set_phrase_bg_green,
                set_phrase_bg_blue,
                set_phrase_bg_hex
            );
            init_color_field!(
                dialog,
                scheme.translation_color,
                set_translation_color_use_default,
                set_translation_color_red,
                set_translation_color_green,
                set_translation_color_blue,
                set_translation_color_hex
            );
            init_color_field!(
                dialog,
                scheme.translation_background,
                set_translation_bg_use_default,
                set_translation_bg_red,
                set_translation_bg_green,
                set_translation_bg_blue,
                set_translation_bg_hex
            );

            let target_theme = if scheme.dark { "dark" } else { "light" };
            let themes = dialog.get_themes();
            if let Some(index) = themes.iter().position(|t| t.as_str().to_lowercase() == target_theme) {
                dialog.set_theme_index(index as i32);
            }
            dialog.invoke_apply_theme(target_theme.into());
        });

        let dialog_weak = dialog.as_weak();
        let config_manager_for_save = config_manager_for_settings.clone();
        let window_weak_for_save = window_weak_for_settings.clone();
        dialog.on_save_requested(move |provider, theme| {
            let Some(dialog) = dialog_weak.upgrade() else {
                return;
            };

            let new_config = config::GuiConfig {
                translate_provider: provider.to_string(),
                theme: theme.to_lowercase(),
                background_color: color_field_hex(
                    dialog.get_background_use_default(),
                    dialog.get_background_red(),
                    dialog.get_background_green(),
                    dialog.get_background_blue(),
                ),
                phrase_font: FONT_FAMILIES
                    [dialog.get_phrase_font_index().clamp(0, FONT_FAMILIES.len() as i32 - 1) as usize]
                    .to_string(),
                phrase_size: dialog.get_phrase_size(),
                phrase_color: color_field_hex(
                    dialog.get_phrase_color_use_default(),
                    dialog.get_phrase_color_red(),
                    dialog.get_phrase_color_green(),
                    dialog.get_phrase_color_blue(),
                ),
                phrase_background: color_field_hex(
                    dialog.get_phrase_bg_use_default(),
                    dialog.get_phrase_bg_red(),
                    dialog.get_phrase_bg_green(),
                    dialog.get_phrase_bg_blue(),
                ),
                translation_font: FONT_FAMILIES[dialog
                    .get_translation_font_index()
                    .clamp(0, FONT_FAMILIES.len() as i32 - 1)
                    as usize]
                    .to_string(),
                translation_size: dialog.get_translation_size(),
                translation_color: color_field_hex(
                    dialog.get_translation_color_use_default(),
                    dialog.get_translation_color_red(),
                    dialog.get_translation_color_green(),
                    dialog.get_translation_color_blue(),
                ),
                translation_background: color_field_hex(
                    dialog.get_translation_bg_use_default(),
                    dialog.get_translation_bg_red(),
                    dialog.get_translation_bg_green(),
                    dialog.get_translation_bg_blue(),
                ),
                block_spacing_px: dialog.get_block_spacing_px(),
                phrases_spacing_px: dialog.get_phrases_spacing_px(),
                show_prompt: dialog.get_show_prompt(),
            };

            if let Err(err) = config_manager_for_save.lock().unwrap().update(new_config.clone()) {
                eprintln!("Warning: failed to save tagent-gui.json: {err}");
            }
            if let Some(window) = window_weak_for_save.upgrade() {
                apply_style(&window, &new_config);
            }
            dialog.hide().ok();
        });

        dialog.show().unwrap();
    });

    window.run()?;
    Ok(())
}
