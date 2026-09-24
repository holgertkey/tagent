# tagent-gui

A [Slint](https://slint.dev/) desktop GUI translator with a global "translate my
selection" hotkey, a system-tray icon, dictionary lookups and text-to-speech, built
directly on the [`tagent`](https://github.com/holgertkey/tagent/tree/main/tagent)
library (Google Translate by default).

`tagent-gui` is a fully independent application from
[`tagent-cli`](https://github.com/holgertkey/tagent/tree/main/tagent-cli) — its own
interface, its own configuration file, its own feature set, and its own versioning and
[CHANGELOG.md](CHANGELOG.md). The only thing the two share is the `tagent` library
underneath. It is not held to feature parity with `tagent-cli`.

## Install and run

```bash
cargo install tagent-gui        # from crates.io
tagent-gui
```

or, from a checkout of the repository:

```bash
cargo run -p tagent-gui
```

On Linux and macOS, starting it from a terminal doesn't hold that terminal: the app moves
itself to the background and the prompt comes back at once. Its diagnostics (config
warnings, "Global hotkeys disabled", speech errors) go to a log file,
`~/.local/share/tagent-gui/tagent-gui.log` on Linux and
`~/Library/Application Support/tagent-gui/tagent-gui.log` on macOS. To keep the app
attached, with its output in the terminal (e.g. while debugging), pass `--foreground`
(or `-f`):

```bash
tagent-gui --foreground
cargo run -p tagent-gui -- --foreground
```

On Windows the app has no console window of its own, so starting it from Explorer or a
shortcut shows only the GUI. Its diagnostics (config warnings, "Global hotkeys disabled",
speech errors) go to `stderr`, and they still appear if you start it from a terminal:

```powershell
cargo run -p tagent-gui                 # output stays in the terminal, in order
.\tagent-gui.exe | Out-Host             # PowerShell: waits for the app, keeps the output in order
.\tagent-gui.exe 2> gui.log             # or capture it in a file
```

```bat
start /wait tagent-gui.exe
```

Plain `.\tagent-gui.exe` also prints to the terminal, but `cmd` and PowerShell don't wait
for a GUI program, so the prompt returns at once and the output interleaves with it.

Building on Linux needs the X11, XTest, ALSA and fontconfig development packages, e.g.
on Debian/Ubuntu:

```bash
sudo apt-get install libx11-dev libxtst-dev libasound2-dev libfontconfig1-dev
```

The "translate the current selection" hotkey simulates Ctrl+C in the source application
through the X server's XTest extension, so it needs no extra programs, but it does need
X11 or XWayland.

By default the app starts minimized to the tray; click the tray icon (or use the hotkey)
to bring it up. Set `start_minimized` to `false` in `tagent-gui.json`, or untick it in
Settings > "Hotkeys & Tray", to open the window at launch.

## What it does

- **Translate.** Pick a source and target language, type text, press Enter (or click
  Translate); Shift+Enter inserts a newline. The ⇄ button swaps the languages, and 📋
  pulls the clipboard contents into the input box. Results accumulate in a selectable,
  copyable transcript. The language list is fixed at Auto/English/Russian/Spanish/French/German.
- **Dictionary.** A single word gets a dictionary entry instead of a plain translation
  (definitions grouped by part of speech, with a notice when the provider silently
  corrected a misspelling). Toggle with `show_dictionary` / `spell_check`.
- **Text-to-speech.** Every transcript row has 🔊 buttons for the phrase and for the
  translation. Only one plays at a time; the playing button turns into ⏹ and a second
  click stops it. Toggle with `enable_text_to_speech`.
- **Global hotkeys** (Linux and Windows):
  - `Alt+A` copies whatever you have selected in any application and translates it into
    the transcript, and shows the result in a small always-on-top popup next to the
    mouse cursor. The popup hides itself after `popup_auto_hide_seconds` (default 3)
    unless the cursor rests on it, and can be dragged; with
    `remember_popup_position` on, it reappears where you dropped it.
  - `Alt+S` speaks the current selection aloud, and adds a row with a replay button to
    the transcript. `Esc` stops whatever is speaking, from any application.
  - Both are configurable (see below) and take effect after a restart.
- **System tray.** A tray icon with "Show Tagent", "Settings…" and "Quit". Closing the
  main window hides it to the tray; "Quit" in the tray menu is the only way to exit.
  The hotkeys keep working while the window and tray are hidden — on a desktop without a
  tray host, that is the way back in.
- **Settings** (the ⚙ button, or "Settings…" in the tray menu):
  - *General* — translate, dictionary and speech providers (each an independent
    choice), and the dictionary/spell-check/text-to-speech switches.
  - *View* — theme (`Auto`/`Light`/`Dark`, applied live), color scheme, fonts, sizes and
    colors of the transcript, spacing.
  - *Hotkeys & Tray* — the two hotkeys (with a "Record" button that validates them
    live), the switch for the speech hotkey, start minimized, and remembering the window's
    size and position.
  - *Popup* — the popup's font, colors, auto-hide delay, size limits and border, and
    whether it remembers where you dragged it.
  - *About*.

## Platforms

| | Linux | Windows | macOS |
|---|---|---|---|
| Translate, dictionary, text-to-speech, tray, Settings | ✅ | ✅ | ✅ |
| Global hotkeys and selection popup | ✅ X11 / XWayland | ✅ | not implemented |

On Linux the hotkeys use X11 key grabbing, so they need X11 or XWayland; on a pure Wayland
session everything else still works. On macOS the window, dictionary, text-to-speech and
tray work, but the global hotkeys and the selection popup are stubs.

## Configuration

Settings live in a plain, pretty-printed JSON file that is meant to be hand-editable:

- Linux/macOS: `~/.config/tagent-gui/tagent-gui.json`
- Windows: `%APPDATA%\tagent-gui\tagent-gui.json`

A missing file is created with defaults on first run. Changes are live-reloaded (checked
before each translation), except for the hotkeys, `start_minimized` and the tray, which
are read at startup. A file that is present but invalid is left untouched: the app logs
a warning and keeps using its last valid settings. It does not read `tagent-cli`'s
`tagent-cli.conf`.

The main keys:

| Key | Default | |
|---|---|---|
| `translate_provider` / `dictionary_provider` / `speech_provider` | `"google"` | Three independent backends |
| `theme` | `"Auto"` | `Auto`, `Light` or `Dark` |
| `translate_hotkey` / `speech_hotkey` | `"Alt+A"` / `"Alt+S"` | `F1`–`F12`, `Modifier+Key`, or a double press like `Ctrl+Ctrl` |
| `enable_speech_hotkey` | `true` | |
| `show_dictionary` / `spell_check` | `true` | |
| `enable_text_to_speech` | `true` | Shows the 🔊 buttons |
| `popup_auto_hide_seconds` | `3` | `0` means the default, not "never" |
| `remember_popup_position` | `false` | |
| `start_minimized` | `true` | |
| `remember_window_geometry` | `true` | |

The provider dropdowns are filled from the `tagent` library, so a new backend appears in
Settings as soon as the library offers it. The file also accepts any provider name by
hand, and a bad or missing hotkey only disables that hotkey.

## Notes

- `Auto` theme follows the system setting. On Linux, a fresh window may briefly flash
  light before settling into dark; this is an upstream Slint/winit limitation
  ([`slint-ui/slint#4392`](https://github.com/slint-ui/slint/issues/4392)). Pick `Light`
  or `Dark` explicitly to avoid it.
- No translation history logging yet (`tagent-cli` has one).

## Status

Early (`0.x`): usable day to day, but settings and behavior may still change between
releases. See [CHANGELOG.md](CHANGELOG.md) for the version history and the
[architecture notes](https://github.com/holgertkey/tagent/blob/main/docs/ARCHITECTURE.md)
for how it works.
