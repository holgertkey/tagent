use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;
use windows::{
    Win32::UI::Input::KeyboardAndMouse::*,
};

#[derive(Clone)]
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

    /// Автоматически копирует выделенный текст (симуляция Ctrl+C)
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            // Симулируем нажатие Ctrl+C
            // Нажимаем Ctrl (значение 0 означает нажатие)
            keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            
            // Нажимаем C
            keybd_event(b'C', 0, KEYBD_EVENT_FLAGS(0), 0);
            
            // Отпускаем C (KEYEVENTF_KEYUP = 2)
            keybd_event(b'C', 0, KEYEVENTF_KEYUP, 0);
            
            // Отпускаем Ctrl
            keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            
            // Небольшая задержка для завершения операции копирования
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        
        Ok(())
    }

    /// Получить текст из буфера обмена с автоматическим копированием
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error>> {
        // Сначала копируем выделенный текст
        self.copy_selected_text()?;
        
        // Затем читаем из буфера обмена
        self.get_text()
    }
}