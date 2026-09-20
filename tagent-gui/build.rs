fn main() {
    slint_build::compile("ui/app.slint").unwrap();

    #[cfg(target_os = "windows")]
    build_windows_resources();
}

/// Embed the executable icon and version info (Explorer, shortcuts, pinned taskbar
/// button). The window and tray icons are separate: they come from `tray.png` via
/// `app.slint`.
#[cfg(target_os = "windows")]
fn build_windows_resources() {
    // `winresource` fills FileVersion/ProductVersion from CARGO_PKG_VERSION on its own,
    // so only the descriptive fields are set here.
    // `.compile()` already emits the link directives; adding a manual link step on top
    // links resource.lib twice (see `tagent-cli/build.rs`).
    winresource::WindowsResource::new()
        .set_icon("assets/icons/tagent-gui.ico")
        .set("ProductName", "Tagent")
        .set(
            "FileDescription",
            "Tagent GUI - desktop text translator with dictionary and text-to-speech",
        )
        .set("CompanyName", "Holgert K")
        .set(
            "LegalCopyright",
            "© 2024 Holgert K. Licensed under MIT License",
        )
        .set("OriginalFilename", "tagent-gui.exe")
        .set("InternalName", "tagent-gui")
        .compile()
        .expect("Failed to compile Windows resources");
}
