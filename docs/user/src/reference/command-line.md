# Command-line options

```
tagent-cli [OPTIONS] [text]
```

Without arguments, `tagent-cli` starts [unified mode](../cli/modes.md) (the prompt and the
hotkeys). With text, it translates it and exits.

| Option | What it does |
|--------|--------------|
| `<text>` | Translate the text and exit; quote a phrase with spaces. A single word shows a dictionary entry |
| `-l <target> <text>`, `--lang` | Translate into `<target>` (source `auto`), for this run only |
| `-l <source> <target> <text>` | Translate from `<source>` into `<target>`, for this run only |
| `-s <text>`, `--speech` | Read the text aloud |
| `-c`, `--config` | Show the current settings and profiles (keys masked) |
| `-v`, `--version` | Show the version |
| `-h`, `--help` | Show the help |
| `--print-default-config` | Print a new configuration file with every setting at its default; changes nothing |
| `--update-config` | Add the settings your configuration file lacks; see [Upgrading](../troubleshooting/upgrading.md) |
| `--install-desktop` | Linux: add the "Tagent CLI" menu entry and icon for your user, pointing at this program; the hotkeys on Wayland need it. See [Hotkeys: On Wayland](../cli/hotkeys.md#on-wayland) |
| `--uninstall-desktop` | Linux: remove what `--install-desktop` added |

Languages are names or codes: `-l German`, `-l de`, `-l English German`.

```bash
tagent-cli hello
tagent-cli "Hello world"
tagent-cli -l de "Hello world"
tagent-cli -l en de "Hello world"
tagent-cli -s "Bonjour le monde"
tagent-cli --print-default-config > tagent-cli.new.toml
```
