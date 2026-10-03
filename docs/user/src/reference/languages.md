# Supported languages

Tagent knows these languages by name. Use the code or the name, in any case, wherever a
language is asked for: `source_language`/`target_language` in `tagent-cli.toml`, `/l` and
`-l` in `tagent-cli`, and the language lists of `tagent-gui`. The source language can
also be `auto`, which detects it.

{{#include generated/languages.md}}

## Other languages

The providers support many more. In `tagent-cli`, any other code (`uk`, `zh-TW`,
`pt-BR`) is passed to the provider as it is, with a warning; whether it works depends on
the provider. `tagent-gui`'s lists offer only the languages above.

Part-of-speech names in a dictionary entry are translated into Russian, Spanish, French,
German, Italian, Portuguese and Chinese, and shown in English for any other target.

Speech reads every language Google's text-to-speech supports.
