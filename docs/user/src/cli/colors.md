# Colors

The `[colors]` section sets the color of each part of the output:

| Setting | Colors | Default |
|---------|--------|---------|
| `source_prompt_color` | The prompt, `[auto → ru]: `, and the corrected word in a spelling notice | `None` |
| `target_prompt_color` | The provider label, `[google]: ` | `BrightYellow` |
| `dictionary_prompt_color` | The dictionary label, `[Word]: ` | `BrightYellow` |
| `part_of_speech_color` | Parts of speech in a dictionary entry (`Noun`, `Глагол`) | `Cyan` |
| `synonym_color` | Synonyms in a dictionary entry, `[fierce, brutal]` | `Green` |
| `notice_color` | The spelling-correction notice | `Magenta` |
| `error_color` | Errors: translation, speech, clipboard, history | `Red` |

The values: `Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`, their
bright variants (`BrightBlack` … `BrightWhite`), or `None` for the terminal's own color.

```toml
[colors]
target_prompt_color = "BrightCyan"
synonym_color = "None"
```

Colors are used only when the output goes to a terminal. Output to a file or a pipe is
plain text, and so is everything when the environment has `NO_COLOR` set or
`CLICOLOR=0`. The clipboard and the history file always get plain text.
