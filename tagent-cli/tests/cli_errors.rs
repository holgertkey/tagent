//! How CLI mode reports errors, run against the real binary.

/// Linux only: `XDG_CONFIG_HOME` points the binary at a throwaway config there (on
/// Windows and macOS `dirs` doesn't read it, and the user's own config would be used).
#[cfg(target_os = "linux")]
#[test]
fn a_failed_translation_is_reported_once_and_without_the_debug_form() {
    use std::process::Command;

    let home = tempfile::tempdir().unwrap();
    let config_dir = home.path().join("config/tagent-cli");
    std::fs::create_dir_all(&config_dir).unwrap();
    // Port 9 (discard) on localhost: nothing listens, so the request fails at once.
    std::fs::write(
        config_dir.join("tagent-cli.toml"),
        r#"[provider]
translate_provider = "unreachable"

[dictionary]
show_dictionary = false

[provider_options.unreachable]
type = "openai"
endpoint = "http://127.0.0.1:9/v1"
model = "test"
max_retries = "0"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tagent-cli"))
        .arg("hello world")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_DATA_HOME", home.path().join("data"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let lines: Vec<&str> = stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 1, "stderr: {stderr}");
    assert!(
        lines[0].starts_with("Translation failed: network error: "),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("Network("), "stderr: {stderr}");
}
