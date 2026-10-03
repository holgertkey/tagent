# Interactive commands

At the prompt of [unified mode](modes.md), anything you type is translated, and a line
starting with `/` is a command. The prompt shows the language pair; each translation is
labeled with the provider that made it:

```
[auto → ru]: How are you?
[google]: Как вы?
```

The arrow keys edit the line, Ctrl+R searches earlier input (kept across sessions), and
Tab completes commands and, after `/p`, provider names. An empty line does nothing.

## Commands

| Command | What it does |
|---------|--------------|
| `/h`, `/help`, `/?` | Show the help |
| `/c`, `/config` | Show the current settings, as they would appear in `tagent-cli.toml`, with every provider profile (keys masked) |
| `/config update` | Add the settings your configuration file lacks; see [Upgrading](../troubleshooting/upgrading.md) |
| `/v`, `/version` | Show the version |
| `/l`, `/lang` | Swap the source and target languages |
| `/l <target>` | Set the target language; the source becomes `auto` |
| `/l <source> <target>` | Set both languages |
| `/p`, `/provider` | List the providers of all three jobs, numbered |
| `/p <number>` | Switch to that entry of the list |
| `/p <name>` | Switch the translation provider |
| `/p t\|d\|s <name>` | Switch the translation, dictionary or speech provider |
| `/save` | Save the languages and the providers to the configuration file |
| `/s <text>`, `/speech <text>` | Read the text aloud |
| `/s`, `/speech` | Read the last translated phrase aloud |
| `/ss` | Read the translation of the last phrase aloud |
| `/clear`, `/cls` | Clear the screen |
| `/q`, `/quit`, `/e`, `/exit` | Quit |

## Languages: `/l`

Languages can be given as names (`German`, any case) or codes (`de`):

```
[auto → ru]: /l de
Languages set: Auto (auto) -> German (de)

[auto → de]: /l en ru
Languages set: English (en) -> Russian (ru)

[en → ru]: /l
Languages swapped: Russian (ru) -> English (en)
```

Swapping with the source on `auto` makes English the new target, since `auto` can't be a
target. A code Tagent doesn't list by name (such as `uk`) is passed to the provider as
it is, with a warning. See [Supported languages](../reference/languages.md).

## Providers: `/p`

`/p` lists every provider and profile you can use, by job, with `*` on the ones in use
and the missing options of those that aren't ready:

```
Providers:
 Translation
 * 1  google  Google Translate
   2  deepl   DeepL (missing: api_key)
   3  openai  OpenAI-compatible (missing: endpoint, model)
   4  ollama  OpenAI-compatible (ollama)
 Dictionary
 * 5  google  Google Dictionary
   6  openai  OpenAI-compatible (missing: endpoint, model)
   7  ollama  OpenAI-compatible (ollama)
 Speech
 * 8  google  Google TTS
Switch with /p <number>, /p <name> (translation) or /p t|d|s <name>; /save keeps the choice.
```

Then:

- `/p 7` switches to entry 7, here the dictionary to `ollama`;
- `/p deepl` switches translation to `deepl`;
- `/p d ollama` switches the dictionary (`t` or `translation`, `d`, `dict` or
  `dictionary`, `s` or `speech` name the job).

A switch first checks that the provider can be built; one that lacks a required option
is refused with the reason, and the previous one stays. A job that is turned off (the
dictionary with `show_dictionary = false`, speech with `enable_text_to_speech = false`)
is marked as off in the list and can still be switched.

Providers and profiles are explained in [How providers work](../providers/how-providers-work.md).

## Keeping changes: `/save`

`/l` and `/p` last until you quit. `/save` writes them to `tagent-cli.toml`:
`source_language`, `target_language`, `translate_provider`, `dictionary_provider` and
`speech_provider`. It changes only these values, in place: your comments, the order of
the settings and everything else in the file stay as they are. A provider setting the
file doesn't have is added only if you switched away from the default.

If you edit the file while `tagent-cli` runs, the file wins: the app reloads it, and
unsaved `/l` and `/p` changes are lost.

## Reading aloud: `/s` and `/ss`

```
[auto → ru]: Good morning
[google]: Доброе утро
[auto → ru]: /s
```

reads "Good morning" again, and `/ss` reads "Доброе утро". For a single word, `/ss` reads
its main translation, not the whole dictionary entry. Both also replay a phrase you
translated with the hotkey, in the languages it was translated with. See
[Text-to-speech](speech.md).
