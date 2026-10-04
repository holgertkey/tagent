# Hotkeys

In [unified mode](modes.md), `tagent-cli` listens for two global hotkeys, in any
application:

| Hotkey | Default | Setting | What it does |
|--------|---------|---------|--------------|
| Translate | Alt+A | `translate_hotkey` in `[hotkeys]` | Translates the selected text into the terminal |
| Speech | Alt+S | `speech_hotkey` in `[speech]` | Reads the selected text aloud |

They work on Windows and on Linux: with X11, and on Wayland desktops such as GNOME, see
[On Wayland](#on-wayland).

## What happens on the translate hotkey

1. Tagent copies the selected text, as if you had pressed Ctrl+C, and reads it from the
   clipboard. The selection stays in the clipboard afterwards, replacing what was there.
   With nothing selected, the clipboard may keep its old content, and Tagent translates
   that; an empty clipboard gives "No selected text or clipboard is empty". (On Wayland
   it reads the text selected with the mouse instead and leaves the clipboard alone.)
2. With `show_terminal_on_translate = true` (the default), the terminal window comes to
   the front (not on Wayland).
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
the source is `auto`). Pressing Alt+S **again** stops it, and so does **Esc** (Windows,
X11) or **Ctrl+C** in the terminal (Linux). See [Text-to-speech](speech.md).

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
- The operating system or another application may take a combination first. On Linux
  with X11, Tagent warns when another application already holds it; pick a different
  one. A double press can't be reserved this way on X11: Tagent sees it, but the other
  application gets the keys too.

## On Wayland

On a Wayland session (the default on Ubuntu, Fedora and other GNOME desktops) an
application can't watch the keyboard of other applications, so the desktop delivers the
hotkeys instead, through its "Global Shortcuts" portal:

- **The menu entry is required once.** The portal accepts only an application it knows
  by its menu entry: run `tagent-cli --install-desktop` (it adds "Tagent CLI" to the
  application menu, opening in a terminal, and its icon) and start `tagent-cli` again.
  Without it, `tagent-cli` says that the hotkeys are off and why.
- **The first start** shows the desktop's dialog with the two shortcuts and the keys from
  your settings. Confirm them, or pick other keys. A double press (`Ctrl+Ctrl`) can't be
  suggested; the dialog then asks for a key. `tagent-cli` then prints what is bound:
  `Hotkeys bound by the desktop: translation Alt+A, speech Alt+S.`
- **Afterwards the desktop owns them.** Change them in the system settings (GNOME:
  Settings > Apps > Tagent CLI); `translate_hotkey` and `speech_hotkey` only suggested
  them the first time.
- **What is translated or spoken** is the text selected with the mouse; nothing is
  copied, and the clipboard stays as it was.
- **The terminal stays where it is**: Wayland doesn't let an application bring another
  one's window forward, so `show_terminal_on_translate` and `auto_hide_terminal_seconds`
  have no effect (a note at start says so when the setting is on). The translation
  appears at the prompt as usual.
- **Esc** doesn't stop speech; press the speech hotkey again, or Ctrl+C in the terminal.
- `tagent-gui` uses the same default keys. If both run, give one of them other keys.
- A desktop without the portal (Sway, Hyprland and other wlroots-based ones) falls back
  to the X11 way, which only sees keys while an X11 (XWayland) window is focused.
