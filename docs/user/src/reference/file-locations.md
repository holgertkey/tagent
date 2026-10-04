# File locations

Both applications keep their files in your user's standard folders, so they survive
reinstalling and each user has their own.

## tagent-cli

| File | Linux | macOS | Windows |
|------|-------|-------|---------|
| Settings, `tagent-cli.toml` | `~/.config/tagent-cli/` | `~/Library/Application Support/tagent-cli/` | `%APPDATA%\tagent-cli\` |
| Backup made by `--update-config`, `tagent-cli.toml.bak` | same folder | same folder | same folder |
| The prompt's input history, `interactive_history.txt` | same folder | same folder | same folder |
| History, `translation_history.txt` (when turned on) | `~/.local/share/tagent-cli/` | `~/Library/Application Support/tagent-cli/` | `%APPDATA%\tagent-cli\` |
| Menu entry, after `--install-desktop` | `~/.local/share/applications/io.github.holgertkey.TagentCli.desktop` | — | — |
| Icon, after `--install-desktop` | `~/.local/share/icons/hicolor/512x512/apps/io.github.holgertkey.TagentCli.png` | — | — |

`%APPDATA%` is usually `C:\Users\<you>\AppData\Roaming`. `/h` and `/config` show the
settings file's full path; `history_file` can move the history anywhere.

## tagent-gui

| File | Linux | macOS | Windows |
|------|-------|-------|---------|
| Settings, `tagent-gui.json` | `~/.config/tagent-gui/` | `~/Library/Application Support/tagent-gui/` | `%APPDATA%\tagent-gui\` |
| Log, `tagent-gui.log` | `~/.local/share/tagent-gui/` | `~/Library/Application Support/tagent-gui/` | — |
| Menu entry, after `--install-desktop` | `~/.local/share/applications/io.github.holgertkey.TagentGui.desktop` | — | — |
| Icon, after `--install-desktop` | `~/.local/share/icons/hicolor/512x512/apps/io.github.holgertkey.TagentGui.png` | — | — |
| While running: `io.github.holgertkey.TagentGui.sock`, how a second start finds it | `$XDG_RUNTIME_DIR` (e.g. `/run/user/1000/`) | `~/Library/Application Support/tagent-gui/` | a named pipe, no file |

The log is written when the app was started from a terminal and moved itself to the
background (Linux, macOS); with `--foreground`, messages go to the terminal instead. On
Windows, messages go to the standard error output, visible only when started from a
terminal.

## Moving to another computer

Copy the settings file. Profiles with API keys in them carry the keys along; keys kept in
environment variables have to be set again.
