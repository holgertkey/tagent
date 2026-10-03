# DeepL

[DeepL](https://www.deepl.com) is a translation-only provider (kind `deepl`). Use it for
translation and keep the dictionary and speech on `google` (or an
[OpenAI-compatible](openai-compatible.md) dictionary).

## What you need

An API key from your DeepL account (the DeepL API, not a DeepL Translator
subscription): see [DeepL's API plans](https://www.deepl.com/pro-api). Both the Free
and the Pro plan work. A Free key ends in `:fx`, and Tagent sends it to DeepL's Free API
(`https://api-free.deepl.com`) by itself; any other key goes to `https://api.deepl.com`.

## Setup

### tagent-cli

Add the key as an option of the built-in `deepl` and select it:

```toml
[provider]
translate_provider = "deepl"

[provider_options.deepl]
api_key = "your-key:fx"
```

To keep the key out of the file, leave `api_key` out and set the environment variable
`TAGENT_DEEPL_API_KEY` instead (see [API keys and environment variables](api-keys.md)).
For the built-in name `deepl`, the `[provider_options.deepl]` table can then be left out
entirely.

### tagent-gui

1. Open **Settings (⚙) > Providers**.
2. Click **Options…** on the `deepl (built-in)` row, paste the key into `api_key`, and
   click **Test**. A successful test translates a short text, which DeepL bills as a few
   characters.
3. Click **OK**, pick `deepl` in the **Translation** list, and click **OK** in the dialog.

Until the key is set, a ⚠ marks `deepl` wherever it is selected.

## Two DeepL accounts

A profile of your own with `type = "deepl"` is a second, independent DeepL setup, with
its own key and its own environment variable:

```toml
[provider_options.deepl-work]
type = "deepl"
api_key = ""   # or set TAGENT_DEEPL_WORK_API_KEY
```

In `tagent-gui`, add it under **New profile** with the kind `deepl`.

## Options

| Option | Default | Meaning |
|--------|---------|---------|
| `api_key` | — (required) | DeepL authentication key; a Free key ends in `:fx` |
| `endpoint` | chosen by the key | API base URL: `https://api-free.deepl.com` or `https://api.deepl.com` |
| `timeout_secs` | `10` | Time budget for one call, in whole seconds, retries included |
| `max_retries` | `2` | How often a failed request is retried; `0` disables retries |

`endpoint` is needed only to send requests somewhere other than what the key implies.

## Languages

Tagent converts its language codes into DeepL's:

- A source language is sent as its primary code (`pt-BR` → `PT`); `auto` lets DeepL
  detect it.
- A target language keeps its region where DeepL distinguishes one: `pt-BR` → `PT-BR`,
  `en-GB` → `EN-GB`. A bare `en` or `pt` lets DeepL pick the variant.
- Chinese goes by script: `zh`, `zh-CN`, `zh-SG` → Simplified; `zh-TW`, `zh-HK`, `zh-MO`
  → Traditional.

DeepL supports fewer languages than Google. A language it doesn't know is reported as an
error from DeepL.

## Costs and errors

- DeepL bills by characters translated. Speaking text whose source language is `auto`
  also asks DeepL to detect the language, which bills up to 100 characters.
- A wrong or revoked key shows an authentication error; a used-up monthly allowance
  shows a quota error. Neither is retried. DeepL's "too many requests" is retried after
  a short pause.
