# Hotkeys and the popup

`tagent-gui` has two global hotkeys that work in any application, even while its window
is hidden:

| Hotkey | Default | What it does |
|--------|---------|--------------|
| Translate | Alt+A | Translates the selected text into a popup and the transcript |
| Speech | Alt+S | Reads the selected text aloud |

They need Windows, or Linux with X11 or XWayland; on macOS they aren't available yet.
Change them in Settings > [Hotkeys & Tray](settings.md#hotkeys--tray). **A hotkey
change takes effect after a restart** (Quit in the tray menu, then start again).

## The translate hotkey

1. Tagent copies the selection, as if you had pressed Ctrl+C. The selection stays in the
   clipboard afterwards.
2. It translates the text with the window's current languages and providers. A single
   word gets a dictionary entry.
3. The result appears in a **popup** next to the mouse cursor, and as a new entry in the
   transcript.

## The popup

- It **hides itself** after a few seconds (3 by default), but stays while the mouse
  rests on it.
- **Right-click** the phrase or the translation to copy that line. Its border flashes to
  confirm.
- **Drag** it with the left mouse button to move it. With "Remember position after
  dragging" on, later popups appear where you dropped this one instead of next to the
  cursor.
- When it hides, the application you were in gets the keyboard focus back.

All of this is set on Settings > [Popup](settings.md#popup): whether the popup appears
at all ("Show popup on hotkey"; off, the hotkey only adds to the transcript), what it
shows (the prompt, the phrase), its font, colors, size limits, border and how long it
stays.

## The speech hotkey

Select text and press Alt+S: the text is read aloud in the window's source language
(detected if `Auto`), with no translation. A `[Speech]:` entry with its own 🔊 button
appears in the transcript, so you can replay it.

**Esc** stops whatever is playing, from any application. It works wherever the hotkeys
do, and only while they are active: with an unusable translate hotkey, Esc is off too.
The ⏹ button in the transcript always works.

## Choosing a hotkey

| Format | Examples | Notes |
|--------|----------|-------|
| A function key | `F9` | Only `F1`–`F12` work alone |
| Modifiers + key | `Alt+Q`, `Ctrl+Shift+T`, `Win+T` | `Shift+<key>` alone isn't allowed |
| A double press | `Ctrl+Ctrl`, `Shift+Shift`, `F8+F8` | The same key twice in quick succession |

In Settings, type the hotkey, or click **Record** and press it. A hotkey that can't be
used is shown as an error there. In the file, an unusable speech hotkey turns off only
itself, but an unusable translate hotkey turns off all global keys: both hotkeys and
Esc. The app still starts, and the log says why.

On Linux, another application may already hold a combination; Tagent then writes a
warning to its log and that hotkey doesn't work. Pick another one.
