# First translation (tagent-cli)

`tagent-cli` needs no setup: it translates with Google out of the box, into your system
language (English if Tagent doesn't know it). On first start it creates its
configuration file, `tagent-cli.toml` (see [File locations](../reference/file-locations.md)).

## From the command line

```bash
tagent-cli "Hello world"
```

prints the translation and exits. A single word shows a dictionary entry instead:

```bash
tagent-cli translate
```

```
переводить
Глагол
  переводить [transfer, translate, convert, move, interpret, put]
  транслировать [translate, transmit, relay, compile]
  ...
```

Pick other languages for one run with `-l`: `tagent-cli -l de "Hello world"` (into
German), `tagent-cli -l en de "Hello world"` (from English into German).

## The interactive prompt and the hotkey

Run it without arguments:

```bash
tagent-cli
```

It prints a short banner (languages, providers, hotkeys, the main commands) and a prompt
with the language pair. Type text and press Enter:

```
[auto → ru]: How are you?
[google]: Как вы?
```

While it runs, **select text in any other application and press Alt+A**: the translation
appears in the terminal. **Alt+S** reads the selected text aloud. The hotkeys need
Windows or Linux; on a Wayland desktop such as GNOME, run `tagent-cli --install-desktop`
once first and confirm the keys in the dialog that appears at the next start (see
[Hotkeys: On Wayland](../cli/hotkeys.md#on-wayland)).

Type `/h` for the list of commands, `/q` to quit.

## Next steps

- Change the languages: `/l en de` at the prompt, then `/save` to keep them. See
  [Interactive commands](../cli/interactive-commands.md).
- Change the hotkey: [Hotkeys](../cli/hotkeys.md).
- Translate with DeepL or a local model: [How providers work](../providers/how-providers-work.md).
