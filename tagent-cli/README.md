# Tagent Text Translator v0.17.0+012

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
- Can be disabled via `spell_check = false` in config

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
- Configuration reloads automatically before each translation (hotkeys and the dictionary provider need a restart)
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
| Clipboard (`copy_to_clipboard`) | Yes | Yes | No | No |
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

# Print a new config file with every setting, its explanation and its default
# (touches no file; e.g. to compare with yours)
tagent-cli --print-default-config > tagent-cli.new.toml

# Add the settings a newer version introduced to your config file (backup: .bak)
tagent-cli --update-config
```

`-l` only affects that one run; it doesn't change the config file.

## Usage Guide

### GUI Hotkeys (System-wide)

**Translation Hotkey** (default: Alt+A)
1. Select text in any application
2. Press the translation hotkey (Alt+A by default)
3. The translation appears in the terminal (and is copied to the clipboard if `copy_to_clipboard = true`)

**Speech Hotkey** (default: Alt+S)
1. Select text in any application
2. Press the speech hotkey (Alt+S by default)
3. Text is spoken aloud using Google TTS
4. Press Esc to cancel playback

### Interactive Terminal
The prompt shows the current language pair, and a translation is labeled with the provider that made it (`/p` switches it). Phrases are translated; single words get a dictionary entry:
```
[auto → ru]: How are you?
[google]: Как вы?

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
- `/p`, `/provider` - List the translation providers (the active one is marked `*`)
- `/p <name>`, `/provider <name>` - Switch the translation provider for this session (a provider or profile name, e.g. `/p deepl`); `/save` keeps it
- `/save` - Save the languages and the translation provider to the config file
- `/config update` - Add the settings your config file lacks (see "After Upgrading Tagent")
- `/clear`, `/cls` - Clear screen
- `/exit`, `/quit`, `/q`, `/e` - Exit program

Language names (`English`, `German`) and codes (`en`, `de`) are both accepted. Arrow keys edit the line, Ctrl+R searches the input history (kept across sessions), and Tab completes commands.

## Configuration

