use std::fs;
use std::path::Path;

fn main() {
    // Get version from Cargo.toml (format: "MAJOR.MINOR.PATCH+BUILD" or "MAJOR.MINOR.PATCH")
    let version = env!("CARGO_PKG_VERSION");

    // Sync version in documentation files
    sync_version_in_docs(version);

    // Only embed Windows resources when building the tagent binary (not when used as a library).
    // Controlled by the "binary-resources" feature flag so that tagent-gui does not get a
    // duplicate VERSION resource linker error.
    #[cfg(target_os = "windows")]
    if std::env::var("CARGO_FEATURE_BINARY_RESOURCES").is_ok() {
        build_windows_resources(version);
    }
}

/// Build Windows executable resources (icon, version info)
#[cfg(target_os = "windows")]
fn build_windows_resources(version: &str) {
    use winres::WindowsResource;

    // Convert version to Windows format (x.x.x.x)
    // Example: "0.8.0+001" -> "0.8.0.1"
    let windows_version = convert_to_windows_version(version);

    WindowsResource::new()
        // Set the main icon (shows in file explorer, taskbar, etc.)
        .set_icon("assets/icons/taa_256.ico")
        // Set application information
        .set("ProductName", "Text Agent Translator")
        .set(
            "FileDescription",
            "Text translator with unified GUI/Interactive interface and CLI mode",
        )
        .set("CompanyName", "Holgert K")
        .set(
            "LegalCopyright",
            "© 2024 Holgert K. Licensed under MIT License",
        )
        .set("FileVersion", &windows_version)
        .set("ProductVersion", &windows_version)
        .set("OriginalFilename", "tagent-cli.exe")
        .set("InternalName", "tagent-cli")
        // Compile the resource
        .compile()
        .expect("Failed to compile Windows resources");

    // No manual link step needed here: `.compile()` already emits
    // `cargo:rustc-link-lib=static=resource` + `cargo:rustc-link-search=native=...`.
    // Cargo scopes those to the package's [lib] target only when one exists; this
    // crate has none (the translation logic lives in the separate `tagent` library
    // crate), so Cargo falls back to applying them directly to the [[bin]] target.
    // Adding an explicit `cargo:rustc-link-arg-bins=.../resource.lib` on top of that
    // linked resource.lib twice and caused CVTRES error CVT1100 (duplicate VERSION
    // resource) at link time.
}

