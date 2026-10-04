/// Set up macOS-specific signal handling
pub fn setup() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(())
}

/// Always `false`: no Ctrl+C handling on macOS yet. Same signature as Linux's, which
/// speech polls.
pub fn take_interrupted() -> bool {
    false
}
