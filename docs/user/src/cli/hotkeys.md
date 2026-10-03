# Hotkeys

In [unified mode](modes.md), `tagent-cli` listens for two global hotkeys, in any
application:

| Hotkey | Default | Setting | What it does |
|--------|---------|---------|--------------|
| Translate | Alt+A | `translate_hotkey` in `[hotkeys]` | Translates the selected text into the terminal |
| Speech | Alt+S | `speech_hotkey` in `[speech]` | Reads the selected text aloud |

They need Windows, or Linux with X11 or XWayland.

## What happens on the translate hotkey

1. Tagent copies the selected text, as if you had pressed Ctrl+C, and reads it from the
   clipboard. The selection stays in the clipboard afterwards, replacing what was there.
   With nothing selected, the clipboard may keep its old content, and Tagent translates
   that; an empty clipboard gives "No selected text or clipboard is empty".
2. With `show_terminal_on_translate = true` (the default), the terminal window comes to
   the front.
3. The translation, or a dictionary entry for a single word, appears in the terminal,
   exactly as at the prompt.
4. With `copy_to_clipboard = true`, the result goes to the clipboard, ready to paste.
   Off by default.
5. If step 2 brought the terminal forward and `auto_hide_terminal_seconds` is above `0`
   (default `3`), the terminal hides again after that many seconds and the window you were in gets the focus back. `0` keeps the
   terminal in front.

These settings are in `[interface]`:

```toml
[interface]
show_terminal_on_translate = true
auto_hide_terminal_seconds = 3
copy_to_clipboard = false
```

## The speech hotkey

Select text and press Alt+S: the text is read aloud in the source language (detected if
the source is `auto`). **Esc** stops it. See [Text-to-speech](speech.md).

`enable_speech_hotkey = false` in `[speech]` turns the speech hotkey off;
`enable_text_to_speech = false` turns speech off entirely.

## Choosing a hotkey

Both hotkeys take the same formats:

| Format | Examples | Notes |
|--------|----------|-------|
| A function key | `F9` | Only `F1`–`F12` work alone, so normal typing isn't caught |
| Modifiers + key | `Alt+Q`, `Ctrl+Shift+T`, `Win+T`, `Alt+Space` | `Shift+<key>` alone isn't allowed: it is how you type capitals |
| A double press | `Ctrl+Ctrl`, `Shift+Shift`, `Alt+Alt`, `F8+F8` | The same key twice, 50–500 ms apart |

```toml
[hotkeys]
translate_hotkey = "Ctrl+Ctrl"

[speech]
speech_hotkey = "F10"
```

- **A hotkey change needs a restart** of `tagent-cli`. Everything else in the file
  applies without one.
- Use two different combinations for the two hotkeys.
- `Ctrl+Alt+Delete` and `Win+L` are refused; `Alt+F4` draws a warning.
- A hotkey Tagent can't read or won't accept is turned off with a warning, and the rest
  of the app keeps working.
- The operating system or another application may take a combination first. On Linux,
  Tagent warns when another application already holds it; pick a different one. A
  double press can't be reserved this way on Linux: Tagent sees it, but the other
  application gets the keys too.
