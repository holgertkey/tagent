# Upgrading

## tagent-cli: new settings

`tagent-cli` never rewrites your configuration file on its own, so settings a newer
version adds don't appear in it by themselves. At start, under the banner, it tells you
when there are some:

```
Config: 3 new settings are available (run /config update or tagent-cli --update-config)
```

To add them:

```bash
tagent-cli --update-config        # or /config update at the prompt
```

- Each missing setting is added to its section with its explanation and default value;
  a missing section is added in its place. Example provider profiles your file lacks
  (for a provider that became available since, say) are added at the end, commented out.
- Your values, comments and the order of your settings stay as they are. Nothing is
  removed or renamed; settings this version doesn't know are listed for you to handle.
- The previous file is kept as `tagent-cli.toml.bak`.
- A file `tagent-cli` can't load is left alone: fix the reported mistake first.

To keep a setting out of your file for good, leave it there commented out
(`# speech_hotkey = "Alt+S"`): a commented-out setting counts as present, so it isn't
added back and isn't counted as new.

`--update-config` doesn't change examples already in the file. To see the current ones
(the built-in prompts of the OpenAI-compatible provider, for example), compare with
`tagent-cli --print-default-config`.

## tagent-cli: from 0.16 and older

Version 0.17.0 replaced the INI file `tagent-cli.conf` with `tagent-cli.toml`. The old
file is no longer read; the new one starts with defaults. Copy your settings over by
hand, using [tagent-cli.toml](../reference/tagent-cli-toml.md) as the guide.

## tagent-gui: the menu entry from 0.14 and older (Linux)

`tagent-gui` 0.15 renamed its menu entry and icon to `io.github.holgertkey.TagentGui`
(the Wayland hotkeys need a name of that form). An entry installed earlier with
`--install-desktop` stays behind, so the app may show up twice in the menu. Remove the
old files and install the new ones:

```bash
rm ~/.local/share/applications/tagent-gui.desktop \
   ~/.local/share/icons/hicolor/512x512/apps/tagent-gui.png
tagent-gui --install-desktop
```

The `.deb` replaces its own files when upgraded.
