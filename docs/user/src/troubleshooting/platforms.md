# Platforms

What works where, in short:

| | Windows | Linux, X11 or XWayland | Linux, pure Wayland | macOS |
|---|:---:|:---:|:---:|:---:|
| Translation, dictionary, speech | ✅ | ✅ | ✅ | ✅ |
| Global hotkeys, the `tagent-gui` popup | ✅ | ✅ | | |
| Clipboard features | ✅ | ✅ | | |
| `tagent-cli`: showing and hiding the terminal | ✅ | ✅ | | |
| `tagent-gui`: tray icon | ✅ | ✅ (needs a tray) | ✅ (needs a tray) | ✅ |

**Wayland:** most Wayland desktops (GNOME, KDE) also run XWayland, and with it
everything works as on X11. On a session without it (no `DISPLAY` variable), Wayland
doesn't let applications watch global keys, copy another application's selection, or
manage its windows.

**macOS:** the global hotkeys and the clipboard features aren't implemented yet.

## tagent-cli

### The hotkey does nothing

- **Linux on pure Wayland, and macOS:** global hotkeys aren't available; `tagent-cli`
  says so at start. Use the prompt or the command line. Most Wayland desktops also run
  XWayland, and there the hotkeys work.
- **Another application holds the combination.** On Linux, `tagent-cli` warns at start
  when it can't reserve the hotkey. Pick a different one in `tagent-cli.toml`.
- **You changed the hotkey without restarting.** Hotkeys are read at start only.
- **The hotkey was rejected.** A malformed or disallowed hotkey (such as `Shift+A`) is
  turned off with a warning at start; see [Hotkeys](../cli/hotkeys.md#choosing-a-hotkey).
- **Windows:** some applications running as administrator don't pass keys to programs
  that aren't; run `tagent-cli` as administrator too.

### "No selected text or clipboard is empty"

The hotkey found nothing to translate. Select the text again and press the hotkey while
the application with the selection has the focus. Some applications don't copy on Ctrl+C
(terminals often use Ctrl+Shift+C); copy the text yourself and paste it at the prompt.

### The hotkey translates something I didn't select

The hotkey copies the selection to the clipboard and translates the clipboard. If the
copy didn't happen (nothing selected, or the application ignores Ctrl+C), the clipboard
still holds what you copied earlier, and that gets translated.

### Speech plays, but Esc doesn't stop it

Esc works on Windows, and on Linux only in unified mode with X11 or XWayland. It
doesn't work on macOS or pure Wayland. A one-shot `tagent-cli -s` stops with Ctrl+C.

### No sound

Check that your audio device works and isn't muted, and that `enable_text_to_speech` is
`true`. Speech needs a network connection with the default `google` provider.

### It exits at start with "invalid configuration file"

```
invalid configuration file .../tagent-cli.toml:
TOML parse error at line 2, column 21
  |
2 | copy_to_clipboard = "yes"
  |                     ^^^^^
invalid type: string "yes", expected a boolean
```

Fix the value at the line and column shown: `true`/`false` and numbers go without quotes,
other values in quotes, and profile options always in quotes. While `tagent-cli` runs, the
same mistake prints a warning once and keeps the previous settings. Deleting the file
brings back the defaults. See [tagent-cli.toml](../reference/tagent-cli-toml.md).

A warning such as `unknown section [translaton] (did you mean [translation]?)` isn't
fatal: that part of the file is ignored until you fix the name.

## tagent-gui

`tagent-gui` writes its messages (warnings, why a hotkey is off, speech errors) to a log
file when started from a terminal on Linux or macOS, and to the terminal with
`--foreground`. See [File locations](../reference/file-locations.md).

### No tray icon, no window

Some Linux desktops have no tray: GNOME needs an extension for it. Since `tagent-gui`
starts in the tray, nothing appears. See
[Tray and startup: Without a tray](../gui/tray-and-startup.md#without-a-tray).

### The hotkeys do nothing

- **Pure Wayland, macOS:** not available; use the window.
- **A restart is needed** after changing a hotkey: Quit from the tray menu and start
  again.
- **An unusable translate hotkey turns off everything global**: both hotkeys and Esc. The
  log says `Global hotkeys disabled.` with the reason; Settings > Hotkeys & Tray shows the
  error under the field.
- **Another application holds the combination** (Linux): the log has a warning. Pick
  another one.
- **Windows:** applications running as administrator don't pass keys to `tagent-gui`
  unless it runs as administrator too.

### The hotkey translates something I didn't select

As in `tagent-cli`: the hotkey copies the selection, then translates the clipboard. If
the copy didn't happen, the previous clipboard content gets translated.

### Copying does nothing (macOS)

Right-click copying and the 📋 button need the clipboard, which isn't implemented on
macOS yet.

### The window flashes light, then turns dark (Linux)

With the theme on `Auto`, a newly shown window can be light for a moment before it
follows the system's dark setting. Pick `Light` or `Dark` in Settings > View to avoid it.

### A setting I edited in tagent-gui.json is ignored

A file that isn't valid JSON (a missing comma, a trailing comma) is ignored as a whole
until fixed, and the log says why. Hotkeys and `start_minimized` are read only at start.

