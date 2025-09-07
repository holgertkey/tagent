use winres::WindowsResource;

fn main() {
    // Only build resources on Windows
    if cfg!(target_os = "windows") {
        WindowsResource::new()
            // Set the main icon (shows in file explorer, taskbar, etc.)
            .set_icon("assets/icons/taa_256.ico")
            // Set application information
            .set("ProductName", "Text Agent Translator")
            .set("FileDescription", "Text translator with unified GUI/Interactive interface and CLI mode")
            .set("CompanyName", "Holgert K")
            .set("LegalCopyright", "© 2024 Holgert K. Licensed under MIT License")
            .set("FileVersion", "0.8.0.0")
            .set("ProductVersion", "0.8.0.0")
            .set("OriginalFilename", "tagent.exe")
            .set("InternalName", "tagent")
            // Compile the resource
            .compile()
            .expect("Failed to compile Windows resources");
    }
}