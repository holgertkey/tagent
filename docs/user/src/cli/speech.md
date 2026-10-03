# Text-to-speech

`tagent-cli` can read text aloud, with Google's text-to-speech by default.

| Where | How |
|-------|-----|
| Any application | Select text, press the speech hotkey (Alt+S); see [Hotkeys](hotkeys.md) |
| The prompt | `/s <text>`; `/s` alone repeats the last phrase, `/ss` its translation |
| The command line | `tagent-cli -s "Hello world"` |

**Esc** stops playback on Windows, and on Linux in unified mode with X11 or XWayland,
where `tagent-cli` watches the keyboard. On macOS, on pure Wayland, and in a one-shot
`tagent-cli -s` on Linux, Esc does nothing: wait for the end, or press Ctrl+C to stop a
one-shot `tagent-cli -s`.

## Which language

- Text from `/s <text>`, `-s` and the speech hotkey is read in the **source language**.
  If it is `auto`, the translation provider detects the language first.
- `/s` and `/ss` without text use the languages of the last translation as they were
  when it was made: `/s` reads the phrase in its source language, `/ss` the translation
  in its target language.

Long text is read in pieces, one after another, without a long wait at the start.

## Settings

```toml
[speech]
enable_text_to_speech = true   # false turns speech off everywhere
speech_hotkey = "Alt+S"        # restart required
enable_speech_hotkey = true
speech_provider = "google"
```

`speech_provider` is independent of the translation provider. Today `google` is the only
speech provider; see [How providers work](../providers/how-providers-work.md).
