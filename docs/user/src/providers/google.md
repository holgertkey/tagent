# Google

`google` is the default provider for all three jobs: translation (Google Translate), the
dictionary (Google Dictionary) and speech (Google TTS). It needs no account, no key and
no setup, so a fresh install of either app translates right away.

## What to know

- **It uses unofficial, free Google Translate endpoints**, not the paid Google Cloud
  Translation API. They have no published limits and no guarantees: Google may slow
  down or refuse very heavy use.
- **Your text is sent to Google.** Use a [local model](recipes/ollama.md) for text that
  must not leave your computer.
- **The dictionary** works best for English words. It also corrects spelling: a typo
  such as `violnt` is looked up as `violent`, and Tagent says so (with `spell_check` on).
- **Speech** is split into pieces of up to 100 characters, which play one after another.
  Playback starts after the first piece arrives, so long text doesn't keep you waiting.

## Options

Google takes only the two general options:

| Option | Default | Meaning |
|--------|---------|---------|
| `timeout_secs` | `10` | Time budget for one call, in whole seconds, retries included |
| `max_retries` | `1` | How often a failed request is retried; `0` disables retries |

A "too many requests" answer from Google is never retried.

To change them for the built-in `google`:

- **tagent-cli:**

  ```toml
  [provider_options.google]
  timeout_secs = "20"
  ```

- **tagent-gui:** Settings > Providers > **Options…** on the `google (built-in)` row.
