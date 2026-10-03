# tagent-gui

A desktop translator: select text in any application and press **Alt+A**, and the
translation pops up next to the mouse cursor. It lives in the system tray, keeps a
transcript of your translations, looks single words up in a dictionary, and reads text
aloud. Built with [Slint](https://slint.dev/) on the
[`tagent`](https://github.com/holgertkey/tagent/tree/main/tagent) library.

📖 **[User guide](https://holgertkey.github.io/tagent/)**: installation, usage, providers, settings reference and
troubleshooting.

`tagent-gui` is independent of
[`tagent-cli`](https://github.com/holgertkey/tagent/tree/main/tagent-cli), the terminal
version: its own interface, settings and versions. They share only the `tagent` library.

## Features

- **Selection hotkeys** (Windows, Linux with X11 or XWayland): translate into a popup (Alt+A), or read aloud (Alt+S).
- **Main window** with language pickers, a highlighted transcript, 🔊 on every result, copy by right-click.
- **Dictionary** for single words, with synonyms and spelling correction.
- **Providers**: Google by default (no setup), DeepL, or a local or cloud language model (Ollama, LM Studio, OpenAI, ...), separately for translation, the dictionary and speech; switch them from the window for a session.
- **Settings dialog**: themes and color schemes, fonts, the popup's look, hotkeys, provider profiles with a Test button.
- **System tray**: starts minimized; closing the window hides it.

## Install

- **Download** from the [latest release](https://github.com/holgertkey/tagent/releases/latest):
  a `.zip` for Windows, a `.tar.gz` or a `.deb` for Linux
  (`sudo apt install ./tagent-gui_<version>-1_amd64.deb`).
- **Or with Cargo**: `cargo install tagent-gui` (on Linux, first
  `sudo apt-get install libx11-dev libxtst-dev libasound2-dev libfontconfig1-dev` or your
  system's equivalent), then `tagent-gui --install-desktop` for a menu entry.

More in [Install](https://holgertkey.github.io/tagent/getting-started/install.html).

## Quick start

1. Start `tagent-gui`. It goes to the system tray; click the icon to show the window.
2. Select text anywhere and press **Alt+A**.
3. Or type in the window and press Enter.
4. ⚙ opens Settings.

No tray on your desktop? See [Tray and startup](https://holgertkey.github.io/tagent/gui/tray-and-startup.html#without-a-tray).

## More

- [User guide](https://holgertkey.github.io/tagent/): every feature, the `tagent-gui.json` reference, troubleshooting.
- [CHANGELOG.md](CHANGELOG.md): the version history.
- Issues and suggestions: [GitHub issues](https://github.com/holgertkey/tagent/issues).

Early (`0.x`): usable day to day, but settings and behavior may still change between
releases.

## License

MIT, see [LICENSE](../LICENSE).
