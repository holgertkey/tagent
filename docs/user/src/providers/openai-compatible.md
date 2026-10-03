# OpenAI-compatible

The `openai` provider kind talks to any server with an OpenAI-style chat-completions API:
a local model in [Ollama](recipes/ollama.md) or [LM Studio](recipes/lm-studio.md), or a
cloud service such as [OpenAI](recipes/openai.md) or [OpenRouter](recipes/openrouter.md).
It can do two jobs, translation and the dictionary; it can't speak.

A language model translates by following an instruction (a *prompt*) that Tagent sends
with your text. It can translate in a style you describe, and a local model keeps your
text on your computer. It is also slower than Google or DeepL, and a small model makes
more mistakes.

## What you need

- **`endpoint`**: the server's base URL, **including `/v1`**, such as
  `http://localhost:11434/v1`. There is no default, so your text only goes where you
  point it.
- **`model`**: the model's name as the server knows it, such as `qwen3:8b`.
- **`api_key`**: only for a cloud service. A local server needs none.

The recipes have these values for each server.

## Setup

A profile of your own, named after the server, keeps several servers apart. With a local
Ollama:

### tagent-cli

```toml
[provider]
translate_provider = "ollama"

[provider_options.ollama]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen3:8b"
```

A newly created `tagent-cli.toml` ends with this example, commented out, after the
example for `[provider_options.openai]`.

### tagent-gui

1. Open **Settings (⚙) > Providers**. Under **New profile**, type `ollama`, keep the
   kind `openai`, and click **Add**.
2. Click **Options…** on the `ollama (openai)` row, fill in `endpoint` and `model`, and
   click **Test**. The test translates a short text and looks up a word, so you see both
   jobs work.
3. Click **OK**, pick `ollama` in the **Translation** list (and in **Dictionary**, if you
   want it there too), and click **OK** in the dialog.

## How the answer is cleaned up

A model doesn't always answer with the translation alone, so Tagent removes:

- reasoning output at the start of the answer (`<think>…</think>`), from models that
  think out loud;
- a code block around the answer;
- quotation marks around the whole answer, unless your text had them.

An answer the model cut off (it ran out of output length) or refused is an error, never a
partial translation.

## The dictionary

The same profile can look up single words. Select it as the dictionary provider:

- **tagent-cli:** `dictionary_provider = "ollama"` in `[dictionary]`.
- **tagent-gui:** pick it in Settings > Providers > **Dictionary**.

The model is asked for a dictionary entry as JSON: parts of speech, translations with
synonyms, and a spelling correction if the word was misspelled. Tagent keeps at most 6
parts of speech, 8 translations each and 4 synonyms per translation.

With the profile on both jobs, a single word costs two requests to the model, made at the
same time, and the result waits for the slower one. A failed lookup quietly falls back to
the plain translation. If you never see dictionary entries, use **Test** in `tagent-gui`
to see the lookup's error; see also
[Troubleshooting: Providers](../troubleshooting/providers.md).

Small models sometimes drift from the requested JSON. Two things help:

- **`response_format`**: `"json_schema"` (or `"json_object"`) asks the server to return
  structured output. It isn't sent unless set, since not every server accepts it, and a
  server that rejects it makes every lookup fail. LM Studio accepts only `json_schema`.
- **A bigger model.** A 3-billion-parameter model works for common words; a larger one
  gives fuller entries.

## Prompts

Tagent sends a built-in system prompt that names both languages. Two options replace it:

- **`translate_prompt`**: the prompt for translations. `{from}` and `{to}` become language
  names (`{from}` becomes "its original language (detect it)" when the source is `auto`).
- **`dictionary_prompt`**: the prompt for dictionary lookups. It must keep asking for the
  same JSON answer shape, or every lookup fails.

To see the built-in prompts, look at the `[provider_options.openai]` example at the end
of a new `tagent-cli.toml` (or `tagent-cli --print-default-config`), or open **Options…**
in `tagent-gui`, which shows them in the prompt fields. Start from the built-in prompt and
change what you need:

```toml
[provider_options.ollama-formal]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen3:8b"
translate_prompt = """
Translate the text in the user message from {from} into {to} in a formal register.
Output only the translation.
"""
```

Several profiles of one server with different prompts or models can coexist; switch
between them with `/p` in `tagent-cli` or the provider menu in `tagent-gui`. In
`tagent-gui`, **Reset to default** under a prompt field brings back the built-in one, and
a ⚠ warns when your prompt lacks `{to}`.

A config file created by an older `tagent-cli` may lack `#dictionary_prompt` and
`#response_format` in its `openai` example: `--update-config` doesn't change existing
examples. `--print-default-config` shows the current one.

## Options

| Option | Default | Meaning |
|--------|---------|---------|
| `endpoint` | — (required) | API base URL including `/v1` |
| `model` | — (required) | Model name |
| `api_key` | none | API key, sent as a Bearer token; not needed for a local server |
| `temperature` | the model's | Sampling temperature from 0 to 2; not sent unless set |
| `translate_prompt` | built in | System prompt for translations |
| `dictionary_prompt` | built in | System prompt for dictionary lookups |
| `response_format` | not sent | `json_schema` or `json_object`, for dictionary answers |
| `timeout_secs` | `60` | Time budget for one call, in whole seconds, retries included |
| `max_retries` | `1` | How often a failed request is retried; `0` disables retries |

Some models, reasoning models in particular, accept only their default temperature;
leave `temperature` unset for them.

## Speed and costs

- **A local model's first request is slow**: the server loads the model into memory
  first. Later requests are faster. If the first one times out, raise `timeout_secs`.
- **Reasoning models** (Qwen3 and others that write `<think>`) think before every answer,
  which can take many seconds even for one word. A non-reasoning model is faster for
  everyday translation.
- **Speaking `auto`-source text** asks the model to detect the language first: one more
  short request.
- **A cloud service bills by tokens**, for your text, the answer and the prompt each time.
- **Errors:** a wrong key shows an authentication error; a spent balance or budget a quota
  error, never retried; "too many requests" is retried after a short pause.
