use windows::Win32::System::Console::SetConsoleCtrlHandler;

/// Set up Windows-specific signal handling (disable default Ctrl+C handler)
pub fn setup() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    unsafe {
        SetConsoleCtrlHandler(None, true)?;
    }
    Ok(())
}

/// Always `false`: Ctrl+C is disabled in the console here (see [`setup`]); Esc stops
/// speech on Windows. Same signature as Linux's, which speech polls.
pub fn take_interrupted() -> bool {
    false
}
