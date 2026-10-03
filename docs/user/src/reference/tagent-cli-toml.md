# tagent-cli.toml

`tagent-cli` keeps its settings in one [TOML](https://toml.io) file, `tagent-cli.toml`:

- Linux, macOS: `~/.config/tagent-cli/tagent-cli.toml`
- Windows: `%APPDATA%\tagent-cli\tagent-cli.toml`

It is created with every setting at its default on first start, each with a comment that
explains it. `tagent-cli --print-default-config` prints such a file without touching
yours, for comparison. `tagent-gui` has its own settings and doesn't read this file.

## Sections

| Section | Settings | Page |
|---------|----------|------|
| `[provider]` | `translate_provider` | [How providers work](../providers/how-providers-work.md) |
| `[translation]` | `source_language`, `target_language` | [Interactive commands](../cli/interactive-commands.md#languages-l) |
| `[dictionary]` | `show_dictionary`, `spell_check`, `dictionary_provider` | [Dictionary and spell check](../cli/dictionary.md) |
| `[interface]` | `show_terminal_on_translate`, `auto_hide_terminal_seconds`, `copy_to_clipboard` | [Hotkeys](../cli/hotkeys.md#what-happens-on-the-translate-hotkey) |
| `[colors]` | seven colors | [Colors](../cli/colors.md) |
| `[history]` | `save_translation_history`, `history_file` | [History](../cli/history.md) |
| `[hotkeys]` | `translate_hotkey` | [Hotkeys](../cli/hotkeys.md) |
| `[speech]` | `enable_text_to_speech`, `speech_hotkey`, `enable_speech_hotkey`, `speech_provider` | [Text-to-speech](../cli/speech.md) |
| `[provider_options.<name>]` | a provider profile, any number of them | [How providers work](../providers/how-providers-work.md#profiles) |

## Languages

`source_language` and `target_language` are language codes (`en`, `ru`, `de`, ...).
`source_language = "auto"` detects the language of each text; the target can't be
`auto`. Names work too, in any case (`"German"`), and are written as codes by `/save`. A
code Tagent doesn't know by name (such as `"uk"` or `"zh-TW"`) is passed to the provider
as it is, with a warning at start. See [Supported languages](languages.md).

The default target is your system language, if Tagent knows it, else English.

## Writing values

```toml
[interface]
copy_to_clipboard = true          # true/false and numbers without quotes
auto_hide_terminal_seconds = 0

[history]
history_file = 'C:\Users\me\history.txt'   # single quotes keep backslashes

[provider_options.work]
type = "google"
timeout_secs = "20"               # profile options are always quoted, numbers too
```

- A missing setting or section takes its default.
- A setting Tagent doesn't know is reported with its line at start, with the likely
  intended name when it looks like a typo or sits in the wrong section. The rest of the
  file still applies.

## Changes apply at once

`tagent-cli` checks the file before each translation and reloads it when it has changed,
so an edit applies to the next translation. Two exceptions: the **hotkeys** need a
restart, and a reload replaces unsaved `/l` and `/p` changes with the file's values.

## Mistakes in the file

A syntax error or a wrong value type (`copy_to_clipboard = "yes"`) is reported with its
line and column:

- at start, `tagent-cli` prints the message and exits;
- while it runs, it prints a warning once and keeps the previous settings until the file
  is fixed.

Deleting the file brings back the defaults on the next start.

## How the app writes it

`tagent-cli` never rewrites the file on its own. It writes only when you ask:

- `/save` changes the languages and the three provider settings, in place, and keeps
  everything else, comments included;
- `--update-config` (or `/config update`) adds settings a newer version introduced; see
  [Upgrading](../troubleshooting/upgrading.md).

On Linux and macOS the file is written readable by you only (`0600`), since it can hold
API keys.

Versions before 0.17.0 used `tagent-cli.conf` (INI). It is no longer read: copy your
settings into `tagent-cli.toml` by hand.
