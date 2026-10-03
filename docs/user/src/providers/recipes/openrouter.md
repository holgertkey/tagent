# Recipe: OpenRouter

[OpenRouter](https://openrouter.ai) gives one API, and one key, for models from many
companies. It bills by tokens, per model.

**Not tested with Tagent yet.** The values below come from OpenRouter's
[quickstart](https://openrouter.ai/docs/quickstart); if they don't work for you, please
[open an issue](https://github.com/holgertkey/tagent/issues).

## 1. Get a key and pick a model

1. Create an API key in your OpenRouter account.
2. Pick a model at [openrouter.ai/models](https://openrouter.ai/models). Its identifier
   names the company and the model, such as `openai/gpt-4o`.

The API is at `https://openrouter.ai/api/v1`.

## 2. Add the profile

### tagent-cli

```toml
[provider]
translate_provider = "openrouter"

[provider_options.openrouter]
type = "openai"
endpoint = "https://openrouter.ai/api/v1"
model = "openai/gpt-4o"
```

and the key in the environment:

```bash
export TAGENT_OPENROUTER_API_KEY="sk-or-..."
```

### tagent-gui

1. **Settings (⚙) > Providers**. Under **New profile**, type `openrouter`, keep the kind
   `openai`, and click **Add**.
2. **Options…** on the `openrouter (openai)` row: `endpoint`, `model`, and the key in
   `api_key`. Click **Test**.
3. **OK**, then pick `openrouter` in **Translation**, and **OK** in the dialog.

## Notes

- Whether `response_format` and `temperature` work depends on the model behind
  OpenRouter. Leave them unset unless you know the model supports them.
- OpenRouter's optional attribution headers (`HTTP-Referer`, `X-OpenRouter-Title`)
  aren't sent; they aren't needed.
