# Tagent Text Translator v0.14.0

A fast, lightweight text translation tool with unified GUI hotkeys, interactive terminal, and CLI interfaces. Translate selected text from any application with a simple Alt+Q hotkey or use the command line for quick translations. Supports Windows and Linux (X11, with partial Wayland support).

## Features

### 🔥 **Unified Translation Modes**
- **GUI Hotkeys**: Select text anywhere, press Alt+Q, get instant translation
- **Interactive Terminal**: Type text directly in the terminal prompt
- **CLI Mode**: One-time translations from command line

### 📚 **Smart Dictionary Lookup**
- Detailed word definitions with part of speech
- Synonyms and multiple meanings
- Automatic fallback to translation for phrases
- Supports multiple target languages

### ✏️ **Spell Checking**
- Automatically detects and corrects misspelled words during dictionary lookup
- Shows a correction notice in the target language 
- Works transparently: typos like "vialent" or "violnt" resolve to the correct word "violent"
- Can be disabled via `SpellCheck = false` in config

### 🔊 **Text-to-Speech (TTS)**
- Built-in speech synthesis using Google TTS API
- Available in all modes (GUI, Interactive, CLI)
- Speech hotkey for selected text (default: Alt+E)
- Press Esc to cancel speech playback
- Automatic language detection for speech
- Supports long text with automatic chunking

### 📝 **Translation History**
- Optional logging of all translations with timestamps
- Multi-line format for better readability
- Configurable file path
- Works across all translation modes

### ⚙️ **Customizable Hotkeys**
- Fully configurable translation hotkey
- Single keys (F1-F12)
- Modifier combinations (Alt+Q, Ctrl+Shift+T)
- Double-press patterns (Ctrl+Ctrl, Shift+Shift)
- No hardcoded hotkeys - complete flexibility

### ⚡ **Performance & Usability**
- Instant translations using Google Translate API
- Real-time configuration reloading (no restart required)
- Automatic clipboard copying (configurable)
- Smart terminal window management
- Multi-language support
- Colored terminal output (customizable)

## Platform Support

| Feature | Windows | Linux (X11) | Linux (Wayland) |
|---|---|---|---|
| Interactive mode | Yes | Yes | Yes |
| CLI mode | Yes | Yes | Yes |
| Clipboard read/write | Yes | Yes | Yes |
| Global hotkeys (Alt+Q, etc.) | Yes | Yes | No |
| Auto-copy selected text | Yes | Yes | No |
| Show/hide terminal | Yes | Yes | No |
| Auto-hide terminal | Yes | Yes | No |
| Text-to-speech | Yes | Yes | Yes |

**Wayland notes:**
- On Wayland, the application runs in **interactive and CLI modes only**. Global hotkeys and window management are disabled due to Wayland's security model which prevents applications from intercepting input or managing other windows.
- If XWayland is available, hotkeys may work through the X11 compatibility layer, but this is not guaranteed on all compositors.
- Clipboard read/write works natively on Wayland via the `arboard` crate.
- Full Wayland hotkey support (via `xdg-desktop-portal` GlobalShortcuts API) is planned for a future release.

## Installation

### Prerequisites
- **Windows**: Windows 10/11
- **Linux**: X11 or Wayland display server, `xdotool` (for hotkey auto-copy on X11)
- Internet connection for translations

## Download

