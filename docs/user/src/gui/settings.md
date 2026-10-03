# Settings

Open Settings with **⚙** in the main window or **Settings…** in the tray menu. Changes
are saved when you click **OK**; **Cancel** drops them. Most apply at once; the hotkeys
and "Start minimized to tray" need a restart, as noted below.

The settings are stored in `tagent-gui.json`; see
[tagent-gui.json](../reference/tagent-gui-json.md) to edit it by hand.

## General

- **Default languages**: the source and target language the main window opens with. A
  warning appears when they are the same.
- **Show dictionary for single words**: a single word gets a dictionary entry instead of
  a plain translation.
- **Spell check suggestions**: a misspelled word is looked up under its correct spelling,
  with a notice. No effect while the dictionary is off.
- **Enable text-to-speech**: makes the transcript's prompts speak buttons (`[🔊 English]:`)
  and allows the speech hotkey.
- **Show menu on right-click**: right-click opens a **Copy** menu instead of copying at
  once.
- **Reset to Defaults**: sets every setting on every tab back to its default, except
  provider profiles, their options and "Show in lists". Nothing is saved before OK.

## Providers

- **Translation, Dictionary, Speech**: the provider for each job. A ⚠ next to a list
  means that its provider lacks a required option.
- **The list below**: the built-in providers and your profiles. **Options…** opens the
  options of one, with a **Test** button; **Delete** removes a profile of your own. The
  checkbox ("Show in lists") hides an entry from the lists here and in the main window's
  provider menu, without deleting it.
- **New profile**: a name and a kind, then **Add**.

The whole tab is explained in [How providers work](../providers/how-providers-work.md#in-tagent-gui).

## View

The look of the main window. Everything applies at once.

- **Theme**: `Auto` (follows the system's light or dark setting), `Light` or `Dark`.
- **Color scheme**: a set of colors for the transcript: Solarized Dark and Light,
  Dracula, Nord, Gruvbox Dark, Monokai, One Dark, Tokyo Night, Catppuccin Mocha, or
  Default (the theme's colors).
- **Background color**, **Prompt color**, and for the **Phrase** and **Translation**
  lines their font, size, text color and background. "Theme default" next to a color
  follows the theme.
- **Blocks spacing**, **Phrases spacing**: the gaps between entries, and between a
  phrase and its translation.
- **Show prompt**: the `[Language]:` prompt before each line.

On Linux, `Auto` may show a light window for a moment before it turns dark; pick `Light`
or `Dark` to avoid it.

## Hotkeys & Tray

- **Global hotkey (translate)**, **Global hotkey (speech)**: type a hotkey, or click
  **Record** and press it (Esc cancels). An error below the field explains what's wrong
  with it. See [Hotkeys and the popup](hotkeys-and-popup.md#choosing-a-hotkey).
- **Enable speech hotkey**.
- **Start minimized to tray**, **Remember window size and position**: see
  [Tray and startup](tray-and-startup.md).

The hotkeys take effect after a restart.

## Popup

The popup the translate hotkey shows; see [Hotkeys and the popup](hotkeys-and-popup.md).
Everything applies at once.

- **Show popup on hotkey**: off, the hotkey only adds to the transcript.
- **Show prompt**, **Prompt color**, **Show phrase**: what the popup shows.
- **Remember position after dragging**.
- **Font**, **Size**, **Text color**, **Background**. "Theme default" follows the View
  tab's color scheme.
- **Auto-hide (seconds)**: how long the popup stays; `0` means the default, 3 seconds.
- **Max width**, **Max height**: text taller than the maximum scrolls. **Border width**.

## About

The version of `tagent-gui`.
