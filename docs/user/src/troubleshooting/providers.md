# Providers

Errors look the same in both apps. `tagent-cli` prints them (`Translation failed: ...`);
`tagent-gui` shows them in the transcript in place of the translation (`Error: ...`).
In `tagent-gui`, **Test** in a profile's **Options…** panel (Settings > Providers) tries
the provider with the values in the panel and shows the full error, which is the
quickest way to check a setup.

## Before the first request

### "unknown provider: …"

```
unknown provider: deepel (supported values for translate_provider: google, deepl, openai)
```

The provider setting names neither a built-in provider nor a profile of yours: a typo,
or a profile without its `[provider_options.<name>]` table. `tagent-cli` lists the
valid built-in names. A dictionary or speech provider that is unknown doesn't stop
translation: the dictionary falls back to plain translations, and speech reports the
error when used.

### "invalid provider options: missing required option …"

```
invalid provider options: missing required option `api_key` (check [provider_options.deepl]
in tagent-cli.toml, or the TAGENT_DEEPL_<KEY> environment variables)
```

The profile lacks an option its kind requires: `api_key` for DeepL, `endpoint` and `model`
for an OpenAI-compatible server. In `tagent-gui`, a ⚠ marks such a profile, in Settings
and in the provider menu. Add the option (see [Provider options](../reference/provider-options.md)),
or set it in the [environment](../providers/api-keys.md). An environment variable must
be visible to the app: started from the desktop menu, it doesn't see variables set only
in a terminal.

### "invalid provider options: `endpoint` must be an http(s) URL"

The `endpoint` lacks `http://` or `https://`. Write it in full, with `/v1` for an
OpenAI-compatible server: `http://localhost:11434/v1`.

## The request fails

### "network error: …"

```
network error: error sending request: error trying to connect: tcp connect error:
Connection refused (os error 111)
```

The server can't be reached:

- **A local server** (Ollama, LM Studio) isn't running, or listens on another port.
  `curl http://localhost:11434/v1/models` (or your `endpoint` plus `/models`) should
  answer.
- **A cloud service**: check the internet connection, a firewall or proxy that blocks the
  app, and the address in `endpoint` if you set one.
- **Google** may be briefly unavailable, or refuse very heavy use. Try again later.

"request timed out" means the server didn't answer within `timeout_secs`. A local model
loading on its first request often needs longer: raise `timeout_secs` (60 by default for
OpenAI-compatible servers).

### "authentication failed: HTTP 401 / 403 …"

```
authentication failed: HTTP 403 Forbidden: {"message":"Forbidden. ..."}
```

The service refused the key: it is wrong, revoked, or for another service. For DeepL,
check that it is an API key (from the API section of your account), not something else.
A key set in an environment variable wins over the one in the file; `/config` in
`tagent-cli` shows which one is used. Not retried.

### "rate limited by the provider"

The service asks to slow down (HTTP 429). When the service names a short wait, DeepL and
OpenAI-compatible providers wait and retry by themselves (up to `max_retries`); the error
means that didn't help, or the wait was too long. The message shows the wait when the
service named one: `(retry after 30 s)`. Wait a little before trying again. Google is
never retried.

### "provider quota exceeded: …"

The account's allowance is used up: DeepL's monthly character limit, or an OpenAI-style
account's credit or spending limit. Not retried. Check the usage in your account, or
switch to another provider for now (`/p google` in `tagent-cli`, the provider menu in
`tagent-gui`).

### "provider API error: HTTP 404 … model … not found"

```
provider API error: HTTP 404 Not Found: {"error":{"message":"model 'qwen3' not found", ...}}
```

The server doesn't have the model in `model`. Use the exact name the server lists, tag
included: `ollama list` for Ollama (`qwen3:8b`, not `qwen3`), `curl <endpoint>/models`
for others. A 404 for any model usually means `endpoint` lacks `/v1`.

### Other "provider API error: …"

The service answered with an error the message quotes. Common ones:

- **HTTP 400** naming `response_format`: the server doesn't support the format you set.
  Remove `response_format`, or use `json_schema` (LM Studio accepts only that).
- **HTTP 400** naming `temperature`: the model accepts only its default; remove
  `temperature`.
- **The answer was cut off, or the model refused**: a language model stopped before
  finishing (it ran out of output length) or declined the text. Tagent never shows a
  partial translation. Try a shorter text or another model.

## The dictionary is silent

You translate a single word and get a plain translation, never a dictionary entry. A
dictionary lookup that fails falls back to the plain translation **without an error**,
so check:

1. **The dictionary is on**: `show_dictionary = true` in `tagent-cli.toml`; "Show
   dictionary for single words" in `tagent-gui`'s Settings > General.
2. **It's a single word.** Two words are a phrase and always get a plain translation.
3. **The dictionary provider works.** In `tagent-gui`, **Test** in the profile's
   **Options…** panel runs a lookup and shows its error. In `tagent-cli`, `/p` lists the
   dictionary provider; one that lacks options says what is missing.
4. **With a language model as the dictionary:**
   - **Small models** sometimes answer with an empty `{}` or something other than the
     requested JSON. An empty answer counts as "no entry". Add
     `response_format = "json_schema"` to the profile, if the server supports it, or use
     a larger model.
   - **A custom `dictionary_prompt`** must still ask for the exact JSON shape of the
     built-in one (see [Provider options](../reference/provider-options.md#openai)). A
     prompt that asks for anything else makes every lookup fail, which looks like "no
     dictionary". Remove it to go back to the built-in prompt.
   - **A slow model** may run past `timeout_secs` on the lookup, while the translation,
     a shorter answer, makes it in time.
5. **Google's dictionary** knows English best. A word in another source language, or a
   rare word, may have no entry.

## Speech

- **Speaking `auto`-source text** first asks the *translation* provider to detect the
  language. If that fails (an unknown or misconfigured translation provider), speech
  falls back to English rather than failing.
- **No sound at all**: see [Platforms](platforms.md).