**Latest Release**: [Download tagent.exe](https://github.com/holgertkey/tagent/releases/latest)

All releases: https://github.com/holgertkey/tagent/releases

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
- **GUI hotkeys** (Alt+Q) for system-wide translation

### CLI Mode
```bash
# Translate a single word
tagent hello

# Translate a phrase
tagent "Hello world"

# Text-to-speech (speaks the text)
tagent -s "Hello world"
tagent --speech "Привет мир"

# Translate with specific languages (names or codes)
tagent -l German hello
tagent -l en de "Hello world"

# Show help
tagent --help

# Show current configuration
tagent --config
```

## Usage Guide

### GUI Hotkeys (System-wide)

**Translation Hotkey** (default: Alt+Q)
1. Select any text in any Windows application
2. Press the translation hotkey (Alt+Q by default)
3. Translation appears in terminal and copies to clipboard
4. Paste anywhere with Ctrl+V

**Speech Hotkey** (default: Alt+E)
1. Select any text in any Windows application
2. Press the speech hotkey (Alt+E by default)
3. Text is spoken aloud using Google TTS
4. Press Esc to cancel playback

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
- `/?`, `/h`, `/help` - Show help
- `/c`, `/config` - Show current configuration
- `/v`, `/version` - Show version information
- `/s <text>`, `/speech <text>` - Text-to-speech (press Esc to cancel)
- `/l`, `/lang` - Swap source and target languages
- `/l <target>`, `/lang <target>` - Set target language (source=Auto)
- `/l <source> <target>`, `/lang <source> <target>` - Set both languages
- `/save` - Save current configuration to file
- `/clear`, `/cls` - Clear screen
- `/exit`, `/quit`, `/q` - Exit program

Language names (`English`, `German`) and codes (`en`, `de`) are both accepted.

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

; Detect and correct spelling errors, show correction notice
SpellCheck = true

[Interface]
; Show terminal window during GUI translation
ShowTerminalOnTranslate = true

; Auto-hide terminal after translation (seconds, 0 = disabled)
AutoHideTerminalSeconds = 3

; Terminal output colors (Red, Green, Blue, Yellow, Magenta, Cyan, White)
TranslationPromptColor = Green
DictionaryPromptColor = Cyan
AutoPromptColor = Yellow

[History]
; Save all translations to file with timestamps
SaveTranslationHistory = false

; History file path (defaults to AppData\Tagent folder)
HistoryFile = C:\Users\<YourName>\AppData\Roaming\Tagent\translation_history.txt

[Hotkeys]
; Translation hotkey (configurable)
; Formats:
;   - Single keys: F1-F12 (e.g., F9)
;   - Modifier combos: Alt+Q, Ctrl+Shift+T, Win+T
;   - Double-press: Ctrl+Ctrl, Shift+Shift, Alt+Alt, F8+F8
TranslateHotkey = Alt+Q

; Text-to-speech hotkey (same formats as TranslateHotkey)
SpeechHotkey = Alt+E

; Enable or disable the speech hotkey
EnableSpeechHotkey = true
```

### Customizing Hotkeys

Both translation and speech hotkeys are fully customizable. Edit `[Hotkeys]` section in config file:

**Single Keys (F1-F12 only)**
```ini
TranslateHotkey = F9
```

**Modifier Combinations**
```ini
TranslateHotkey = Alt+Q         # Alt + Q
TranslateHotkey = Ctrl+Shift+T  # Ctrl + Shift + T
TranslateHotkey = Win+T         # Windows key + T
TranslateHotkey = Alt+Space     # Alt + Spacebar
```

**Double-Press Patterns**
```ini
TranslateHotkey = Ctrl+Ctrl     # Double-press Ctrl
TranslateHotkey = Shift+Shift   # Double-press Shift
TranslateHotkey = Alt+Alt       # Double-press Alt
TranslateHotkey = F8+F8         # Double-press F8
```

**Speech Hotkey Examples**
```ini
SpeechHotkey = Alt+E          # Alt + E (default)
SpeechHotkey = F10            # Function key F10
SpeechHotkey = Ctrl+Shift+S   # Ctrl + Shift + S
SpeechHotkey = Win+S          # Windows key + S

; Disable speech hotkey
EnableSpeechHotkey = false
```

**Notes:**
- Changes require application restart
- Both hotkeys use the same format (single keys, combos, double-press)
- Avoid dangerous combinations (Ctrl+Alt+Del, Win+L)
- Some system shortcuts may be intercepted by Windows
- Single non-function keys require modifiers for safety
- Speech and translation hotkeys must be different

### Supported Languages
- **Auto-detection**: Auto
- **Major Languages**: English, Russian, Spanish, French, German, Chinese, Japanese, Korean, Italian, Portuguese, Dutch, Polish, Turkish, Arabic, Hindi
- **Language Codes**: en, ru, es, fr, de, zh, ja, ko, it, pt, nl, pl, tr, ar, hi
- **Speech Support**: All languages supported by Google TTS

## Translation History

When enabled (`SaveTranslationHistory = true`), all translations are logged in a readable format:

```
[2025-09-06 14:30:15 UTC] en -> ru
IN:  hello
OUT: привет
---

[2025-09-06 14:32:45 UTC] en -> ru
IN:  cat
OUT: [Word]: кот
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

### Spell Check
When a misspelled word is entered, the correct word is found automatically and a notice is shown:
```
[English]: vialent
Показан перевод слова violent
[Word]: жестокий
Прилагательное
  насильственный [violent, forcible]
  неистовый [violent, outrageous, frantic]
  яростный [furious, violent, raging]
```
The notice is shown in the target language. Works with both minor typos ("violnt") and heavily misspelled words ("vialent").

### Text-to-Speech Examples

**GUI Mode - Speech Hotkey**
```bash
# 1. Select text in any application
# 2. Press Alt+E (or your configured speech hotkey)
# 3. Text is spoken aloud
# 4. Press Esc to cancel playback
```

**CLI Mode**
```bash
tagent -s "Hello, how are you?"
tagent --speech "Привет, как дела?"
```

**Interactive Mode**
```bash
[Auto]: /s Hello world
# Speaks "Hello world" (press Esc to cancel)

[Auto]: /speech Bonjour le monde
# Speaks "Bonjour le monde" in French
```

**Speech Notes:**
- **GUI Speech Hotkey**: Select text → Press Alt+E (or configured key)
- Press **Esc** anytime to cancel speech playback
- Speech language determined by `SourceLanguage` config setting
- Long text is automatically chunked (100 char limit per chunk)
- Works in GUI (hotkey), Interactive, and CLI modes

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
# Translation Hotkey: Alt+Q
# Speech Hotkey: Alt+E
# Speech Hotkey Enabled: Yes
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
SpellCheck = false

[Interface]
ShowTerminalOnTranslate = false
```

### Configure Hotkeys
```ini
[Hotkeys]
; Translation hotkeys
TranslateHotkey = Alt+Q         # Use Alt+Q instead of Ctrl+Ctrl
TranslateHotkey = F9            # Or use function key
TranslateHotkey = Shift+Shift   # Or double-press Shift

; Speech hotkeys
SpeechHotkey = Alt+S            # Change speech hotkey to Alt+S
SpeechHotkey = F10              # Or use F10
EnableSpeechHotkey = false      # Disable speech hotkey if not needed
```

### Customize Colors
```ini
[Interface]
; Available colors: Red, Green, Blue, Yellow, Magenta, Cyan, White
TranslationPromptColor = Green
DictionaryPromptColor = Cyan
AutoPromptColor = Yellow
```

## Troubleshooting

### Common Issues

**"No selected text or clipboard is empty"**
- Ensure text is properly selected before pressing the translation hotkey
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
- Run as administrator if needed (Windows)
- Check if another application is capturing the hotkey
- Ensure application has keyboard input permissions
- Try changing the hotkey in config file (e.g., Alt+Q, F9)
- Restart the application after changing hotkey configuration
- Verify hotkey format in config file is correct
- **Linux (Wayland)**: Global hotkeys are not supported on Wayland — use interactive or CLI mode instead
- **Linux (X11)**: Ensure `xdotool` is installed (`sudo apt install xdotool`)

**Speech (TTS) not working**
- Check internet connection (uses Google TTS API)
- Verify audio device is working
- Try shorter text if speech fails
- Press Esc to cancel stuck speech playback
- Speech language is based on `SourceLanguage` config setting
- Check if `EnableSpeechHotkey` is set to `true` in config
- Verify speech hotkey is not conflicting with other applications
- Try changing `SpeechHotkey` to a different key combination

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
- **Windows**: Windows 10 or later
- **Linux**: X11 or Wayland, `xdotool` for hotkey auto-copy (X11 only)
- ~5MB disk space
- Network access for translations

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
git clone https://github.com/holgertkey/tagent
cd tagent
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

See [CHANGELOG.md](../CHANGELOG.md) for detailed version history and release notes.

**Current Version**: v0.14.0

**Recent Changes**:
- Spell checking for single words with correction notice in target language
- Code quality improvements (fixed all Clippy warnings)
- Text-to-speech (TTS) with Esc cancellation
- Fully configurable hotkeys (single keys, combos, double-press)
- Customizable terminal colors
- Translation history logging with timestamps
- Unified GUI + Interactive interface
- Configuration moved to AppData folder

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## Support

For issues, feature requests, or questions:
- Create an issue in the repository
- Check existing issues for solutions
- Review this README for common problems

---

**Tagent Text Translator v0.14.0** - Fast, reliable, and feature-rich translation tool for Windows and Linux.