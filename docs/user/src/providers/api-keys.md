# API keys and environment variables

A provider that needs a key ([DeepL](deepl.md), or an
[OpenAI-compatible](openai-compatible.md) cloud service) takes it as the `api_key`
option of its profile. You can store the key in the configuration file, or keep it in an
environment variable.

## In the configuration file

- **tagent-cli:** `api_key = "..."` in the profile's `[provider_options.<name>]` table.
- **tagent-gui:** the `api_key` field in the profile's **Options…** panel (Settings >
  Providers). The field hides what you type.

Both apps protect the file, but only as well as your user account is protected:

- On Linux and macOS, the apps write their configuration file readable by you only
  (permissions `0600`).
- On Windows, the file is in your own profile folder (`%APPDATA%`), which other ordinary
  users can't read.
- The key is stored as plain text. Anyone with access to your account can read it, and so
  can backups of your home folder.

`/config` in `tagent-cli` shows keys masked.

## In an environment variable

An environment variable named `TAGENT_<PROFILE>_<KEY>` sets option `<KEY>` of profile
`<PROFILE>`, and wins over the file:

| Profile | Option | Variable |
|---------|--------|----------|
| `deepl` | `api_key` | `TAGENT_DEEPL_API_KEY` |
| `deepl-work` | `api_key` | `TAGENT_DEEPL_WORK_API_KEY` |
| `openrouter` | `api_key` | `TAGENT_OPENROUTER_API_KEY` |
| `ollama` | `model` | `TAGENT_OLLAMA_MODEL` |

The name is the profile name and the option name in capitals, with every character other
than a letter or digit (such as `-`) turned into `_`.

- Any option works this way, not just `api_key`, except `type`.
- An empty variable is ignored.
- The profile has to exist: a profile of your own needs its table (or its entry in
  `tagent-gui.json`) with at least `type`. A built-in name (`deepl`, `openai`) works
  with no table at all, so `TAGENT_DEEPL_API_KEY` alone is enough for `deepl`.
- `/config` in `tagent-cli` marks each value set by a variable. In `tagent-gui`, the
  **Options…** panel says which field a variable currently overrides.

The variable must be set where the app starts:

- **Linux, macOS:** for a terminal, `export TAGENT_DEEPL_API_KEY="..."` in your shell's
  startup file (`~/.bashrc`, `~/.zshrc`). For `tagent-gui` started from the desktop
  menu, the variable must be in your login session's environment (`~/.profile` on most
  Linux desktops); log out and in again after adding it.
- **Windows:** `setx TAGENT_DEEPL_API_KEY "..."` in a terminal, or System > About >
  Advanced system settings > Environment Variables. Programs started afterwards see it;
  restart Tagent.

An app reads the variables each time it builds a provider, but a changed variable only
reaches an app started after the change.
