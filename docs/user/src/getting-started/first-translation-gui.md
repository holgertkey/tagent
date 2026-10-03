# First translation (tagent-gui)

`tagent-gui` needs no setup: it translates with Google out of the box, into your system
language (English if Tagent doesn't know it).

## Start it

Start `tagent-gui` from the application menu (on Linux, after installing the `.deb` or
running `tagent-gui --install-desktop`; see [Install](install.md)), or from a terminal
(the terminal is free again at once; the app runs on its own).

It starts **in the system tray**, without a window: look for its icon in the tray or
the top bar. Click the icon to show the window. To have the window open at start, untick
"Start minimized to tray" in Settings > Hotkeys & Tray.

If you see no tray icon (GNOME, for one, has no tray without an extension), the window
can't be shown: quit Tagent (`pkill tagent-gui`), set `"start_minimized": false` in
`tagent-gui.json` (see [File locations](../reference/file-locations.md)), and start it
again. See [Tray and startup](../gui/tray-and-startup.md#without-a-tray).

## Translate a selection

Select text in any application and press **Alt+A**. The translation appears in a small
popup next to the mouse cursor, and is added to the main window's transcript. The popup
hides itself after a few seconds.

**Alt+S** reads the selected text aloud; **Esc** stops it (Windows, and Linux with X11
or XWayland).

The hotkeys need Windows, or Linux with X11 or XWayland. On macOS, use the window.

## Translate in the window

Pick the languages at the top of the window, type text in the box at the bottom, and
press **Enter** (Shift+Enter starts a new line). A single word gets a dictionary entry
instead of a plain translation. The 🔊 buttons read a result aloud.

![The main window at first start](../images/gui-main-window.png)

## Next steps

- The window in detail: [The main window](../gui/main-window.md).
- Change the hotkeys, the popup and the look: [Settings](../gui/settings.md).
- Translate with DeepL or a local model: [How providers work](../providers/how-providers-work.md).