The configuration file is [TOML](https://toml.io), created with defaults on first run and reloaded automatically:
- **Windows**: `%APPDATA%\tagent-cli\tagent-cli.toml` (typically `C:\Users\<YourName>\AppData\Roaming\tagent-cli\tagent-cli.toml`)
- **Linux/macOS**: `~/.config/tagent-cli/tagent-cli.toml`

The generated file documents every setting in its comments. Its sections, with the default values:

```toml
[provider]
# Translation backend: a provider profile name (google, deepl)
translate_provider = "google"

[translation]
# Language codes (en, ru, de, ...; names such as "German" work too)
# Source language: "auto" detects it
source_language = "auto"
# Target language (never auto); default: your system language, else en
target_language = "ru"

[dictionary]
# Show a dictionary entry for single words
show_dictionary = true
# Detect and correct spelling errors, show a correction notice
spell_check = true
# Dictionary backend, independent of translate_provider (google; restart required)
dictionary_provider = "google"

[interface]
# Show the terminal window during hotkey translation
show_terminal_on_translate = true
# Auto-hide the terminal after translation (seconds, 0 = disabled)
auto_hide_terminal_seconds = 3
# Copy results to the clipboard automatically
copy_to_clipboard = false

[colors]
# Terminal output colors: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White,
# their Bright* variants (e.g. BrightYellow), or None
source_prompt_color = "None"
target_prompt_color = "BrightYellow"
dictionary_prompt_color = "BrightYellow"
# Dictionary article highlighting and status messages
part_of_speech_color = "Cyan"
synonym_color = "Green"
notice_color = "Magenta"
error_color = "Red"

[history]
# Save all translations to a file with timestamps
save_translation_history = false
# History file path (default: translation_history.txt in the per-user data folder,
# %APPDATA%\tagent-cli on Windows, ~/.local/share/tagent-cli on Linux)
history_file = "..."

[hotkeys]
# Translation hotkey (restart required)
# Formats:
#   - Single keys: F1-F12 (e.g., F9)
#   - Modifier combos: Alt+Q, Ctrl+Shift+T, Win+T
#   - Double-press: Ctrl+Ctrl, Shift+Shift, Alt+Alt, F8+F8
translate_hotkey = "Alt+A"

[speech]
# Enable text-to-speech
enable_text_to_speech = true
# Text-to-speech hotkey (same formats as translate_hotkey; restart required)
speech_hotkey = "Alt+S"
# Enable or disable the speech hotkey
enable_speech_hotkey = true
# Speech backend, independent of translate_provider (google)
speech_provider = "google"
```

Strings are quoted; `true`/`false` and numbers are not. A comment can follow a value on the
same line (`copy_to_clipboard = true  # handy`). On Windows, a path with backslashes goes in
single quotes (`history_file = 'C:\Users\me\history.txt'`) or has them doubled. A missing
key or section takes its default. A key or section Tagent doesn't know is reported as a
warning with its line and, when it looks like a typo or a key in the wrong section, the
likely intended name; the rest of the file still applies.

A mistake in the file (a syntax error, or e.g. `copy_to_clipboard = "yes"`) is reported
with its line and column: at startup Tagent exits with the message; while running it prints
a warning once and keeps the previous settings until the file is fixed.

`/save` updates only `source_language`, `target_language` and `translate_provider` in the
file, in place: your comments, the key order and everything else stay as they are. It
writes the languages as codes, so a file from an older version that has names
(`target_language = "Russian"`) gets codes on its first `/save`.

The languages are language codes. The generated file lists the ones Tagent knows by
name (`en (English), ru (Russian), ...`); names work too, in any case, and any other code
(e.g. `"uk"`) is passed to the translation service as it is, with a warning. On first run,
the target language is your system language (from the locale: `LANGUAGE`/`LC_ALL`/
`LC_MESSAGES`/`LANG` on Linux, the preferred UI languages on Windows and macOS) when Tagent
knows it, else English.

### After Upgrading Tagent

Tagent never rewrites your config file on its own, so settings added by a newer version
don't appear in it by themselves. At startup, Tagent tells you when there are some
(`Config: 3 new settings are available ...`). To add them:

```bash
tagent-cli --update-config      # or /config update at the interactive prompt
```

This adds each missing setting with its explanation and default value (and the example
provider profiles, if your file doesn't have them) and keeps everything else: your values,
comments and key order. The previous file is kept as `tagent-cli.toml.bak`. Nothing is
removed or renamed; keys this version doesn't know are listed for you to handle. To keep a
setting out of your file for good, leave it commented out (`# speech_hotkey = "Alt+S"`):
a commented-out setting isn't added back and doesn't count as new.
`tagent-cli --print-default-config` shows a complete new file for comparison.

### Provider Profiles and API Keys

`translate_provider`, `dictionary_provider` and `speech_provider` take a **profile name**. A
built-in provider name (`google`) works as is; a `[provider_options.<name>]` table adds
options for it, or defines a new profile whose optional `type` key picks the provider kind
(default: the profile name). One profile can serve several of the three settings.

```toml
[provider]
translate_provider = "work"

[provider_options.work]
type = "google"
timeout_secs = "20"   # time budget for one request in seconds, retries included
max_retries = "0"     # 0 disables the automatic retry

# Options for the built-in name itself
[provider_options.google]
max_retries = "0"
```

Every option value is a quoted string, numbers too. Keys are passed to the provider as
written (e.g. `api_key`, `endpoint`, `model`, for providers that need them). An environment
variable `TAGENT_<NAME>_<KEY>` overrides a key, e.g. `TAGENT_WORK_API_KEY`, so an API key
never has to be written to the file. `/config` lists every profile with its effective
options, secret values masked and each value's origin shown. On Linux/macOS the config file
is written with permissions `0600` (owner only).

#### DeepL

DeepL (translation only) needs an API key from your DeepL account; a Free key ends in `:fx`
and is sent to DeepL's Free API automatically:

```toml
[provider]
translate_provider = "deepl"

[provider_options.deepl]
api_key = "your-key:fx"   # or leave it out and set TAGENT_DEEPL_API_KEY
# endpoint = "https://api.deepl.com"   # optional; normally chosen by the key
```

Keep `dictionary_provider` and `speech_provider` on `google`. Speaking text whose source
language is `Auto` asks DeepL to detect the language, which bills up to 100 characters.

### Customizing Hotkeys

Both translation and speech hotkeys are fully customizable: `translate_hotkey` in the `[hotkeys]` section, `speech_hotkey` in `[speech]`. Pick one line from each example.

**Single Keys (F1-F12 only)**
```toml
translate_hotkey = "F9"
```

**Modifier Combinations**
```toml
translate_hotkey = "Alt+Q"          # Alt + Q
translate_hotkey = "Ctrl+Shift+T"   # Ctrl + Shift + T
translate_hotkey = "Win+T"          # Windows key + T
translate_hotkey = "Alt+Space"      # Alt + Spacebar
```

**Double-Press Patterns** (F1-F12 or a modifier key)
```toml
translate_hotkey = "Ctrl+Ctrl"      # Double-press Ctrl
translate_hotkey = "Shift+Shift"    # Double-press Shift
translate_hotkey = "Alt+Alt"        # Double-press Alt
translate_hotkey = "F8+F8"          # Double-press F8
```

**Speech Hotkey Examples**
```toml
[speech]
speech_hotkey = "Alt+S"             # Alt + S (default)
speech_hotkey = "F10"               # Function key F10
speech_hotkey = "Ctrl+Shift+S"      # Ctrl + Shift + S
speech_hotkey = "Win+S"             # Windows key + S

# Disable speech hotkey
enable_speech_hotkey = false
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

When enabled (`save_translation_history = true`), all translations are logged in a readable format:

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
[google]: Как вы?
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
[google]: Доброе утро
[auto → ru]: /s
# Speaks "Good morning" again
[auto → ru]: /ss
# Speaks "Доброе утро" in Russian
```

**Speech Notes:**
- **GUI Speech Hotkey**: Select text → Press Alt+S (or configured key)
- Press **Esc** anytime to cancel speech playback
- Speech language is the `source_language` setting, detected from the text when it's Auto; `/s` and `/ss` without text use the source and target language of the last translation (as they were when it was made)
- Long text is automatically chunked (100 char limit per chunk)
- Works in GUI (hotkey), Interactive, and CLI modes

### Configuration Management
```bash
# Show current settings
tagent-cli --config

# Output (the settings as they appear in tagent-cli.toml):
# === Current Configuration ===
# [provider]
# translate_provider = "google"
#
# [translation]
# source_language = "auto"  # Auto
# target_language = "ru"  # Russian
#
# [dictionary]
# show_dictionary = true
# ...
#
# [speech]
# enable_text_to_speech = true
# speech_hotkey = "Alt+S"
# enable_speech_hotkey = true
# speech_provider = "google"
#
# # Provider profiles (effective options, secrets masked)
# [provider_options.google]
# # (no options)
#
# # Config file: /home/<you>/.config/tagent-cli/tagent-cli.toml
```

## Advanced Usage

### Custom Language Pairs
Edit `tagent-cli.toml` (or use `/l` in the interactive terminal, then `/save`):
```toml
[translation]
source_language = "en"
target_language = "es"
```

### Enable History Logging
```toml
[history]
save_translation_history = true
history_file = "my_translations.txt"
```

### Disable Automatic Features
```toml
[dictionary]
show_dictionary = false
spell_check = false

[interface]
show_terminal_on_translate = false
copy_to_clipboard = false
```

### Configure Hotkeys
```toml
[hotkeys]
translate_hotkey = "Alt+Q"          # Use Alt+Q instead of Alt+A

[speech]
speech_hotkey = "F10"               # Change the speech hotkey to F10
enable_speech_hotkey = false        # Or disable it if not needed
```

### Customize Colors
```toml
[colors]
# Black, Red, Green, Yellow, Blue, Magenta, Cyan, White, their Bright* variants, or None
source_prompt_color = "None"             # "[auto → ru]: " prompt
target_prompt_color = "BrightYellow"     # "[google]: " label (the provider)
dictionary_prompt_color = "BrightYellow" # "[Word]: " label
part_of_speech_color = "Cyan"            # "Noun", "Прилагательное", ... in a dictionary entry
synonym_color = "Green"                  # "[fierce, brutal]" in a dictionary entry
notice_color = "Magenta"                 # spelling-correction notice
error_color = "Red"                      # translation, speech, clipboard and history errors
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

**"invalid configuration file ..." / "Config reload error"**
- The message names the line and column: fix the value there (strings need quotes, `true`/`false` and numbers don't; provider option values are always quoted)
- Ensure file is not locked by another application
- Delete the config file to regenerate default settings
- An old `tagent-cli.conf` (the INI file of versions before 0.17.0) is no longer read: copy your settings into `tagent-cli.toml` (see [Configuration](#configuration))

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
- Speech language is based on the `source_language` config setting
- Check that `enable_text_to_speech` and `enable_speech_hotkey` are set to `true` in the `[speech]` section
- Verify speech hotkey is not conflicting with other applications
- Try changing `speech_hotkey` to a different key combination

### Performance Tips

- Use `show_terminal_on_translate = false` for faster GUI translations
- Set `auto_hide_terminal_seconds = 0` to keep terminal visible

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

**Current Version**: v0.17.0+012

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

**Tagent Text Translator v0.17.0+012** - Fast, reliable, and feature-rich translation tool for Windows and Linux.
