# Dictionary and spell check

When you translate a **single word**, `tagent-cli` shows a dictionary entry instead of a
bare translation: the word's translations grouped by part of speech, each with its
synonyms.

```
[auto → ru]: translate
[Word]: переводить
Глагол
  переводить [transfer, translate, convert, move, interpret, put]
  транслировать [translate, transmit, relay, compile]
  преобразовывать [translate, reform, reorganize, transfashion, reorganise]
```

- The first line is the main translation.
- The parts of speech (`Глагол`, "verb") are named in the target language for English,
  Russian, Spanish, French, German, Italian, Portuguese and Chinese, in English
  otherwise.
- The words in brackets are synonyms in the **source** language: other words that have
  this meaning.

A phrase, or a word the dictionary doesn't know, gets a plain translation.

## Spell check

With `spell_check` on, a misspelled word is looked up under its correct spelling, and a
notice in the target language says which word was used:

```
[en → ru]: vialent
Показан перевод слова violent
[Word]: яростный
Прилагательное
  насильственный [violent, forcible]
  ...
```

Both small typos (`violnt`) and heavier ones (`vialent`) are caught, as far as the
dictionary provider can tell.

## Settings

```toml
[dictionary]
show_dictionary = true        # false: always a plain translation
spell_check = true            # false: no correction, no notice
dictionary_provider = "google"
```

The dictionary has its own provider, independent of the translation provider: `google`
(the default) or an [OpenAI-compatible](../providers/openai-compatible.md) profile.
`/p d <name>` switches it for the session. A dictionary provider that can't be used
(a typo in its name, a missing option) never stops translation: `tagent-cli` warns once
and translates words plainly.

The colors of the entry are set in [Colors](colors.md). The clipboard and the history
file always get the entry as plain text.
