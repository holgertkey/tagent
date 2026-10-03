# Platforms

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
