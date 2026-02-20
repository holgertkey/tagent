use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tagent::{
    config::{ConfigManager, Config},
    providers,
    speech::SpeechManager,
};
use tauri::State;

// ─── Shared app state ────────────────────────────────────────────────────────

pub struct AppState {
    pub config_manager: Arc<ConfigManager>,
    pub speech_manager: Arc<SpeechManager>,
}

// ─── IPC data types ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TranslationResult {
    pub original: String,
    pub translated: String,
    pub from_lang: String,
    pub to_lang: String,
    pub is_dictionary: bool,
    pub dictionary: Option<DictionaryData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DictionaryData {
    pub word: String,
    pub entries: Vec<PosEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PosEntry {
    pub part_of_speech: String,
    pub definitions: Vec<DefData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DefData {
    pub text: String,
    pub synonyms: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigData {
    pub source_language: String,
    pub target_language: String,
    pub show_dictionary: bool,
    pub copy_to_clipboard: bool,
    pub save_translation_history: bool,
    pub translate_hotkey: String,
    pub speech_hotkey: String,
    pub enable_text_to_speech: bool,
    pub enable_speech_hotkey: bool,
    pub translate_provider: String,
    pub history_file: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LanguageEntry {
    pub name: String,
    pub code: String,
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Translate text and return structured result
#[tauri::command]
pub async fn translate(
    text: String,
    from_lang: String,
    to_lang: String,
    state: State<'_, AppState>,
) -> Result<TranslationResult, String> {
    if text.trim().is_empty() {
        return Err("Empty input".into());
    }

    let config = state.config_manager.get_config();
    let provider = providers::create_provider(&config.translate_provider)
        .map_err(|e| e.to_string())?;

    // Try dictionary for single words when enabled
    if config.show_dictionary && is_single_word(text.trim()) {
        if let Ok(Some(entry)) = provider
            .get_dictionary_entry(text.trim(), &from_lang, &to_lang)
            .await
        {
            let dictionary = DictionaryData {
                word: entry.word.clone(),
                entries: entry
                    .definitions
                    .iter()
                    .map(|pos| PosEntry {
                        part_of_speech: pos.part_of_speech.clone(),
                        definitions: pos
                            .definitions
                            .iter()
                            .map(|d| DefData {
                                text: d.text.clone(),
                                synonyms: d.synonyms.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
            };

            // Also get the simple translation for the translated field
            let translated = provider
                .translate_text(text.trim(), &from_lang, &to_lang)
                .await
                .unwrap_or_else(|_| entry.word.clone());

            let result = TranslationResult {
                original: text.trim().to_string(),
                translated,
                from_lang,
                to_lang,
                is_dictionary: true,
                dictionary: Some(dictionary),
            };

            save_history_if_enabled(&result, &config);
            return Ok(result);
        }
    }

    // Regular translation
    let translated = provider
        .translate_text(text.trim(), &from_lang, &to_lang)
        .await
        .map_err(|e| e.to_string())?;

    let result = TranslationResult {
        original: text.trim().to_string(),
        translated,
        from_lang,
        to_lang,
        is_dictionary: false,
        dictionary: None,
    };

    save_history_if_enabled(&result, &config);
    Ok(result)
}

/// Get current configuration
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<ConfigData, String> {
    let config = state.config_manager.get_config();
    Ok(config_to_data(&config))
}

/// Save configuration
#[tauri::command]
pub fn save_config(config: ConfigData, state: State<'_, AppState>) -> Result<(), String> {
    {
        let manager = &state.config_manager;
        let mut current = manager.get_config();
        current.source_language = config.source_language;
        current.target_language = config.target_language;
        current.show_dictionary = config.show_dictionary;
        current.copy_to_clipboard = config.copy_to_clipboard;
        current.save_translation_history = config.save_translation_history;
        current.translate_hotkey = config.translate_hotkey;
        current.speech_hotkey = config.speech_hotkey;
        current.enable_text_to_speech = config.enable_text_to_speech;
        current.enable_speech_hotkey = config.enable_speech_hotkey;
        current.translate_provider = config.translate_provider;
        current.history_file = config.history_file;

        manager.set_config(current);
    }
    state
        .config_manager
        .save_config()
        .map_err(|e| e.to_string())
}

/// Return list of all supported languages
#[tauri::command]
pub fn get_languages() -> Vec<LanguageEntry> {
    vec![
        LanguageEntry { name: "Auto".into(),       code: "auto".into() },
        LanguageEntry { name: "English".into(),    code: "en".into() },
        LanguageEntry { name: "Russian".into(),    code: "ru".into() },
        LanguageEntry { name: "Spanish".into(),    code: "es".into() },
        LanguageEntry { name: "French".into(),     code: "fr".into() },
        LanguageEntry { name: "German".into(),     code: "de".into() },
        LanguageEntry { name: "Chinese".into(),    code: "zh".into() },
        LanguageEntry { name: "Japanese".into(),   code: "ja".into() },
        LanguageEntry { name: "Korean".into(),     code: "ko".into() },
        LanguageEntry { name: "Italian".into(),    code: "it".into() },
        LanguageEntry { name: "Portuguese".into(), code: "pt".into() },
        LanguageEntry { name: "Dutch".into(),      code: "nl".into() },
        LanguageEntry { name: "Polish".into(),     code: "pl".into() },
        LanguageEntry { name: "Turkish".into(),    code: "tr".into() },
        LanguageEntry { name: "Arabic".into(),     code: "ar".into() },
        LanguageEntry { name: "Hindi".into(),      code: "hi".into() },
    ]
}

/// Speak text with TTS
#[tauri::command]
pub async fn speak(
    text: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .speech_manager
        .speak_text_full(&text, &state.config_manager)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Swap source and target languages in config (in memory only)
#[tauri::command]
pub fn swap_languages(state: State<'_, AppState>) -> Result<(String, String), String> {
    let config = state.config_manager.get_config();
    let mut source = config.source_language.clone();
    let target = config.target_language.clone();

    if source.to_lowercase() == "auto" {
        source = "English".to_string();
    }

    state.config_manager.set_languages(&target, &source);
    Ok((target, source))
}

/// Copy text to clipboard
#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    use tagent::platform::ClipboardManager;
    let cb = ClipboardManager::new();
    cb.set_text(&text).map_err(|e| e.to_string())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn is_single_word(text: &str) -> bool {
    text.split_whitespace().count() == 1
        && text.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

fn config_to_data(config: &Config) -> ConfigData {
    ConfigData {
        source_language: config.source_language.clone(),
        target_language: config.target_language.clone(),
        show_dictionary: config.show_dictionary,
        copy_to_clipboard: config.copy_to_clipboard,
        save_translation_history: config.save_translation_history,
        translate_hotkey: config.translate_hotkey.clone(),
        speech_hotkey: config.speech_hotkey.clone(),
        enable_text_to_speech: config.enable_text_to_speech,
        enable_speech_hotkey: config.enable_speech_hotkey,
        translate_provider: config.translate_provider.clone(),
        history_file: config.history_file.clone(),
    }
}

fn save_history_if_enabled(result: &TranslationResult, config: &Config) {
    if !config.save_translation_history {
        return;
    }
    let output = if result.is_dictionary {
        result
            .dictionary
            .as_ref()
            .map(|d| {
                let lines: Vec<String> = d
                    .entries
                    .iter()
                    .flat_map(|e| {
                        std::iter::once(e.part_of_speech.clone()).chain(
                            e.definitions
                                .iter()
                                .map(|def| format!("  {}", def.text)),
                        )
                    })
                    .collect();
                lines.join("\n")
            })
            .unwrap_or_else(|| result.translated.clone())
    } else {
        result.translated.clone()
    };

    tagent::config::save_translation_history(
        &result.original,
        &output,
        &result.from_lang,
        &result.to_lang,
        config,
    )
    .ok();
}
