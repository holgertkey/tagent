# Recipe: LM Studio (local)

[LM Studio](https://lmstudio.ai) runs language models on your own computer, with a
graphical model browser. Tagent talks to its
[OpenAI-compatible server](https://lmstudio.ai/docs/developer/openai-compat).

**Not tested with Tagent yet.** The values below come from LM Studio's documentation;
if they don't work for you, please
[open an issue](https://github.com/holgertkey/tagent/issues).

## 1. Download a model and start the server

1. Install LM Studio from [lmstudio.ai](https://lmstudio.ai) and download a chat model in
   it.
2. Start the server: in the app, from the **Developer** tab; or from a terminal with
   `lms server start`. It listens on port `1234`, so the API is at
   `http://localhost:1234/v1`.
3. Note the model's identifier as LM Studio shows it. `curl
   http://localhost:1234/v1/models` lists the identifiers.

## 2. Add the profile

### tagent-cli

```toml
[provider]
translate_provider = "lmstudio"

[provider_options.lmstudio]
type = "openai"
endpoint = "http://localhost:1234/v1"
model = "the-model-identifier"
```

### tagent-gui

1. **Settings (⚙) > Providers**. Under **New profile**, type `lmstudio`, keep the kind
   `openai`, and click **Add**.
2. **Options…** on the `lmstudio (openai)` row: `endpoint` = `http://localhost:1234/v1`,
   `model` = the identifier. Click **Test**.
3. **OK**, then pick `lmstudio` in **Translation**, and **OK** in the dialog.

## The dictionary

To use the same model for dictionary lookups, select the profile as the dictionary
provider too (see [one profile for both jobs](one-profile-two-axes.md)). If you add
`response_format`, use `json_schema`: LM Studio's
[structured output](https://lmstudio.ai/docs/developer/openai-compat/structured-output)
accepts that type, and a `json_object` request would make every lookup fail.
