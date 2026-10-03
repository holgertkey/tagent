# Recipe: Ollama (local)

[Ollama](https://ollama.com) runs language models on your own computer. Your text never
leaves it, and nothing is billed. Tagent talks to it through Ollama's
[OpenAI-compatible API](https://docs.ollama.com/api/openai-compatibility).

**Tested** with Tagent and the models `qwen3:8b` and `qwen2.5:3b`, for translation and
the dictionary.

## 1. Install Ollama and a model

Install Ollama from [ollama.com/download](https://ollama.com/download), then download a
model:

```bash
ollama pull qwen3:8b
```

Any chat model works. Some guidance:

- `qwen3:8b` translates well, but it is a reasoning model: it thinks before every
  answer, which takes a while on a computer without a strong graphics card.
- `qwen2.5:3b` is small and quick, and fine for common words and short phrases.

`ollama list` shows the models you have, by the names Tagent needs.

Ollama's server listens on `http://localhost:11434`; its OpenAI-compatible API is at
`http://localhost:11434/v1`. The server usually starts with Ollama; if it doesn't,
run `ollama serve`.

## 2. Add the profile

### tagent-cli

In `tagent-cli.toml`:

```toml
[provider]
translate_provider = "ollama"

[dictionary]
dictionary_provider = "ollama"   # optional: the dictionary from the same model

[provider_options.ollama]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen3:8b"
```

The app reloads the file by itself. Try it:

```bash
tagent-cli "Good morning"
```

At the interactive prompt (`tagent-cli` with no arguments), each translation is labeled
with the profile that made it, `[ollama]:`.

### tagent-gui

1. **Settings (⚙) > Providers**. Under **New profile**, type `ollama`, keep the kind
   `openai`, and click **Add**.
2. **Options…** on the `ollama (openai)` row: `endpoint` = `http://localhost:11434/v1`,
   `model` = `qwen3:8b`. Click **Test**.
3. **OK**, then pick `ollama` in **Translation** (and **Dictionary**, if wanted), and
   **OK** in the dialog.

No `api_key` is needed. Ollama ignores it locally.

## 3. If something goes wrong

- **A connection error**: the Ollama server isn't running, or the address is wrong.
  `curl http://localhost:11434/v1/models` should list your models.
- **"model not found"**: the `model` name doesn't match `ollama list` exactly, tag
  included (`qwen3:8b`, not `qwen3`).
- **A timeout on the first translation**: Ollama loads the model into memory on the
  first request. Try again, or raise `timeout_secs` (default `60`).
- **No dictionary entries**: small models sometimes answer in the wrong shape. Add
  `response_format = "json_schema"` to the profile, or use a larger model. In
  `tagent-gui`, **Test** shows the lookup's error.
