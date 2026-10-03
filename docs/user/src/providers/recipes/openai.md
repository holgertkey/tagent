# Recipe: OpenAI

The [OpenAI API](https://platform.openai.com) runs OpenAI's models in the cloud. It needs
an account with API billing, and it bills by tokens: your text, the answer and Tagent's
prompt, on every request.

**Not tested with Tagent yet.** The values below come from OpenAI's documentation; if
they don't work for you, please
[open an issue](https://github.com/holgertkey/tagent/issues).

## 1. Get a key and pick a model

1. Create an API key in your OpenAI account's API settings.
2. Pick a model from [OpenAI's model list](https://developers.openai.com/api/docs/models).
   A small, inexpensive one is enough for translation, for example `gpt-5.4-mini`.
   Model names change often; use one the list currently shows.

The API is at `https://api.openai.com/v1`.

## 2. Add the profile

### tagent-cli

```toml
[provider]
translate_provider = "openai"

[provider_options.openai]
endpoint = "https://api.openai.com/v1"
model = "gpt-5.4-mini"
```

and set the key in the environment rather than in the file:

```bash
export TAGENT_OPENAI_API_KEY="sk-..."
```

(`api_key = "sk-..."` in the table works too; see
[API keys and environment variables](../api-keys.md).)

Here the built-in name `openai` is the profile, so no `type` is needed. To keep
OpenAI next to a local model, give each its own profile instead (`type = "openai"`).

### tagent-gui

1. **Settings (⚙) > Providers > Options…** on the `openai (built-in)` row: `endpoint`,
   `model`, and the key in `api_key`. Click **Test**: it translates a short text and
   looks up a word, which costs a few tokens.
2. **OK**, then pick `openai` in **Translation**, and **OK** in the dialog.

## Notes

- `temperature` isn't sent unless you set it. If the server rejects it for your model,
  leave it unset.
- OpenAI supports `response_format = "json_schema"` for
  [structured outputs](https://developers.openai.com/api/docs/guides/structured-outputs),
  which keeps dictionary answers in shape.
- A spent balance or budget shows a quota error, which Tagent doesn't retry.
