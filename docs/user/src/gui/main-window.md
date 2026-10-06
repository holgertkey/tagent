# The main window

![The main window](../images/gui-main-window.png)

From top to bottom:

## The toolbar

- **Source and target language.** Each language is listed with its code, such as
  `Russian (ru)`; the source list starts with `Auto` (detect the language). The window opens with the default languages from Settings > General; a
  change here holds until you quit, or until you change the defaults. `/l` in the input
  box does the same from the keyboard ([commands](commands.md)).
- **⇄** swaps the two languages. It is disabled while the source is `Auto`.
- **The provider button** names the translation provider in use, such as `google ▼`.
  Its menu switches providers for this run; see [below](#switching-providers).
- **⚠** appears when a provider in use lacks a required option (an API key, a server
  address). Click it to open Settings on the Providers tab.
- **⚙** opens [Settings](settings.md).

## The transcript

Every translation is added to the transcript, newest at the bottom. Each entry shows
your phrase and its translation, each behind a prompt, as in `tagent-cli`: the phrase's
names the language pair (`[auto → ru]:`), the translation's the provider that made it
(`[deepl]:`; for a dictionary entry the dictionary provider). So every entry still says
which provider answered after you switch providers. A single word shows a dictionary
entry: the main translation, the parts of speech, and the synonyms in brackets. Parts of speech, synonyms, a spelling-correction
notice and errors each get their own color.

The transcript's header names the providers in use for each job.

- **The prompt is the speak button.** Click `[auto → ru 🔊]:` before a phrase to hear
  it, in its source language (detected if `auto`); click `[deepl 🔊]:` before the
  translation to hear the translation, in the target language. For a dictionary entry, only the main translation is read.
  With the prompt turned off (Settings > View), a block starts with just 🔊. The prompt
  is tinted while it plays; click it again to stop, or press the speech hotkey again,
  or **Esc** in any application (Windows and Linux with X11; on Wayland only in Tagent's
  window; while the hotkeys are active). Only one entry
  plays at a time: the others can't be clicked meanwhile. With text-to-speech off
  (Settings > General), the 🔊 disappears from the prompts. The
  [popup](hotkeys-and-popup.md#the-popup)'s prompts speak the same way.
- **Right-click** an entry's phrase or translation to copy it as plain text, without
  the prompt. A brief flash of its border confirms the copy. With "Show menu on
  right-click" (Settings > General), right-click opens a **Copy** menu instead.

The transcript is cleared when you quit. Text in it can't be selected with the mouse;
copy with right-click.

## The input box

Type or paste text and press **Enter**, or click **Translate**. **Shift+Enter** starts a
new line. The label before the box shows the selected language pair, `[auto → ru]:`. Drag the bar above the box to make it taller.
Whenever the window opens (at startup, from the tray or from a second start), the
keyboard focus is in the box, so you can type or paste right away.

**📋** puts the clipboard's content into the box, ready to translate.

A line starting with `/` can be a command: `/l de` sets the languages, `/p deepl` the
provider, `/help` lists them all. See [Commands in the input box](commands.md).

**Ctrl+V**, **Ctrl+C**, **Ctrl+X**, **Ctrl+A** and **Ctrl+Z** work in any keyboard layout,
Russian or Greek included: a key counts as the Latin letter at its place on a US keyboard.

## Switching providers

The provider button's menu has a section for each job: Translation, Dictionary and
Speech. Each lists the built-in providers and your profiles; a ⚠ marks one that lacks
a required option, and a job that is turned off in Settings shows `(off)`.

A pick holds **for this run only**: the header marks it `(this session)`, and Settings
keeps showing, and saving, your defaults. The pick ends when you quit, pick the default
again, or change that default in Settings. **Providers…** at the bottom of the menu opens
Settings > Providers. `/p` in the input box lists and picks the same entries from the
keyboard ([commands](commands.md)).

See [How providers work](../providers/how-providers-work.md).

## Closing the window

The window's close button hides it to the tray; Tagent keeps running, and the hotkeys
keep working. **Quit** in the tray menu exits. Without a tray, see
[Tray and startup](tray-and-startup.md#without-a-tray). See [Tray and startup](tray-and-startup.md).