/// Synchronize version in documentation files (README.md, CLAUDE.md, CHANGELOG.md)
/// This ensures version is defined only in Cargo.toml and auto-syncs everywhere
fn sync_version_in_docs(version: &str) {
    // Files to update with version patterns
    let files = vec![
        (
            "README.md",
            vec![
                ("# Tagent Text Translator v", "\n"),
                ("**Current Version**: v", "\n"),
                ("**Tagent Text Translator v", "** - Fast, reliable"),
            ],
        ),
        ("../CLAUDE.md", vec![("(v", ") built in Rust")]),
        (
            "../CHANGELOG.md",
            vec![
                ("## [", "] - "), // Changelog section header: ## [VERSION] - DATE
            ],
        ),
    ];

    for (file_path, patterns) in files {
        if let Err(e) = update_version_in_file(file_path, version, &patterns) {
            println!(
                "cargo:warning=Failed to sync version in {}: {}",
                file_path, e
            );
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
        let mut search_from = 0usize;

        while let Some(rel_start_pos) = updated_content[search_from..].find(pattern_start) {
            let start_pos = search_from + rel_start_pos;
            let search_start = start_pos + pattern_start.len();

            // Skip a CHANGELOG "## [Unreleased]" header: it has no version to sync, and
            // naively searching for the next "] - " from here would run past it into the
            // following real version header, swallowing everything in between (including
            // the Unreleased section's own entries) when the span gets replaced.
            if updated_content[search_start..].starts_with("Unreleased]") {
                search_from = search_start;
                continue;
            }

            let Some(end_pos) = updated_content[search_start..].find(pattern_end) else {
                break;
            };
            let full_end_pos = search_start + end_pos;
            let old_version = &updated_content[search_start..full_end_pos];

            // Only update if version actually changed
            if old_version != new_version {
                updated_content.replace_range(search_start..full_end_pos, new_version);
                changed = true;
            }

            // This is the current (topmost, non-Unreleased) match for this pattern —
            // stop here regardless of whether it needed updating. Every other
            // occurrence further down the file (e.g. older CHANGELOG headers) is
            // historical and must never be touched by a version sync. Previously this
            // only broke on the "already current" branch and kept scanning after an
            // actual replacement, which cascaded through the entire file rewriting
            // every historical version header to the new version.
            break;
        }
    }

    // Only write if content changed
    if changed {
        fs::write(file_path, updated_content)?;
        println!(
            "cargo:warning=Updated version to {} in {}",
            new_version, file_path
        );
    }

    Ok(())
}

/// Convert Cargo version format to Windows version format
/// Examples:
///   "0.8.0+001" -> "0.8.0.1"
///   "0.8.0" -> "0.8.0.0"
///   "1.2.3+123" -> "1.2.3.123"
#[cfg(target_os = "windows")]
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

// NOTE: build.rs is compiled as a build-dependency binary, not as part of any
// `[lib]`/`[[bin]]` target, so `cargo test` never runs these — verify manually with
// `rustc --edition 2021 --test build.rs -o /tmp/build_rs_tests && /tmp/build_rs_tests`.
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tagent_build_rs_test_{}_{}",
            std::process::id(),
            name
        ));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn windows_resource_link_is_not_manually_duplicated() {
        // Regression test: winres's `.compile()` already emits
        // `cargo:rustc-link-lib=static=resource` + `cargo:rustc-link-search=native=...`.
        // Since this crate has no [lib] target, Cargo applies those directly to the
        // [[bin]] target automatically (see the Cargo book: link directives are scoped
        // to the [lib] target only when one exists). A manual
        // `cargo:rustc-link-arg-bins=.../resource.lib` (or the older, pre-library-split
        // `cargo:rustc-link-arg=.../resource.lib`) on top of that linked resource.lib
        // twice and broke the Windows link step with
        // `CVTRES : fatal error CVT1100: duplicate resource. type:VERSION`.
        // Built from pieces (rather than one literal) so this check doesn't trip on its
        // own doc comments above, which quote the forbidden directive as prose; the
        // split point is chosen so it matches both the `-bins` and non-`-bins` forms.
        let forbidden_call = ["println!(\"cargo:rustc-link-", "arg"].concat();
        let source = include_str!("build.rs");
        assert!(
            !source.contains(&forbidden_call),
            "a manual rustc-link-arg(-bins) directive for resource.lib duplicates \
             winres's own automatic link directives now that this crate has no [lib] \
             target, and breaks the Windows link step (CVT1100)"
        );
    }

    #[test]
    fn changelog_unreleased_header_is_not_swallowed_by_version_sync() {
        // Regression test: `find("## [")` used to match "## [Unreleased]" first, then
        // scan forward for "] - ", which only appears at the *next* real version header —
        // replacing everything in between (including the Unreleased section's own
        // entries) with just the new version string.
        let path = write_temp_file(
            "changelog",
            "# Changelog\n\n\
             ## [Unreleased]\n\n\
             ### Changed\n\
             - Some entry that must survive a version sync.\n\n\
             ## [0.12.0] - 2026-01-01\n\n\
             ### Fixed\n\
             - Old entry.\n",
        );

        update_version_in_file(path.to_str().unwrap(), "0.13.0", &[("## [", "] - ")]).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();

        assert!(
            updated.contains("## [Unreleased]"),
            "Unreleased header must survive version sync, got:\n{}",
            updated
        );
        assert!(
            updated.contains("Some entry that must survive a version sync."),
            "Unreleased entry must survive version sync, got:\n{}",
            updated
        );
        assert!(
            updated.contains("## [0.13.0] - 2026-01-01"),
            "the real version header must still get updated, got:\n{}",
            updated
        );
    }

    #[test]
    fn changelog_historical_headers_below_the_current_one_are_not_touched() {
        // Regression test for the cascade-corruption bug: after updating the topmost
        // (non-Unreleased) header, the loop used to keep scanning and rewrite every
        // subsequent "## [OLD_VERSION] - DATE" header to the new version too, since
        // each one's old version also didn't match new_version. Reproduced live
        // multiple times against the real CHANGELOG.md before this fix.
        let path = write_temp_file(
            "cascade",
            "# Changelog\n\n\
             ## [Unreleased]\n\n\
             ## [0.14.0] - 2026-08-07\n\n\
             ### Changed\n\
             - Current release notes.\n\n\
             ## [0.13.0+003] - 2026-08-05\n\n\
             ### Fixed\n\
             - Old fix from a previous release.\n\n\
             ## [0.13.0+001] - 2026-01-06\n\n\
             ### Added\n\
             - Even older entry.\n",
        );

        update_version_in_file(path.to_str().unwrap(), "0.15.0", &[("## [", "] - ")]).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();

        assert!(
            updated.contains("## [0.15.0] - 2026-08-07"),
            "topmost real header must be updated to the new version, got:\n{}",
            updated
        );
        assert!(
            updated.contains("## [0.13.0+003] - 2026-08-05"),
            "older historical header must survive untouched, got:\n{}",
            updated
        );
        assert!(
            updated.contains("## [0.13.0+001] - 2026-01-06"),
            "oldest historical header must survive untouched, got:\n{}",
            updated
        );
    }

    #[test]
    fn changelog_historical_headers_survive_when_topmost_already_current() {
        // The round-trip variant of the cascade bug: even when the topmost header
        // already matches new_version (so the "already current" branch breaks
        // immediately), a stale build could still have left older headers rewritten
        // from a *previous* buggy run. This just confirms the no-op path never
        // touches anything below the topmost match.
        let path = write_temp_file(
            "cascade_nochange",
            "## [Unreleased]\n\n\
             ## [0.15.0] - 2026-08-07\n\n\
             ## [0.13.0+003] - 2026-08-05\n\n\
             ## [0.13.0+001] - 2026-01-06\n",
        );

        update_version_in_file(path.to_str().unwrap(), "0.15.0", &[("## [", "] - ")]).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();

        assert!(updated.contains("## [0.13.0+003] - 2026-08-05"));
        assert!(updated.contains("## [0.13.0+001] - 2026-01-06"));
    }

    #[test]
    fn version_header_updates_when_changed() {
        let path = write_temp_file("simple", "## [0.12.0] - 2026-01-01\n");
        update_version_in_file(path.to_str().unwrap(), "0.13.0", &[("## [", "] - ")]).unwrap();
        let updated = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(updated, "## [0.13.0] - 2026-01-01\n");
    }

    #[test]
    fn version_header_left_unchanged_when_already_current() {
        let content = "## [0.13.0] - 2026-01-01\n";
        let path = write_temp_file("nochange", content);
        update_version_in_file(path.to_str().unwrap(), "0.13.0", &[("## [", "] - ")]).unwrap();
        let updated = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(updated, content);
    }
}
