# Tagent Text Translator v0.16.0+007

A fast, lightweight text translation tool with unified GUI hotkeys, interactive terminal, and CLI interfaces. Translate selected text from any application with a simple Alt+A hotkey or use the command line for quick translations. Full support on Windows and Linux (X11 or XWayland); on pure Wayland and on macOS, the interactive terminal and CLI modes work.

## Features

### 🔥 **Unified Translation Modes**
- **GUI Hotkeys**: Select text anywhere, press Alt+A, get instant translation
- **Interactive Terminal**: Type text directly in the terminal prompt
- **CLI Mode**: One-time translations from command line

### 📚 **Smart Dictionary Lookup**
- Single words get a dictionary entry: translations grouped by part of speech, each with its synonyms
- Part-of-speech names in the target language, highlighted in color
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
- Speech hotkey for selected text (default: Alt+S)
- Replay the last translation in the interactive terminal: `/s` speaks the phrase, `/ss` its translation
- Press Esc to cancel speech playback
- Automatic language detection for speech
- Supports long text with automatic chunking

### 📝 **Translation History**
- Optional logging of all translations with timestamps
- Multi-line format for better readability
- Configurable file path
- Works across all translation modes

### ⚙️ **Customizable Hotkeys**
- Fully configurable translation and speech hotkeys
- Single keys (F1-F12)
- Modifier combinations (Alt+Q, Ctrl+Shift+T)
- Double-press patterns (Ctrl+Ctrl, Shift+Shift)
- No hardcoded hotkeys - complete flexibility

### ⚡ **Performance & Usability**
- Instant translations using Google Translate API
- Configuration reloads automatically before each translation (hotkeys and providers need a restart)
- Interactive prompt with line editing, persistent input history and Tab-completion of commands
- Optional automatic clipboard copying
- Smart terminal window management
- Multi-language support
- Colored terminal output (customizable)

## Platform Support

| Feature | Windows | Linux (X11 / XWayland) | Linux (pure Wayland) | macOS |
|---|---|---|---|---|
| Interactive mode | Yes | Yes | Yes | Yes |
| CLI mode | Yes | Yes | Yes | Yes |
| Text-to-speech | Yes | Yes | Yes | Yes |
| Clipboard (`CopyToClipboard`) | Yes | Yes | No | No |
| Global hotkeys (Alt+A, etc.) | Yes | Yes | No | No |
| Auto-copy selected text | Yes | Yes | No | No |
| Show/hide terminal | Yes | Yes | No | No |
| Auto-hide terminal | Yes | Yes | No | No |

**Wayland notes:**
- Most Wayland desktops (GNOME, KDE) also run XWayland; with it, everything works as on X11.
- On a pure Wayland session (no `DISPLAY`), the application runs in **interactive and CLI modes only**. Global hotkeys, auto-copy and window management are disabled, because Wayland's security model prevents applications from intercepting input or managing other windows.
- Full Wayland hotkey support (via the `xdg-desktop-portal` GlobalShortcuts API) is planned for a future release.

**macOS notes:** the platform layer (clipboard, global hotkeys, window management) is not implemented yet; the interactive and CLI modes work.

## Installation

### Prerequisites
- **Windows**: Windows 10/11
- **Linux**: X11 or XWayland for the hotkeys (interactive and CLI modes work anywhere)
- Internet connection for translations

## Download

