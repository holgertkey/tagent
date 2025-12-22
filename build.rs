use winres::WindowsResource;
use std::fs;
use std::path::Path;

fn main() {
    // Get version from Cargo.toml (format: "MAJOR.MINOR.PATCH+BUILD" or "MAJOR.MINOR.PATCH")
    let version = env!("CARGO_PKG_VERSION");

    // Sync version in documentation files
    sync_version_in_docs(version);

    // Only build resources on Windows
    if cfg!(target_os = "windows") {
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

/// Synchronize version in documentation files (README.md, CLAUDE.md, CHANGELOG.md)
/// This ensures version is defined only in Cargo.toml and auto-syncs everywhere
fn sync_version_in_docs(version: &str) {
    // Files to update with version patterns
    let files = vec![
        ("README.md", vec![
            ("# Tagent Text Translator v", "\n"),
            ("**Current Version**: v", "\n"),
            ("**Tagent Text Translator v", "** - Fast, reliable"),
        ]),
        ("CLAUDE.md", vec![
            ("(v", ") built in Rust"),
        ]),
        ("CHANGELOG.md", vec![
            ("## [", "] - "),  // Changelog section header: ## [VERSION] - DATE
        ]),
    ];

    for (file_path, patterns) in files {
        if let Err(e) = update_version_in_file(file_path, version, &patterns) {
            println!("cargo:warning=Failed to sync version in {}: {}", file_path, e);
        }
    }
}

/// Update version in a specific file using pattern matching
fn update_version_in_file(
    file_path: &str,
    new_version: &str,
    patterns: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(file_path).exists() {
        return Ok(()); // Skip if file doesn't exist
    }

    let content = fs::read_to_string(file_path)?;
    let mut updated_content = content.clone();
    let mut changed = false;

    for (prefix, suffix) in patterns {
        // Find all occurrences of the pattern
        let pattern_start = *prefix;
        let pattern_end = *suffix;

        while let Some(start_pos) = updated_content.find(pattern_start) {
            let search_start = start_pos + pattern_start.len();

            if let Some(end_pos) = updated_content[search_start..].find(pattern_end) {
                let full_end_pos = search_start + end_pos;
                let old_version = &updated_content[search_start..full_end_pos];

                // Only update if version actually changed
                if old_version != new_version {
                    updated_content.replace_range(search_start..full_end_pos, new_version);
                    changed = true;
                } else {
                    // Break to avoid infinite loop when version matches
                    break;
                }
            } else {
                break;
            }
        }
    }

    // Only write if content changed
    if changed {
        fs::write(file_path, updated_content)?;
        println!("cargo:warning=Updated version to {} in {}", new_version, file_path);
    }

    Ok(())
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