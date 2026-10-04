# Hotkeys and the popup

`tagent-gui` has two global hotkeys that work in any application, even while its window
is hidden:

| Hotkey | Default | What it does |
|--------|---------|--------------|
| Translate | Alt+A | Translates the selected text into a popup and the transcript |
| Speech | Alt+S | Reads the selected text aloud |

They work on Windows and on Linux (X11, and Wayland desktops such as GNOME, see
[below](#on-wayland)); on macOS they aren't available yet. Change them in Settings >
[Hotkeys & Tray](settings.md#hotkeys--tray). **A hotkey change takes effect after a
restart** (Quit in the tray menu, then start again).

## The translate hotkey

1. Tagent copies the selection, as if you had pressed Ctrl+C. The selection stays in the
   clipboard afterwards. (On Wayland it reads the text selected with the mouse instead
   and leaves the clipboard alone, see [below](#on-wayland).)
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
- On Wayland it can't know where the mouse is: it opens where you last dropped it
  (with "Remember position after dragging" on) or in the top-right corner of the
  screen.

All of this is set on Settings > [Popup](settings.md#popup): whether the popup appears
at all ("Show popup on hotkey"; off, the hotkey only adds to the transcript), what it
shows (the prompt, the phrase), its font, colors, size limits, border and how long it
stays.

## The speech hotkey

Select text and press Alt+S: the text is read aloud in the window's source language
(detected if `Auto`), with no translation. A `[🔊 Speech]:` entry appears in the
transcript; click its prompt to replay it.

Pressing the speech hotkey **again** while something is playing stops it. **Esc** does
too, from any application, on Windows and X11 (on Wayland only while a Tagent window
is focused). Both work only while the hotkeys are active: with an unusable translate
hotkey they are off. Clicking the playing entry's prompt in the transcript always stops
it.

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

On Linux with X11, another application may already hold a combination; Tagent then
writes a warning to its log and that hotkey doesn't work. Pick another one.

## On Wayland

On a Wayland session (the default on Ubuntu, Fedora and other GNOME desktops) an app
can't watch the keyboard of other apps, so the desktop delivers the hotkeys instead
(through the "Global Shortcuts" portal):

- **The first start** shows the desktop's dialog with the two shortcuts and the keys
  from Tagent's settings. Confirm them, or pick other keys there. A double press
  (`Ctrl+Ctrl`) can't be suggested; the dialog then asks for a key.
- **Afterwards the desktop owns them.** Change them in the system settings (GNOME:
  Settings > Apps > Tagent > Global Shortcuts). Settings > Hotkeys & Tray shows the keys
  that are bound; its fields only suggested them the first time.
- **The desktop entry is required.** The portal accepts only an app it knows by its
  menu entry. The `.deb` installs one; otherwise run `tagent-gui --install-desktop`
  once and restart Tagent. Without it the transcript shows a `[Hotkey]` line saying the
  hotkeys are off.
- **What is translated or spoken** is the text selected with the mouse (the "primary
  selection"); nothing is copied and the clipboard stays as it was. Select the text
  first; Ctrl+A in an app doesn't always count as a mouse selection.
- **`tagent-cli` uses the same default keys.** On GNOME, when both run with the same
  keys, the one started first gets them, and the other doesn't respond to them, even
  after the first one quits. Give one of them other keys (for `tagent-gui`: Settings >
  Apps > Tagent > Global Shortcuts), then restart it.
- A desktop without the portal (Sway, Hyprland and other wlroots-based ones) falls back
  to the X11 way, which only sees keys while an X11 (XWayland) window is focused.
