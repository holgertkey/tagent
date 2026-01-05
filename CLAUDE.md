# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## User notes

- Ensure that all comments and another text are written in English.

- Avoid writing lines like this in the comments:
  "Generated with [Claude Code](https://claude.com/claude-code)
  Co-Authored-By: Claude <noreply@anthropic.com>"

- **Use `.debug/` folder for temporary files**: Write all temporary files, reports, test logs, debug output, and any other temporary artifacts to the `.debug/` folder. This keeps the repository root clean and organized. The `.debug/` folder is already added to `.gitignore`.

- The `.debug/.TESTS` folder contains temporary files and folders for testing the project.

- **IMPORTANT: When fixing bugs or errors in the code, ALWAYS write proper tests immediately to prevent regression.**


### Version Management

**Build Number Convention**: After each compilation with code changes, increment the build number in `Cargo.toml`:
- Format: `version = "MAJOR.MINOR.PATCH+BUILD"`
- Example: `1.0.0+000` → `1.0.0+001` → `1.0.0+002`
- The build number (`+NNN`) is a 3-digit zero-padded counter
- Reset build number to `+000` when MAJOR, MINOR, or PATCH version changes
- This helps track development iterations between releases

**When to increment**:
- ✅ After fixing bugs and recompiling
- ✅ After adding features and recompiling
- ✅ After refactoring and recompiling
- ❌ Do NOT increment for documentation-only changes
- ❌ Do NOT increment if code wasn't modified

**Automatic Version Synchronization**:
Version is defined **ONLY** in `Cargo.toml`. All other locations automatically sync from it:
- **Source code (.rs)**: Uses `env!("CARGO_PKG_VERSION")` macro at compile time
- **Windows resources (build.rs)**: Reads from `env!("CARGO_PKG_VERSION")` and converts to Windows format (x.x.x.x)
- **Documentation files (README.md, CLAUDE.md, CHANGELOG.md)**: Automatically updated by `build.rs` during compilation
  - `sync_version_in_docs()` function scans and updates version patterns
  - Updates only when version changes to avoid unnecessary writes
  - Runs on every `cargo build` or `cargo check`
  - CHANGELOG.md: Updates current version header `## [VERSION] - DATE`
  - README.md: Updates title, current version, and footer
  - CLAUDE.md: Updates project overview

To change version: edit only `Cargo.toml`, then rebuild. All files will automatically use the new version.

