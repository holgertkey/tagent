# Commands in the input box

A few of [`tagent-cli`'s interactive commands](../cli/interactive-commands.md) also work
in the main window's input box: type one and press **Enter**. The command and its answer
appear in the transcript as a `[cmd]:` entry; an error is shown in the error color.

| Command | What it does | Same as |
|---------|--------------|---------|
| `/l`, `/lang` | Swap the source and target languages | **⇄** |
| `/l <target>` | Set the target language; the source becomes `Auto` | the language lists |
| `/l <source> <target>` | Set both languages | the language lists |
| `/p`, `/provider` | List the providers of all three jobs, numbered, the ones in use marked `*` | the provider menu |
| `/p <number>` | Switch to that entry of the list | a pick in the menu |
| `/p <name>` | Switch the translation provider | a pick in the menu |
| `/p t\|d\|s <name>` | Switch the translation, dictionary or speech provider | a pick in the menu |
| `/s`, `/speech` | Read the phrase of the last entry aloud | its phrase prompt |
| `/s <text>` | Read the text aloud, in the source language (detected with `Auto`) | the speech hotkey |
| `/ss` | Read the last translation aloud (for a dictionary entry, its main translation) | its translation prompt |
| `/clear`, `/cls` | Empty the transcript | |
| `/help`, `/h`, `/?` | List the commands | |
| `/version`, `/v` | Show the version of `tagent-gui` and of the `tagent` library | |
| `/quit`, `/q` | Hide the window to the tray; the hotkeys keep working | the window's close button |
| `/exit`, `/e` | Quit `tagent-gui` | **Quit** in the tray menu |

## How commands behave

- **Changes last for this run**, like a choice in the window's lists or provider menu:
  nothing is saved, and Settings keeps your defaults. A `/p` choice is marked
  `(this session)` in the header; `/p` with the default ends it.
- **Languages** are names (`German`, any case) or codes (`de`) from the
  [supported languages](../reference/languages.md). An unknown language changes
  nothing. The target can't be `Auto`: `/l auto`, and `/l` while the source is `Auto`,
  use English as the target and say so.
- **Speaking** works like the speak buttons: one entry at a time, and `/s` or `/ss`
  while something is being read stops it. With text-to-speech off (Settings >
  General), they say so.
- **The box is emptied** after a command, as after a translation. A command that
  couldn't be carried out (a typo in a language or provider name) stays in the box, so
  you can fix it.
- **Only these commands are commands.** Anything else starting with `/`, such as
  `/usr/bin` or `/xyz`, is translated like any text. Command names are lowercase: `/L`
  is translated too.
- **`/q` doesn't quit**, unlike in `tagent-cli`: it hides the window, as closing it does,
  so the hotkeys keep working. Bring the window back with the tray icon or by starting
  `tagent-gui` again. `/exit` (`/e`) quits.
- **To translate a command's name itself**, start with `//`: `//l` translates `/l`.
- Only the input box reads commands. A selection translated with the hotkey is always
  translated: a selected `/l en` is translated, not run.

There is no Tab completion; `/help` lists the commands.
