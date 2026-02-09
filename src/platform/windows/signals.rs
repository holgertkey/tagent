use windows::Win32::System::Console::SetConsoleCtrlHandler;

/// Set up Windows-specific signal handling (disable default Ctrl+C handler)
pub fn setup() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        SetConsoleCtrlHandler(None, true)?;
    }
    Ok(())
}