**Changelog Management**:
- **CHANGELOG.md** follows [Keep a Changelog](https://keepachangelog.com/) format
- When incrementing version in `Cargo.toml`:
  1. Version number auto-syncs to CHANGELOG.md header
  2. Manually add entry describing changes under appropriate sections:
     - **Added**: New features
     - **Changed**: Changes to existing functionality
     - **Deprecated**: Soon-to-be removed features
     - **Removed**: Removed features
     - **Fixed**: Bug fixes
     - **Security**: Security improvements
  3. Update the date in the version header if releasing
- Keep `## [Unreleased]` section at top for ongoing work
- README.md links to CHANGELOG.md for full version history

**Note**: Old version sections in CHANGELOG.md (e.g., `## [0.7.0]`) are intentionally NOT synchronized as they contain historical data.


## Project Overview

**Tagent** is a Windows text translation tool (v0.8.0+033) built in Rust that provides three translation modes:
1. **GUI Hotkeys**: System-wide Ctrl+Ctrl double-press to translate selected text
2. **Interactive Terminal**: Interactive prompt for typing translations
3. **CLI Mode**: One-off command-line translations

The application uses Google Translate API for translations and dictionary lookups, with optional history logging.

## Build Commands

```bash
# Build release version
cargo build --release

# Run in unified mode (GUI hotkeys + interactive terminal)
cargo run

# Run in CLI mode (single translation)
cargo run -- "text to translate"
cargo run -- --help
cargo run -- --config

# Run tests (if available)
cargo test

# Check for compilation errors
cargo check
```

The compiled executable is located at `target/release/tagent.exe`.

## Architecture

### Core Modules

The application is structured into 7 main modules in `src/`:

- **main.rs**: Entry point, orchestrates unified mode (GUI + Interactive) or CLI mode
- **translator.rs**: Translation engine, Google Translate API integration, dictionary lookups
- **config.rs**: Configuration management with live-reload (INI format, stored in AppData)
- **keyboard.rs**: Windows low-level keyboard hook for Ctrl+Ctrl detection
- **interactive.rs**: Interactive terminal mode with command handling
- **cli.rs**: Command-line argument processing and single-shot translations
- **clipboard.rs**: Windows clipboard operations
- **window.rs**: Windows window management (show/hide terminal)

### Key Architectural Patterns

**Dual-Mode Operation**: The application determines its mode from command-line arguments in main.rs:
- No arguments → Unified mode (keyboard hook + interactive prompt run concurrently)
- With arguments → CLI mode (one-time translation then exit)

**Async Runtime**: Uses Tokio for async operations. The keyboard hook spawns a separate Tokio runtime for translation tasks to avoid blocking the Windows message loop.

**Shared State Management**:
- `ConfigManager` uses `Arc<Mutex<Config>>` for thread-safe config access
- `should_exit` flag uses `Arc<AtomicBool>` shared between keyboard hook and interactive mode
- Keyboard hook uses `OnceLock` static variables for global state (translator, timing, processing flags)

**Configuration Hot-Reload**: `ConfigManager::check_and_reload()` compares file modification timestamps and reloads configuration without restart. Called before each translation operation.

**Windows API Integration**:
- Low-level keyboard hook (`SetWindowsHookExW` with `WH_KEYBOARD_LL`)
- Clipboard operations via clipboard-win crate
- Window management for showing/hiding terminal
- Message loop with `PeekMessageW` for non-blocking processing

### Google Translate API Integration

The translator module uses unofficial Google Translate API endpoints:
- Translation: `translate.googleapis.com/translate_a/single?client=gtx&sl=<from>&tl=<to>&dt=t&q=<text>`
- Dictionary: Same endpoint with additional `dt` parameters (bd, ex, ld, md, qca, rw, rm, ss)

Dictionary responses are parsed from JSON arrays and formatted with parts of speech in the target language.

### Double Ctrl Detection

The keyboard hook in `keyboard.rs` implements debounced double-press detection:
1. Track timestamp of last Ctrl press
2. On new Ctrl press, check if 50-500ms elapsed since last press
3. Ignore key repeat events (track Ctrl up/down state)
4. Spawn translation task on successful double-press
5. Prevent concurrent translations with `is_processing` mutex

### Alternative Hotkey Support

The application supports customizable hotkeys in addition to the default Ctrl+Ctrl double-press. This feature allows users to configure alternative key combinations through the configuration file.

**Configuration** (`[Hotkeys]` section in tagent.conf):
- `AlternativeHotkey`: Hotkey string specifying the key combination (default: "Alt+Q")
- `EnableAlternativeHotkey`: Boolean flag to enable/disable alternative hotkey (default: true)

**Supported Hotkey Formats**:
1. **Single keys**: `F1-F12`, `Space`, `Tab`, `Enter`, etc.
   - Example: `AlternativeHotkey = F9` (or single modifier combo like `Alt+Q`)
2. **Modifier combinations**: `Alt+Space`, `Ctrl+Shift+T`, `Win+T`
   - Example: `AlternativeHotkey = Alt+Space`
3. **Double-press patterns**: `F8+F8`, `Ctrl+Ctrl`
   - Example: `AlternativeHotkey = F8+F8`

**Implementation Architecture**:

*Hotkey Parsing* (`src/config.rs`):
- `HotkeyType` enum: Represents three types of hotkeys (SingleKey, ModifierCombo, DoublePress)
- `HotkeyParser::parse()`: Converts configuration strings to HotkeyType enum
- `HotkeyParser::key_name_to_vk()`: Maps key names (e.g., "F9", "Alt", "Space") to Windows VK codes
- `HotkeyParser::validate_hotkey()`: Validates hotkeys against dangerous system shortcuts (Ctrl+Alt+Delete, Win+L)

*Detection Logic* (`src/keyboard.rs`):
- **Single key**: Direct vk_code comparison on WM_KEYDOWN event
- **Modifier combo**:
  - Track modifier key states in `MODIFIER_STATE` HashMap
  - On target key press, verify all required modifiers are currently pressed
  - Clear state on WM_KEYUP to handle key releases
- **Double-press**:
  - Track timestamp of key presses in `LAST_KEY_TIME`
  - Trigger translation if second press occurs within configured time window (50-500ms)
  - Similar to Ctrl+Ctrl logic but configurable for any key

*Static Variables*:
- `ALT_HOTKEY_CONFIG`: Stores parsed HotkeyType configuration
- `ALT_HOTKEY_ENABLED`: Atomic flag for quick enabled/disabled check
- `MODIFIER_STATE`: HashMap tracking modifier key states (for combo detection)
- `LAST_KEY_TIME`: Timestamp for double-press detection

**Backward Compatibility**:
- Ctrl+Ctrl double-press **always works** regardless of alternative hotkey settings
- Missing `[Hotkeys]` section in config uses defaults (Alt+Q enabled)
- Invalid hotkey strings disable alternative hotkey with warning message, Ctrl+Ctrl continues to work
- No breaking changes to existing functionality

**Initialization Flow**:
1. `KeyboardHook::new()` loads configuration via `ConfigManager`
2. Parse `alternative_hotkey` string using `HotkeyParser::parse()`
3. Validate parsed hotkey with `HotkeyParser::validate_hotkey()`
4. Initialize static variables with parsed configuration
5. On parse/validation errors: log warning, disable alternative hotkey, continue with Ctrl+Ctrl only

**Detection Flow** (in `keyboard_hook_proc()`):
1. WM_KEYDOWN event received
2. First, attempt Ctrl+Ctrl detection (existing logic)
3. If not Ctrl+Ctrl, attempt alternative hotkey detection via `handle_alternative_hotkey()`
4. WM_KEYUP event received
5. Update Ctrl state for Ctrl+Ctrl logic
6. Update modifier states for alternative hotkey combos

**System Shortcut Protection**:
- Blocks configuration of dangerous combinations: Ctrl+Alt+Delete, Win+L
- Warns about potentially disruptive shortcuts: Alt+F4
- Returns validation errors before initialization

**Limitations**:
- Hotkey changes require application restart to take effect
- Some system-reserved shortcuts may be intercepted by Windows before reaching the application
- No runtime hot-reload of hotkey configuration (restart required)

### History Logging

When enabled in config (`SaveTranslationHistory = true`), all translations are appended to a file with format:
```
[YYYY-MM-DD HH:MM:SS UTC] <source_lang> -> <target_lang>
IN:  <original text>
OUT: <translation or dictionary entry>
---
```

This is implemented identically in translator.rs, interactive.rs, and cli.rs.

## Configuration System

Configuration file is stored in `%APPDATA%\Tagent\tagent.conf` (typically `C:\Users\<Username>\AppData\Roaming\Tagent\tagent.conf`) and uses INI format with sections:
- `[Translation]`: SourceLanguage, TargetLanguage, CopyToClipboard
- `[Dictionary]`: ShowDictionary
- `[Interface]`: ShowTerminalOnTranslate, AutoHideTerminalSeconds
- `[History]`: SaveTranslationHistory, HistoryFile
- `[Hotkeys]`: AlternativeHotkey, EnableAlternativeHotkey

Language names (e.g., "Russian", "English") are mapped to Google Translate codes (ru, en) in `ConfigManager::language_to_code()`.

### Configuration File Location

The configuration and history files are stored in the Windows AppData folder:
- **Config path**: `%APPDATA%\Tagent\tagent.conf`
- **History path**: `%APPDATA%\Tagent\translation_history.txt` (by default)

The `ConfigManager::get_default_config_path()` function uses the `dirs` crate to obtain the system's config directory and automatically creates the `Tagent` subfolder if it doesn't exist. This approach:
- Follows Windows application standards
- Keeps user data separate from the program installation
- Survives program reinstallation
- Allows per-user configuration in multi-user environments

## Development Notes

### Testing Keyboard Hook
The keyboard hook only works when compiled as a Windows executable. It will not function correctly in WSL or non-Windows environments.

### Building with Windows Resources
The `build.rs` script uses winres to embed application metadata and icons. Icon file must exist at `assets/icons/taa_256.ico`.

### Exit Handling
In unified mode:
- Interactive mode sets `should_exit` flag when user types /exit, /quit, or /q
- Keyboard hook checks flag in message loop and breaks
- Main waits for keyboard task to complete before exiting

### Thread Safety Considerations
- Keyboard hook runs in Windows message loop thread
- Translation tasks spawn new threads with their own Tokio runtime
- Config reload happens in calling thread (main or keyboard hook thread)
- Clipboard operations are thread-safe via clipboard-win

## Common Development Tasks

### Adding a New Language
1. Add language name → code mapping in `ConfigManager::language_to_code()`
2. (Optional) Add part-of-speech translations in `Translator::get_full_part_of_speech()`

### Modifying Translation Output Format
- GUI mode: Edit `Translator::format_dictionary_response()` and `Translator::perform_translation()`
- CLI mode: Edit `Translator::format_dictionary_response_cli()` and `CliHandler::perform_translation()`
- Interactive mode: Uses CLI format methods via public methods

### Changing Hotkey Combination
Modify `keyboard_hook_proc()` in keyboard.rs:
- Change virtual key codes (currently `VK_LCONTROL`/`VK_RCONTROL`)
- Adjust timing thresholds (`Duration::from_millis(50)` to `500`)

### Adding New Interactive Commands
In `InteractiveMode::handle_command()`, add new command patterns to the match statement and implement handler methods.
