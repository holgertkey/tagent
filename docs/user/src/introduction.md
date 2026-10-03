# Introduction

Tagent translates text where you are: select it in any application, press a hotkey, and
read the translation. It also looks single words up in a dictionary, corrects their
spelling, and reads text aloud.

It comes as two separate applications. Pick the one that fits how you work, or use both:

| | **tagent-cli** | **tagent-gui** |
|---|---|---|
| Where results appear | In a terminal | In a window, and in a popup next to the mouse cursor |
| Ways to translate | Global hotkey, an interactive prompt, the command line | Global hotkey, a text box |
| Runs | In a terminal window | In the system tray |
| Settings | A commented text file, `tagent-cli.toml` | A Settings dialog (and a JSON file) |
| Extra | One-shot use in scripts, translation history | Colors, fonts and themes; click any result to hear it |

The two don't share settings: each has its own file, and changing one doesn't affect the
other. They do share how translation works underneath, so everything under
[Providers](providers/how-providers-work.md) applies to both.

## What it can use

By default, Tagent uses Google: no account, no key, no setup. It can also use
[DeepL](providers/deepl.md), or a language model, local or in the cloud, through any
[OpenAI-compatible](providers/openai-compatible.md) server such as Ollama. Translation,
the dictionary and speech each have their own provider setting, so they can be mixed.

## Platforms

Both applications run on Windows and Linux; the global hotkeys on Linux need X11 or
XWayland. On macOS, translation, the dictionary and speech work, but the global hotkeys
and the clipboard features don't yet. See
[Troubleshooting: Platforms](troubleshooting/platforms.md).

## Where to start

1. [Install](getting-started/install.md) one or both.
2. Make a first translation with [tagent-cli](getting-started/first-translation-cli.md)
   or [tagent-gui](getting-started/first-translation-gui.md).
