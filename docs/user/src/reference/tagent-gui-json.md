# tagent-gui.json

`tagent-gui` keeps its settings in a JSON file, `tagent-gui.json`:

- Linux: `~/.config/tagent-gui/tagent-gui.json`
- macOS: `~/Library/Application Support/tagent-gui/tagent-gui.json`
- Windows: `%APPDATA%\tagent-gui\tagent-gui.json`

Settings writes it, but it is meant to be edited by hand too. `tagent-cli` has its own
settings and doesn't read this file.

- A missing file is created with the defaults on first start; a missing key takes its
  default.
- The file is read again before each translation, so a hand edit applies without a
  restart, except where noted below.
- A file that isn't valid JSON is left alone: the app writes a warning to its log and
  keeps its previous settings until the file is fixed.
- On Linux and macOS it is written readable by you only (`0600`), since it can hold API
  keys.

Colors are `"#RRGGBB"`, or `""` for the theme's color.

## Providers and languages

| Key | Default | Meaning | Settings |
|-----|---------|---------|----------|
| `translate_provider` | `"google"` | The translation provider (a profile name) | Providers |
| `dictionary_provider` | `"google"` | The dictionary provider | Providers |
| `speech_provider` | `"google"` | The speech provider | Providers |
| `provider_options` | `{}` | Provider profiles: `{"<name>": {"<option>": "<value>"}}`; see [How providers work](../providers/how-providers-work.md) | Providers |
| `hidden_providers` | `[]` | Profiles unticked under "Show in lists" | Providers |
| `source_language` | `"auto"` | The main window's source language at start (`"auto"` or a code) | General |
| `target_language` | the system language, else `"en"` | The main window's target language at start (a code) | General |

## Behavior

| Key | Default | Meaning | Settings |
|-----|---------|---------|----------|
| `show_dictionary` | `true` | A dictionary entry for a single word | General |
| `spell_check` | `true` | Look misspelled words up under their correct spelling | General |
| `enable_text_to_speech` | `true` | Speaking: the 🔊 in the transcript's prompts and the speech hotkey | General |
| `show_context_menu` | `false` | Right-click opens a Copy menu instead of copying | General |
| `translate_hotkey` | `"Alt+A"` | The translate hotkey; restart required | Hotkeys & Tray |
| `speech_hotkey` | `"Alt+S"` | The speech hotkey; restart required | Hotkeys & Tray |
| `enable_speech_hotkey` | `true` | Whether the speech hotkey is active; restart required | Hotkeys & Tray |
| `start_minimized` | `true` | Start in the tray, without the window; read at start | Hotkeys & Tray |
| `remember_window_geometry` | `true` | Reopen the window where it was | Hotkeys & Tray |
| `window_geometry` | `null` | The remembered window, `{"x", "y", "width", "height"}`; written by the app | — |

## Look

| Key | Default | Meaning | Settings |
|-----|---------|---------|----------|
| `theme` | `"auto"` | `"auto"` (follow the system), `"light"` or `"dark"` | View |
| `background_color` | `""` | The transcript's and the input box's background | View |
| `show_prompt` | `true` | The `[Language]:` prompt before each line | View |
| `prompt_color` | `""` | The prompt's color | View |
| `phrase_font`, `translation_font` | `"monospace"` | Font family of the phrase and the translation lines | View |
| `phrase_size`, `translation_size` | `13` | Font size in pixels | View |
| `phrase_color`, `translation_color` | `""` | Text color | View |
| `phrase_background`, `translation_background` | `""` | Background color of the lines | View |
| `input_size` | `13` | Font size in pixels of the transcript's header, the input box's label and its text | View |
| `block_spacing_px` | `20` | Gap between entries, in pixels | View |
| `phrases_spacing_px` | `2` | Gap between a phrase and its translation, in pixels | View |

The color scheme on the View tab isn't stored by name: picking one sets these colors.

## Popup

| Key | Default | Meaning | Settings |
|-----|---------|---------|----------|
| `show_popup` | `true` | Show the popup on the translate hotkey | Popup |
| `popup_show_prompt` | `true` | The prompt in the popup | Popup |
| `popup_prompt_color` | `""` | Its color; `""` follows `prompt_color` | Popup |
| `popup_show_phrase` | `true` | The phrase line in the popup | Popup |
| `popup_font` | `"monospace"` | Font family | Popup |
| `popup_size` | `13` | Font size in pixels | Popup |
| `popup_color` | `""` | Text color; `""` follows `translation_color` | Popup |
| `popup_background` | `""` | Background color | Popup |
| `popup_auto_hide_seconds` | `3` | Seconds before it hides; `0` means the default | Popup |
| `popup_max_width` | `600` | Maximum width in pixels | Popup |
| `popup_max_height` | `600` | Maximum height in pixels; taller text scrolls | Popup |
| `popup_border_width` | `1` | Border width in pixels | Popup |
| `remember_popup_position` | `false` | Show later popups where you dragged this one | Popup |
| `popup_position` | `null` | The remembered position, `{"x", "y"}`; written by the app | — |

## Example

```json
{
  "translate_provider": "ollama",
  "dictionary_provider": "ollama",
  "target_language": "de",
  "theme": "dark",
  "translate_hotkey": "Ctrl+Ctrl",
  "provider_options": {
    "ollama": {
      "type": "openai",
      "endpoint": "http://localhost:11434/v1",
      "model": "qwen3:8b"
    }
  }
}
```
