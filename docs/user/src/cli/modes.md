# Modes

`tagent-cli` works in one of two modes, chosen by its arguments.

## Unified mode (no arguments)

```bash
tagent-cli
```

Two ways to translate run side by side until you quit:

- **The interactive prompt** in the terminal: type text, press Enter. See
  [Interactive commands](interactive-commands.md).
- **The global hotkeys**, in any application: select text and press the translate hotkey
  (Alt+A) or the speech hotkey (Alt+S). See [Hotkeys](hotkeys.md).

Both use the same settings and the same session state: a language switched with `/l`
applies to the hotkey too, and `/s` can replay a phrase you translated with the hotkey.

At start, a banner shows the languages, the providers, the active hotkeys and the main
commands. If your configuration file lacks settings a newer version added, a line under
the banner says so; see [Upgrading](../troubleshooting/upgrading.md).

## CLI mode (with arguments)

```bash
tagent-cli "Hello world"          # translate a phrase
tagent-cli translate              # a single word: a dictionary entry
tagent-cli -l de "Hello world"    # into German, for this run only
tagent-cli -l en de "Hello world" # from English into German
tagent-cli -s "Hello world"       # read aloud
```

One translation, then it exits. No hotkeys, no prompt. The output is plain text when it
goes to a file or a pipe, so it works in scripts:

```bash
tagent-cli -l en "Guten Morgen" > out.txt
```

`-l` doesn't change the configuration file. All options are listed in
[Command-line options](../reference/command-line.md).

## What works where

| | Windows | Linux, X11 | Linux, Wayland (GNOME, KDE) | macOS |
|---|:---:|:---:|:---:|:---:|
| Interactive prompt, CLI mode | ✅ | ✅ | ✅ | ✅ |
| Text-to-speech | ✅ | ✅ | ✅ | ✅ |
| Global hotkeys | ✅ | ✅ | ✅ (through the desktop) | |
| Showing and hiding the terminal | ✅ | ✅ | | |
| `copy_to_clipboard` | ✅ | ✅ | ✅ | |

- On Wayland the hotkeys come from the desktop's Global Shortcuts portal and need the
  menu entry (`tagent-cli --install-desktop`); they translate the mouse selection, and
  the terminal isn't brought forward. See [Hotkeys: On Wayland](hotkeys.md#on-wayland).
- Without the portal (Sway, Hyprland) or without XWayland (no `DISPLAY`), there are no
  global hotkeys; the prompt and the command line work.
- On macOS, the hotkeys, clipboard and window handling aren't implemented yet.

See [Troubleshooting: Platforms](../troubleshooting/platforms.md).
