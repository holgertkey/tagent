# How providers work

A *provider* is the service that does the work behind Tagent: Google Translate, DeepL, or
a language model behind an OpenAI-compatible API (a local Ollama or LM Studio, OpenAI,
OpenRouter, ...). Both applications, `tagent-cli` and `tagent-gui`, use the same
providers and the same settings for them. Only where the settings are stored differs.

## Three independent jobs

Tagent splits its work into three jobs, and each one has its own provider setting:

| Job | What it does | `tagent-cli.toml` | `tagent-gui` |
|-----|--------------|-------------------|--------------|
| Translation | Translates text; also detects the source language when it is `auto` | `translate_provider` in `[provider]` | Settings > Providers > Translation |
| Dictionary | Looks up a single word: parts of speech, translations, synonyms, spelling correction | `dictionary_provider` in `[dictionary]` | Settings > Providers > Dictionary |
| Speech | Reads text aloud | `speech_provider` in `[speech]` | Settings > Providers > Speech |

Any combination works: DeepL for translation, Google for the dictionary and Google for
speech, for example. Not every provider can do every job:

| Provider kind | Translation | Dictionary | Speech | Needs setup |
|---------------|:-----------:|:----------:|:------:|-------------|
| [`google`](google.md) | ✅ | ✅ | ✅ | No |
| [`deepl`](deepl.md) | ✅ | | | An API key |
| [`openai`](openai-compatible.md) (OpenAI-compatible) | ✅ | ✅ | | A server address and a model |

All three settings default to `google`, which needs no account and no key.

### A single word uses two providers

When the dictionary is on (`show_dictionary`, on by default in both apps) and you
translate a single word, Tagent asks the translation provider and the dictionary
provider at the same time. If the dictionary has an entry, you see the dictionary
article, headed by the translation provider's translation. If it has none, or the lookup
fails, you see the plain translation. A failed lookup shows no error message; see
[Troubleshooting: Providers](../troubleshooting/providers.md).

### Speaking text in an unknown language

Speech needs to know the language of the text. When the source language is `auto`,
Tagent first asks the *translation* provider to detect it, then sends the text to the
speech provider. With a paid translation provider this detection costs a little: DeepL
bills up to 100 characters, a language model one short request.

## Profiles

A setting such as `translate_provider` takes a **profile name**. A profile is a named set
of options for one provider kind:

- **A built-in name works as it is.** `google`, `deepl` and `openai` are profile names
  too. `google` needs no options at all; `deepl` and `openai` need some (an API key, a
  server address), which you add as options of that name.
- **A profile of your own** has any name made of `a`–`z`, `0`–`9`, `_` and `-`, and a
  `type` option that says which kind it is. Several profiles of one kind can coexist: two
  DeepL accounts, or one local model for translation and another for the dictionary.
- **One profile can serve several jobs.** An `openai` profile can be both the translation
  and the dictionary provider; see
  [Recipe: one profile for translation and the dictionary](recipes/one-profile-two-axes.md).

### In tagent-cli

Profiles are `[provider_options.<name>]` tables in `tagent-cli.toml`. Every value is a
quoted string, numbers too:

```toml
[provider]
translate_provider = "work"

# A profile of your own: a second Google setup with a longer time budget
[provider_options.work]
type = "google"
timeout_secs = "20"

# Options for the built-in name itself
[provider_options.google]
max_retries = "0"
```

A newly created `tagent-cli.toml` ends with a commented-out example profile for every
provider kind. To use one, remove the leading `# ` from its lines and fill in the empty
values. `tagent-cli --print-default-config` prints these examples for a file that
predates them.

`/config` at the interactive prompt (or `tagent-cli --config`) lists every profile with
its effective options, secrets masked.

### In tagent-gui

Profiles live in `tagent-gui.json` under `provider_options`, and you manage them in
**Settings (⚙) > Providers**:

1. Under **New profile**, type a name, pick the kind, and click **Add**. The kind list
   starts at `openai`.
2. Click **Options…** on the new row. The panel shows one field per option of that kind;
   required ones are marked `*`. Fill them in.
3. Click **Test** to make one real call per job the kind can do, with the values in the
   panel. A test of a paid service costs a few characters or tokens.
4. Click **OK** to close the panel, pick the profile in the Translation, Dictionary or
   Speech list at the top of the tab, and click **OK** in the dialog. Nothing is saved
   before that last OK.

A ⚠ next to a list means that the profile it selects lacks a required option. The
checkbox on each row ("Show in lists") hides a profile from the lists without deleting
it. **Delete** removes a profile of your own; a list that selected it falls back to
`google`.

The same profile in `tagent-gui.json`, if you prefer to edit the file:

```json
{
  "translate_provider": "work",
  "provider_options": {
    "work": { "type": "google", "timeout_secs": "20" }
  }
}
```

## Saved defaults and session choices

Both apps let you switch providers on the fly without touching the saved setting:

- **tagent-cli:** `/p` at the interactive prompt lists the providers of all three jobs,
  numbered, with the ones in use marked `*`. `/p 3` picks entry 3, `/p deepl` switches
  translation, `/p d ollama` switches the dictionary (`t`, `d`, `s` name the job). The
  switch lasts until you quit; `/save` writes it to the file. See
  [Interactive commands](../cli/interactive-commands.md).
- **tagent-gui:** the button next to ⚙ in the main window names the translation provider
  in use (`google ▾`). Its menu has a section for each job; a pick holds for this run only
  and is marked `(this session)` in the window header. Settings > Providers still shows,
  and saves, the defaults.

Both apps reload their configuration file when it changes, so an edit to a provider
setting applies to the next translation without a restart. In `tagent-cli`, a reload
replaces unsaved `/p` choices with the file's values.

## Time limits and retries

Every provider kind accepts two general options:

| Option | Meaning |
|--------|---------|
| `timeout_secs` | Time budget for one call, in whole seconds, retries included |
| `max_retries` | How often a failed request is retried; `0` disables retries |

The defaults differ by kind (Google: 10 seconds, 1 retry; DeepL: 10 seconds, 2 retries;
OpenAI-compatible: 60 seconds, 1 retry). A retry happens only after a network failure or
a temporary server error (HTTP 502, 503, 504), and, for DeepL and OpenAI-compatible
servers, after a short "too many requests" pause the server asks for. Wrong keys and
exhausted quotas are never retried.

All options of every kind are listed in [Provider options](../reference/provider-options.md).
