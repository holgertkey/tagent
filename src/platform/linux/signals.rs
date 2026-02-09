/// Set up Linux-specific signal handling
pub fn setup() -> Result<(), Box<dyn std::error::Error>> {
    // On Linux, Ctrl+C is handled by the default terminal signal handler.
    // Will be enhanced with ctrlc crate in Phase 3.
    Ok(())
}
