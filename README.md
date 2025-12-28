# Tagent Text Translator v0.8.0+032

A fast, lightweight text translation tool for Windows with unified GUI hotkeys, interactive terminal, and CLI interfaces. Translate selected text from any application with a simple double-Ctrl press or use the command line for quick translations.

## Features

### 🔥 **Unified Translation Modes**
- **GUI Hotkeys**: Select text anywhere, press Ctrl+Ctrl, get instant translation
- **Interactive Terminal**: Type text directly in the terminal prompt
- **CLI Mode**: One-time translations from command line

### 📚 **Smart Dictionary Lookup**
- Detailed word definitions with part of speech
- Synonyms and multiple meanings
- Automatic fallback to translation for phrases
- Supports multiple target languages

### 📝 **Translation History**
- Optional logging of all translations with timestamps
- Multi-line format for better readability
- Configurable file path
- Works across all translation modes

### ⚡ **Performance & Usability**
- Instant translations using Google Translate API
- Real-time configuration reloading (no restart required)
- Automatic clipboard copying (configurable)
- Smart terminal window management
- Multi-language support

## Installation

### Prerequisites
- Windows 10/11
- Internet connection for translations

## Download

**Latest Release**: [Download tagent.exe](https://github.com/holgertkey/tagent-win/releases/latest)

All releases: https://github.com/holgertkey/tagent-win/releases

### Download & Setup
1. Download the latest release
2. Extract to your preferred directory
3. Run `tagent.exe` to start unified mode
4. Configuration file will be created automatically in `%APPDATA%\Tagent\tagent.conf`

## Quick Start

### Unified Mode (Recommended)
```bash
# Start unified mode (no arguments)
tagent.exe
```
This starts both:
- **Interactive prompt** in the terminal
- **GUI hotkeys** (Ctrl+Ctrl) for system-wide translation

### CLI Mode
```bash
# Translate a single word
tagent hello

# Translate a phrase
tagent "Hello world"

# Show help
tagent --help

# Show current configuration
tagent --config
```

## Usage Guide

### GUI Hotkeys (System-wide)
1. Select any text in any Windows application
2. Quickly double-press Ctrl key (Ctrl + Ctrl)
3. Translation appears in terminal and copies to clipboard
4. Paste anywhere with Ctrl+V

### Interactive Terminal
```
[Auto]: hello
привет

[Word]: translate
Глагол
  переводить [перевести, толковать, интерпретировать]
  транслировать [передавать, транслить]
  перемещать [переносить, передвигать]

[Auto]: exit
Goodbye!
```

### Interactive Commands
- `-?`, `-h`, `--help` - Show help
- `-c`, `--config` - Show current configuration  
- `-v`, `--version` - Show version information
- `--clear`, `--cls` - Clear screen
- `--exit`, `--quit`, `-q` - Exit program

## Configuration

Configuration is stored in `%APPDATA%\Tagent\tagent.conf` (typically `C:\Users\<YourName>\AppData\Roaming\Tagent\tagent.conf`) and reloads automatically:

```ini
[Translation]
; Source language (Auto, English, Russian, Spanish, etc.)
SourceLanguage = Auto

; Target language  
TargetLanguage = Russian

; Copy results to clipboard automatically
CopyToClipboard = true

[Dictionary]
; Show detailed word information for single words
ShowDictionary = true

[Interface]
; Show terminal window during GUI translation
ShowTerminalOnTranslate = true

; Auto-hide terminal after translation (seconds, 0 = disabled)
AutoHideTerminalSeconds = 5

[History]
; Save all translations to file with timestamps
SaveTranslationHistory = false

; History file path (defaults to AppData\Tagent folder)
HistoryFile = C:\Users\<YourName>\AppData\Roaming\Tagent\translation_history.txt
```

### Supported Languages
- **Auto-detection**: Auto
- **Major Languages**: English, Russian, Spanish, French, German, Chinese, Japanese, Korean, Italian, Portuguese, Dutch, Polish, Turkish, Arabic, Hindi
- **Language Codes**: en, ru, es, fr, de, zh, ja, ko, it, pt, nl, pl, tr, ar, hi

## Translation History

When enabled (`SaveTranslationHistory = true`), all translations are logged in a readable format:

```
[2025-09-06 14:30:15 UTC] en -> ru
IN:  hello
OUT: привет
---

[2025-09-06 14:32:45 UTC] en -> ru
IN:  cat
OUT: [Word]: cat
Существительное
  кот [кошка, котенок]
  кошка [котенок, котик]
---
```

## Examples

### Basic Translation
```bash
# CLI
tagent "How are you?"
# Output: Как дела?

# Interactive
[Auto]: How are you?
Как дела?
```

### Dictionary Lookup
```bash
# CLI
tagent beautiful
# Output:
# Прилагательное
#   красивый [прекрасный, красивая]
#   прекрасный [великолепный, чудесный]

# Interactive  
[Auto]: beautiful
Прилагательное
  красивый [прекрасный, красивая]
  прекрасный [великолепный, чудесный]
```

### Configuration Management
```bash
# Show current settings
tagent --config

# Output:
# === Current Configuration ===
# Source Language: Auto (auto)
# Target Language: Russian (ru)
# Show Dictionary: Enabled
# Copy to Clipboard: Enabled
# Save Translation History: Disabled
# History File: translation_history.txt
```

## Advanced Usage

### Custom Language Pairs
Edit `%APPDATA%\Tagent\tagent.conf`:
```ini
[Translation]
SourceLanguage = English
TargetLanguage = Spanish
```

### Enable History Logging
```ini
[History]
SaveTranslationHistory = true
HistoryFile = my_translations.txt
```

### Disable Automatic Features
```ini
[Translation]
CopyToClipboard = false

[Dictionary]
ShowDictionary = false

[Interface]
ShowTerminalOnTranslate = false
```

## Troubleshooting

### Common Issues

**"No selected text or clipboard is empty"**
- Ensure text is properly selected before pressing Ctrl+Ctrl
- Try selecting text again
- Check if another application is interfering with clipboard

**"Translation failed: HTTP error"**
- Check internet connection
- Verify firewall settings allow the application
- Google Translate service may be temporarily unavailable

**"Config reload error"**
- Check config file syntax at `%APPDATA%\Tagent\tagent.conf`
- Ensure file is not locked by another application
- Delete config file from AppData folder to regenerate default settings

**Hotkeys not working**
- Run as administrator if needed
- Check if another application is capturing Ctrl key
- Ensure application has keyboard input permissions

### Performance Tips

- Use `ShowTerminalOnTranslate = false` for faster GUI translations
- Set `AutoHideTerminalSeconds = 0` to keep terminal visible
- Disable history logging for maximum speed
- Use specific source language instead of "Auto" for faster processing

## Technical Details

### Dependencies
- **Tokio**: Async runtime
- **Reqwest**: HTTP client for Google Translate API
- **Chrono**: Timestamp handling for history
- **Windows API**: Clipboard and keyboard hook functionality

### System Requirements
- Windows 10 or later
- ~5MB disk space
- Network access for translations
- No additional runtime dependencies

### Architecture
- **Rust**: Safe, fast systems programming
- **Async/await**: Non-blocking translation requests
- **Windows hooks**: Low-level keyboard capture
- **Real-time config**: File watching for instant updates

## Building from Source

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows-specific tools
rustup target add x86_64-pc-windows-msvc
```

### Build
```bash
git clone https://github.com/holgertkey/tagent-win
cd tagent-win
cargo build --release
```

### Dependencies
The project uses these Rust crates:
```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clipboard-win = "5.0"
chrono = { version = "0.4", features = ["serde"] }
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
    "Win32_System_Console"
] }
url = "2.4"
dirs = "5.0"
```

## Version History

See [CHANGELOG.md](CHANGELOG.md) for detailed version history and release notes.

**Current Version**: v0.8.0+032

**Recent Changes**:
- Automatic version synchronization system
- Configuration moved to AppData folder
- Translation history logging with timestamps
- Unified GUI + Interactive interface

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For issues, feature requests, or questions:
- Create an issue in the repository
- Check existing issues for solutions
- Review this README for common problems

---

**Tagent Text Translator v0.8.0+032** - Fast, reliable, and feature-rich translation tool for Windows.