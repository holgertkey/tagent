# History

`tagent-cli` can keep a log of every translation, from the prompt, the hotkey and the
command line alike. It is off by default:

```toml
[history]
save_translation_history = true
history_file = "/home/me/translations.txt"
```

Each translation is appended as one entry:

```
[2026-09-26 14:30:15 UTC] auto -> ru
IN:  How are you?
OUT: Как вы?
---

[2026-09-26 14:32:45 UTC] auto -> ru
IN:  translate
OUT: переводить
Глагол
  переводить [transfer, translate, convert, move, interpret, put]
  транслировать [translate, transmit, relay, compile]
---
```

- The time is in UTC. The languages are the ones set when you translated (`auto` stays
  `auto`).
- A dictionary entry is saved in full, as plain text.
- Errors and text read aloud are not saved.

## Where the file is

By default `translation_history.txt` in the per-user data folder:

- Linux: `~/.local/share/tagent-cli/translation_history.txt`
- macOS: `~/Library/Application Support/tagent-cli/translation_history.txt`
- Windows: `%APPDATA%\tagent-cli\translation_history.txt`

Use an absolute path for `history_file`: a relative one is taken from the folder you
start `tagent-cli` in. Missing folders are created. On Windows, write backslashes doubled
(`"C:\\Users\\me\\history.txt"`) or use single quotes (`'C:\Users\me\history.txt'`).
