use std::error::Error;

#[derive(Clone, Copy, Debug)]
pub struct WindowHandle(pub u64);

pub struct WindowManager;

impl WindowManager {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self)
    }

    pub fn show_terminal(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    pub fn hide_terminal(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    pub fn get_foreground_window(&self) -> Option<WindowHandle> {
        None
    }

    pub fn set_foreground_window(&self, _handle: WindowHandle) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    pub fn is_mouse_over_terminal(&self) -> bool {
        false
    }
}