**Latest Release**: [github.com/holgertkey/tagent/releases/latest](https://github.com/holgertkey/tagent/releases/latest)

All releases: https://github.com/holgertkey/tagent/releases

### Download & Setup
1. Download the archive for your system: `tagent-cli-<version>-windows-x86_64.zip` or `tagent-cli-<version>-linux-x86_64.tar.gz`
2. Extract it to your preferred directory
3. Run `tagent-cli` (`tagent-cli.exe` on Windows) to start unified mode
4. The configuration file is created automatically on first run (see [Configuration](#configuration))

Or install from crates.io: `cargo install tagent-cli` (on Linux this needs the build packages listed in [Building from Source](#building-from-source)).

## Quick Start

### Unified Mode (Recommended)
```bash
# Start unified mode (no arguments)
tagent-cli
```
This starts both:
- **Interactive prompt** in the terminal
- **GUI hotkeys** (Alt+A) for system-wide translation

### CLI Mode
```bash
# Translate a single word (shows a dictionary entry)
tagent-cli hello

# Translate a phrase
tagent-cli "Hello world"

# Text-to-speech (speaks the text)
tagent-cli -s "Hello world"
tagent-cli --speech "Привет мир"

# Translate with specific languages (names or codes)
tagent-cli -l German hello
tagent-cli -l en de "Hello world"

# Show help
tagent-cli --help

# Show current configuration
tagent-cli --config
```

`-l` only affects that one run; it doesn't change the config file.

## Usage Guide

### GUI Hotkeys (System-wide)

**Translation Hotkey** (default: Alt+A)
1. Select text in any application
2. Press the translation hotkey (Alt+A by default)
3. The translation appears in the terminal (and is copied to the clipboard if `CopyToClipboard = true`)

**Speech Hotkey** (default: Alt+S)
1. Select text in any application
2. Press the speech hotkey (Alt+S by default)
3. Text is spoken aloud using Google TTS
4. Press Esc to cancel playback

### Interactive Terminal
The prompt shows the current language pair. Phrases are translated; single words get a dictionary entry:
```
[auto → ru]: How are you?
[Russian]: Как вы?

[auto → ru]: translate
[Word]: переводить
Глагол
  переводить [transfer, translate, convert, move, interpret, put]
  транслировать [translate, transmit, relay, compile]
  преобразовывать [translate, reform, reorganize, transfashion, reorganise]
  переносить [transfer, carry, transport, bear, stand, translate]
  переводиться [transfer, translate]

[auto → ru]: /q

Goodbye!
```
The words in brackets are synonyms in the source language, i.e. other words with that meaning.

### Interactive Commands
- `/?`, `/h`, `/help` - Show help
- `/c`, `/config` - Show current configuration
- `/v`, `/version` - Show version information
- `/s <text>`, `/speech <text>` - Text-to-speech (press Esc to cancel)
- `/s`, `/speech` - Speak the last translated phrase (typed or via hotkey)
- `/ss` - Speak the translation of the last phrase (for a single word, just its main translation)
- `/l`, `/lang` - Swap source and target languages
- `/l <target>`, `/lang <target>` - Set target language (source=Auto)
- `/l <source> <target>`, `/lang <source> <target>` - Set both languages
- `/save` - Save current configuration to file
- `/clear`, `/cls` - Clear screen
- `/exit`, `/quit`, `/q`, `/e` - Exit program

Language names (`English`, `German`) and codes (`en`, `de`) are both accepted. Arrow keys edit the line, Ctrl+R searches the input history (kept across sessions), and Tab completes commands.

## Configuration

The configuration file is created with defaults on first run and reloads automatically:
- **Windows**: `%APPDATA%\tagent-cli\tagent-cli.conf` (typically `C:\Users\<YourName>\AppData\Roaming\tagent-cli\tagent-cli.conf`)
- **Linux/macOS**: `~/.config/tagent-cli/tagent-cli.conf`

The generated file documents every setting in its comments. Its sections, with the default values:

```ini
[Provider]
; Translation backend (google)
TranslateProvider = google

[Translation]
; Source language (Auto, English, Russian, Spanish, etc.)
SourceLanguage = Auto
; Target language (never Auto)
TargetLanguage = Russian

[Dictionary]
; Show a dictionary entry for single words
ShowDictionary = true
; Detect and correct spelling errors, show a correction notice
SpellCheck = true
; Dictionary backend, independent of TranslateProvider (google; restart required)
DictionaryProvider = google

[Interface]
; Show the terminal window during hotkey translation
ShowTerminalOnTranslate = true
; Auto-hide the terminal after translation (seconds, 0 = disabled)
AutoHideTerminalSeconds = 3
; Copy results to the clipboard automatically
CopyToClipboard = false

[Colors]
; Terminal output colors: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
; their Bright* variants (e.g. BrightYellow), or None
SourcePromptColor = None
TargetPromptColor = BrightYellow
DictionaryPromptColor = BrightYellow
; Dictionary article highlighting and status messages
PartOfSpeechColor = Cyan
SynonymColor = Green
NoticeColor = Magenta
ErrorColor = Red

[History]
; Save all translations to a file with timestamps
SaveTranslationHistory = false
; History file path (default: translation_history.txt in the per-user data folder,
; %APPDATA%\tagent-cli on Windows, ~/.local/share/tagent-cli on Linux)
HistoryFile = ...

[Hotkeys]
; Translation hotkey (restart required)
; Formats:
;   - Single keys: F1-F12 (e.g., F9)
;   - Modifier combos: Alt+Q, Ctrl+Shift+T, Win+T
;   - Double-press: Ctrl+Ctrl, Shift+Shift, Alt+Alt, F8+F8
TranslateHotkey = Alt+A

[Speech]
; Enable text-to-speech
EnableTextToSpeech = true
; Text-to-speech hotkey (same formats as TranslateHotkey; restart required)
SpeechHotkey = Alt+S
; Enable or disable the speech hotkey
EnableSpeechHotkey = true
; Speech backend, independent of TranslateProvider (google)
SpeechProvider = google
```

### Customizing Hotkeys

Both translation and speech hotkeys are fully customizable: `TranslateHotkey` in the `[Hotkeys]` section, `SpeechHotkey` in `[Speech]`.

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

**Double-Press Patterns** (F1-F12 or a modifier key)
```ini
TranslateHotkey = Ctrl+Ctrl     # Double-press Ctrl
TranslateHotkey = Shift+Shift   # Double-press Shift
TranslateHotkey = Alt+Alt       # Double-press Alt
TranslateHotkey = F8+F8         # Double-press F8
```

**Speech Hotkey Examples**
```ini
[Speech]
SpeechHotkey = Alt+S          # Alt + S (default)
SpeechHotkey = F10            # Function key F10
SpeechHotkey = Ctrl+Shift+S   # Ctrl + Shift + S
SpeechHotkey = Win+S          # Windows key + S

; Disable speech hotkey
EnableSpeechHotkey = false
```

**Notes:**
- Changes require application restart
- Both hotkeys use the same format (single keys, combos, double-press)
- Dangerous combinations (Ctrl+Alt+Del, Win+L) are rejected
- Some system shortcuts may be intercepted by the OS before they reach Tagent
- Single non-function keys require modifiers for safety; Shift+Key alone isn't allowed (it interferes with typing)
- Use different combinations for the speech and translation hotkeys

### Supported Languages
- **Auto-detection**: Auto (source language only)
- **Major Languages**: English, Russian, Spanish, French, German, Chinese, Japanese, Korean, Italian, Portuguese, Dutch, Polish, Turkish, Arabic, Hindi
- **Language Codes**: en, ru, es, fr, de, zh, ja, ko, it, pt, nl, pl, tr, ar, hi
- **Speech Support**: All languages supported by Google TTS

## Translation History

When enabled (`SaveTranslationHistory = true`), all translations are logged in a readable format:

```
[2026-09-26 14:30:15 UTC] auto -> ru
IN:  How are you?
OUT: Как вы?
---

[2026-09-26 14:32:45 UTC] auto -> ru
IN:  translate
OUT: переводить
Глагол
  переводить [transfer, translate, convert, move, interpret, put]
  транслировать [translate, transmit, relay, compile]
---
```

## Examples

### Basic Translation
```bash
# CLI
tagent-cli "How are you?"
# Output: Как вы?

# Interactive
[auto → ru]: How are you?
[Russian]: Как вы?
```

### Dictionary Lookup
```bash
# CLI
tagent-cli translate
# Output:
# переводить
# Глагол
#   переводить [transfer, translate, convert, move, interpret, put]
#   транслировать [translate, transmit, relay, compile]
#   ...
```

### Spell Check
When a misspelled word is entered, the correct word is found automatically and a notice is shown:
```
[en → ru]: vialent
Показан перевод слова violent
[Word]: яростный
Прилагательное
  насильственный [violent, forcible]
  сильный [strong, keen, powerful, severe, heavy, violent]
  неистовый [violent, outrageous, frantic, frenetic, berserk, fierce]
  яростный [furious, violent, raging, rabid, stormy, rageful]
```
The notice is shown in the target language. Works with both minor typos ("violnt") and heavily misspelled words ("vialent").

### Text-to-Speech Examples

**GUI Mode - Speech Hotkey**
```bash
# 1. Select text in any application
# 2. Press Alt+S (or your configured speech hotkey)
# 3. Text is spoken aloud
# 4. Press Esc to cancel playback
```

**CLI Mode**
```bash
tagent-cli -s "Hello, how are you?"
tagent-cli --speech "Привет, как дела?"
```

**Interactive Mode**
```bash
[auto → ru]: /s Hello world
# Speaks "Hello world" (press Esc to cancel)

[auto → ru]: /speech Bonjour le monde
# Speaks "Bonjour le monde" in French

[auto → ru]: Good morning
[Russian]: Доброе утро
[auto → ru]: /s
# Speaks "Good morning" again
[auto → ru]: /ss
# Speaks "Доброе утро" in Russian
```

**Speech Notes:**
- **GUI Speech Hotkey**: Select text → Press Alt+S (or configured key)
- Press **Esc** anytime to cancel speech playback
- Speech language is the `SourceLanguage` setting, detected from the text when it's Auto; `/s` and `/ss` without text use the source and target language of the last translation (as they were when it was made)
- Long text is automatically chunked (100 char limit per chunk)
- Works in GUI (hotkey), Interactive, and CLI modes

### Configuration Management
```bash
# Show current settings
tagent-cli --config

# Output:
# === Current Configuration ===
# Translation Provider: google
# Dictionary Provider: google
# Speech Provider: google
#
# Source Language: Auto (auto)
# Target Language: Russian (ru)
# Show Dictionary: Enabled
# Copy to Clipboard: Disabled
#
# Translation Hotkey: Alt+A
# Show Terminal on Translate: Enabled
# Auto-hide Terminal: 3 seconds
#
# Text-to-Speech: Enabled
# Speech Hotkey: Alt+S
# Speech Hotkey Enabled: Yes
#
# Save Translation History: Disabled
# History File: /home/<you>/.local/share/tagent-cli/translation_history.txt
#
# Config file: /home/<you>/.config/tagent-cli/tagent-cli.conf
```

## Advanced Usage

### Custom Language Pairs
Edit `tagent-cli.conf` (or use `/l` in the interactive terminal, then `/save`):
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
[Dictionary]
ShowDictionary = false
SpellCheck = false

[Interface]
ShowTerminalOnTranslate = false
CopyToClipboard = false
```

### Configure Hotkeys
```ini
[Hotkeys]
TranslateHotkey = Alt+Q         # Use Alt+Q instead of Alt+A
TranslateHotkey = F9            # Or use function key
TranslateHotkey = Shift+Shift   # Or double-press Shift

[Speech]
SpeechHotkey = Alt+E            # Change speech hotkey to Alt+E
SpeechHotkey = F10              # Or use F10
EnableSpeechHotkey = false      # Disable speech hotkey if not needed
```

### Customize Colors
```ini
[Colors]
; Black, Red, Green, Yellow, Blue, Magenta, Cyan, White, their Bright* variants, or None
SourcePromptColor = None        ; "[auto → ru]: " prompt
TargetPromptColor = BrightYellow ; "[Russian]: " label
DictionaryPromptColor = BrightYellow ; "[Word]: " label
PartOfSpeechColor = Cyan        ; "Noun", "Прилагательное", ... in a dictionary entry
SynonymColor = Green            ; "[fierce, brutal]" in a dictionary entry
NoticeColor = Magenta           ; spelling-correction notice
ErrorColor = Red                ; translation, speech, clipboard and history errors
```

Colors are only emitted when output goes to a terminal: piping or redirecting
(`tagent-cli word > out.txt`) gives plain text, and so do `NO_COLOR` and `CLICOLOR=0`.
The clipboard and the history file always get plain text.

## Troubleshooting

### Common Issues

**"No selected text or clipboard is empty"**
- Ensure text is properly selected before pressing the translation hotkey
- Try selecting text again
- Check if another application is interfering with clipboard
- On pure Wayland, auto-copy isn't available (see [Platform Support](#platform-support))

**"Translation failed: ..."**
- Check internet connection
- Verify firewall settings allow the application
- Google Translate service may be temporarily unavailable

**"Config reload error"**
- Check the config file syntax (see [Configuration](#configuration) for its location)
- Ensure file is not locked by another application
- Delete the config file to regenerate default settings

**Hotkeys not working**
- Run as administrator if needed (Windows)
- Check if another application is capturing the hotkey (on Linux, Tagent logs a warning when the combination is already grabbed)
- Try changing the hotkey in config file (e.g., Alt+Q, F9)
- Restart the application after changing hotkey configuration
- Verify hotkey format in config file is correct
- **Linux (pure Wayland)** and **macOS**: global hotkeys are not supported — use interactive or CLI mode instead

**Speech (TTS) not working**
- Check internet connection (uses Google TTS API)
- Verify audio device is working
- Try shorter text if speech fails
- Press Esc to cancel stuck speech playback
- Speech language is based on `SourceLanguage` config setting
- Check that `EnableTextToSpeech` and `EnableSpeechHotkey` are set to `true` in the `[Speech]` section
- Verify speech hotkey is not conflicting with other applications
- Try changing `SpeechHotkey` to a different key combination

### Performance Tips

- Use `ShowTerminalOnTranslate = false` for faster GUI translations
- Set `AutoHideTerminalSeconds = 0` to keep terminal visible

## Technical Details

### Dependencies
- **tagent**: the translation, dictionary and speech library from this repository
- **Tokio**: Async runtime
- **rodio**: Audio playback for text-to-speech
- **rustyline**: Line editing and input history in the interactive terminal
- **Chrono**: Timestamp handling for history
- **Windows**: Win32 API (keyboard hook, window management) and `clipboard-win`
- **Linux**: `rdev` and X11/XTest (hotkeys, simulated copy), `arboard` (clipboard)

The full list is in [`Cargo.toml`](Cargo.toml).

### System Requirements
- **Windows**: Windows 10 or later
- **Linux**: X11 or XWayland for the hotkeys; any terminal for interactive and CLI modes
- ~5MB disk space
- Network access for translations

### Architecture
- **Rust**: Safe, fast systems programming
- **Async/await**: Non-blocking translation requests
- **Platform layer**: low-level keyboard hook on Windows, X11 key grabbing on Linux
- **Live config**: the file's modification time is checked before each translation

## Building from Source

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Linux (Debian/Ubuntu): X11, XTest and ALSA development packages
sudo apt-get install libx11-dev libxtst-dev libasound2-dev
```

### Build
```bash
git clone https://github.com/holgertkey/tagent
cd tagent
cargo build --release -p tagent-cli
```

The binary lands at `target/release/tagent-cli` (`target/release/tagent-cli.exe` on Windows).

## Version History

See [CHANGELOG.md](CHANGELOG.md) for detailed version history and release notes.

**Current Version**: v0.16.0+007

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

**Tagent Text Translator v0.16.0+007** - Fast, reliable, and feature-rich translation tool for Windows and Linux.
