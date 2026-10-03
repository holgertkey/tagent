# Recipe: one profile for translation and the dictionary

An OpenAI-compatible profile can do two jobs: translation and the dictionary. Select the
same profile for both, and one model translates phrases and looks up single words. Each
job reads its own prompt (`translate_prompt`, `dictionary_prompt`) from the same profile;
everything else (`endpoint`, `model`, `api_key`, `timeout_secs`, ...) is shared.

Speech stays on `google`: language models in Tagent don't speak.

## tagent-cli

```toml
[provider]
translate_provider = "ollama"

[dictionary]
dictionary_provider = "ollama"

[speech]
speech_provider = "google"

[provider_options.ollama]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen3:8b"
response_format = "json_schema"   # optional; helps small models answer in shape
```

## tagent-gui

In **Settings (⚙) > Providers**, pick the profile in both **Translation** and
**Dictionary**, and click **OK**. **Test** in the profile's **Options…** panel tries
both jobs.

## What it costs

A single word now makes two requests to the model at the same time: one translation and
one lookup. The result shows when the slower one finishes, and `timeout_secs` is the
budget for each. A phrase makes one request, as before.

## Mixing

The jobs don't have to share a profile. Two profiles of one server work as well: a
large model for the dictionary, where quality matters, and a fast one for translation.

```toml
[provider]
translate_provider = "fast"

[dictionary]
dictionary_provider = "thorough"

[provider_options.fast]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen2.5:3b"

[provider_options.thorough]
type = "openai"
endpoint = "http://localhost:11434/v1"
model = "qwen3:8b"
```

Or a cloud translator with a local dictionary: `translate_provider = "deepl"` with
`dictionary_provider = "ollama"`.
