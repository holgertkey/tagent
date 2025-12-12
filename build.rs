use winres::WindowsResource;

fn main() {
    // Only build resources on Windows
    if cfg!(target_os = "windows") {
        // Get version from Cargo.toml (format: "MAJOR.MINOR.PATCH+BUILD" or "MAJOR.MINOR.PATCH")
        let version = env!("CARGO_PKG_VERSION");

        // Convert version to Windows format (x.x.x.x)
        // Example: "0.8.0+001" -> "0.8.0.1"
        let windows_version = convert_to_windows_version(version);

        WindowsResource::new()
            // Set the main icon (shows in file explorer, taskbar, etc.)
            .set_icon("assets/icons/taa_256.ico")
            // Set application information
            .set("ProductName", "Text Agent Translator")
            .set("FileDescription", "Text translator with unified GUI/Interactive interface and CLI mode")
            .set("CompanyName", "Holgert K")
            .set("LegalCopyright", "© 2024 Holgert K. Licensed under MIT License")
            .set("FileVersion", &windows_version)
            .set("ProductVersion", &windows_version)
            .set("OriginalFilename", "tagent.exe")
            .set("InternalName", "tagent")
            // Compile the resource
            .compile()
            .expect("Failed to compile Windows resources");
    }
}

/// Convert Cargo version format to Windows version format
/// Examples:
///   "0.8.0+001" -> "0.8.0.1"
///   "0.8.0" -> "0.8.0.0"
///   "1.2.3+123" -> "1.2.3.123"
fn convert_to_windows_version(version: &str) -> String {
    // Split by '+' to separate version from build metadata
    let parts: Vec<&str> = version.split('+').collect();
    let base_version = parts[0];

    // Get build number or default to 0
    let build = if parts.len() > 1 {
        // Remove leading zeros from build number (001 -> 1)
        parts[1].trim_start_matches('0').to_string()
    } else {
        "0".to_string()
    };

    // Handle empty string after trimming zeros
    let build = if build.is_empty() { "0" } else { &build };

    format!("{}.{}", base_version, build)
}