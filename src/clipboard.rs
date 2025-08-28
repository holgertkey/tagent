use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;

pub struct ClipboardManager;

impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// Получить текст из буфера обмена
    pub fn get_text(&self) -> Result<String, Box<dyn Error>> {
        match get_clipboard(formats::Unicode) {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Ошибка чтения буфера обмена: {}", e).into()),
        }
    }

    /// Установить текст в буфер обмена
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error>> {
        match set_clipboard(formats::Unicode, text) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Ошибка записи в буфер обмена: {}", e).into()),
        }
    }
}