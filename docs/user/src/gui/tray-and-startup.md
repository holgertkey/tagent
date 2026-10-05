# Tray and startup

## The tray icon

`tagent-gui` lives in the system tray. Its menu has:

- **Show Tagent**: shows the main window (a left click on the icon does the same);
- **Settings…**: opens Settings;
- **Quit**: exits Tagent. This is the only way to quit: the window's close button only
  hides the window.

## Without a tray

On a Linux desktop without a tray (GNOME without an AppIndicator extension, for example)
the icon doesn't appear, and nothing else shows the window or quits Tagent. The global
hotkeys still work, but they don't show the main window. Then:

1. Quit Tagent: `pkill tagent-gui`.
2. In `tagent-gui.json`, set `"start_minimized": false` (see
   [File locations](../reference/file-locations.md)).
3. Start it again: the window opens at start.

Closing the window still hides it, with no tray to bring it back, so quit with
`pkill tagent-gui` and start again. Installing a tray extension (on GNOME,
"AppIndicator and KStatusNotifierItem Support") is the lasting fix.

## At start

| Setting (Settings > Hotkeys & Tray) | Default | Effect |
|-------------------------------------|---------|--------|
| Start minimized to tray | on | Starts with the window hidden; off opens it |
| Remember window size and position | on | Reopens the window where you left it at the next start |

Both apply the next time the window is shown or the app starts. Within one run, the
window always comes back from the tray where you hid it, whatever the second setting says.

Starting `tagent-gui` from a terminal on Linux or macOS doesn't keep the terminal busy:
the app detaches and the prompt returns at once. Its messages go to a log file (see
[File locations](../reference/file-locations.md)). `tagent-gui --foreground` (or `-f`)
keeps it attached, with its messages in the terminal.

**Only one copy runs.** Starting `tagent-gui` again (from the menu, a terminal, or a
second autostart entry) brings up the running copy's window, as "Show Tagent" in the tray
does, and the new start ends there. From a terminal it says so:

```
tagent-gui is already running (pid 12345); showed its window.
```

Each user of a computer has their own copy.

## Starting with the system

Tagent doesn't add itself to autostart. Use your desktop's own setting:

- **Windows:** put a shortcut to `tagent-gui.exe` in the Startup folder (Win+R,
  `shell:startup`).
- **Linux:** add `tagent-gui` in your desktop's startup applications (GNOME Tweaks >
  Startup Applications, KDE System Settings > Autostart), or copy its menu entry:
  `cp /usr/share/applications/io.github.holgertkey.TagentGui.desktop ~/.config/autostart/` (from the `.deb`;
  `~/.local/share/applications/` after `--install-desktop`).

## The menu entry (Linux)

The `.deb` installs a menu entry and an icon. For the downloaded archive or `cargo
install`, run `tagent-gui --install-desktop` once: it adds both for your user, so the
app appears in the menu and GNOME's dock shows its icon. `--uninstall-desktop` removes
them. See [Install](../getting-started/install.md).
