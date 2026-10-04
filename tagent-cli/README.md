# Tagent Text Translator v0.17.0+022

A fast, lightweight text translator for the terminal. Select text in any application and
press **Alt+A** to see its translation, type at an interactive prompt, or translate from
the command line. Windows and Linux (X11, and Wayland desktops such as GNOME through the
Global Shortcuts portal); on macOS, the prompt and the command line work.

📖 **[User guide](https://holgertkey.github.io/tagent/)**: installation, usage, providers, configuration reference and
troubleshooting.

## Features

- **Global hotkeys**: translate the selection (Alt+A) or read it aloud (Alt+S), in any application.
- **Interactive prompt** with line editing, input history and Tab-completion.
- **Command line**: one-shot translations for scripts (`tagent-cli "Hello world"`).
- **Dictionary** for single words: translations by part of speech, with synonyms and spelling correction.
- **Text-to-speech** of the selection, the last phrase or its translation.
- **Providers**: Google by default (no setup), DeepL, or a local or cloud language model (Ollama, LM Studio, OpenAI, ...), separately for translation, the dictionary and speech.
- **Translation history**, custom hotkeys and colors, a commented TOML configuration that reloads by itself.

## Install

- **Download** `tagent-cli` for Windows or Linux from the
  [latest release](https://github.com/holgertkey/tagent/releases/latest), unpack it and run it.
- **Or with Cargo**: `cargo install tagent-cli` (on Linux, first
  `sudo apt-get install libx11-dev libxtst-dev libasound2-dev` or your system's equivalent).

More in [Install](https://holgertkey.github.io/tagent/getting-started/install.html).

## Quick start

```bash
tagent-cli                      # the prompt, plus the hotkeys: select text anywhere, press Alt+A
tagent-cli "Hello world"        # translate once and exit
tagent-cli -l de "Hello world"  # into German
tagent-cli -s "Hello world"     # read aloud
tagent-cli --help
```

At the prompt, `/h` lists the commands, `/l en de` sets the languages, `/p` switches
providers, `/save` keeps your choices. See
[First translation](https://holgertkey.github.io/tagent/getting-started/first-translation-cli.html).

## More

- [User guide](https://holgertkey.github.io/tagent/): every feature, the configuration reference, troubleshooting.
- [CHANGELOG.md](CHANGELOG.md): the version history.
- `tagent-gui`, the desktop window version: [tagent-gui](../tagent-gui/README.md).
- Issues and suggestions: [GitHub issues](https://github.com/holgertkey/tagent/issues).

**Current Version**: v0.17.0+022

## License

MIT, see [LICENSE](../LICENSE).

---

**Tagent Text Translator v0.17.0+022** - Fast, reliable, and feature-rich translation tool for Windows and Linux.
