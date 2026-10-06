# Changelog

All notable changes to `tagent-gui` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`tagent-gui` versions independently of `tagent-cli` (see the root
[CHANGELOG.md](../tagent-cli/CHANGELOG.md) for that application's history) and independently of
the `tagent` library. See [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) for the design
decisions behind this project.

There is one section per version. Between releases the version carries a `+BUILD` counter
(`0.15.0+003`) that names a development iteration; an entry names its build in
parentheses (`(+003)`, or the full version for an earlier version's build, e.g.
`(0.14.0+041)`). Sections from before 2026-10-02 have one header per build.

## [Unreleased]

## [0.15.0] - 2026-10-06

### Added
- (+018) **Commands in the input box.** `/l de` or `/l en ru` sets the languages and `/l`
  swaps them; `/p` lists the providers of all three jobs, numbered, and `/p 3`,
  `/p deepl` or `/p d ollama` switches one; `/s` and `/ss` read the last phrase or
  translation aloud, `/s <text>` reads the text; `/clear` empties the transcript;
  `/help` lists the commands and `/v` shows the version (with the `tagent` library's).
  The answer appears in the transcript as a `[cmd]:` entry. Like the window's lists and
  provider menu, nothing is saved. Only these names are commands: other text starting
  with `/` is translated, and `//l` translates `/l`. A command with a typo stays in the
  box to be fixed. The hotkey's selection is never read as a command. The empty input
  box now says `Type to translate, /help for commands`.
- (+014) **A font size for the header and the input box.** Settings > View >
  "Header & input" (`input_size` in `tagent-gui.json`, default `13`, as before) sets the
  size of the transcript's header, the input box's `[auto → ru]:` label and the text
  typed in the input box, which were fixed at 13 px; the input box grows to fit a
  larger size. The transcript's prompts keep following the Phrase and Translation
  sizes.
- (+012) **The popup's prompts speak, like the transcript's.** Click `[🔊 English]:`
  before the phrase to hear it (in the detected language with `Auto`), or
  `[🔊 Russian]:` before the translation to hear the translation (for a dictionary
  entry, its main translation); with the popup's prompt off, a line starts with just 🔊.
  It is the same playback as the transcript row of that translation: tinted in both
  windows while it plays, a second click (or the speech hotkey, or Esc where it works)
  stops it, and only one plays at a time. The popup stays open while its own translation
  is being read and hides the usual number of seconds after it ends. Right-click copies
  over the prompt too; the popup is dragged by the rest of its surface. With
  text-to-speech off, the prompts stay plain.
- (+009) **Only one copy runs per user.** Starting `tagent-gui` again (the menu entry, a
  terminal) shows the running copy's window, as the tray's "Show Tagent" does, and the
  new start exits; from a terminal it prints `tagent-gui is already running (pid …);
  showed its window.` A second copy used to start next to the first: without hotkeys on
  X11, with undefined hotkey delivery on Wayland, a second tray icon, and both rewriting
  `tagent-gui.json`. The copies talk over a local socket, per user
  (`$XDG_RUNTIME_DIR/io.github.holgertkey.TagentGui.sock` on Linux, a named pipe on
  Windows); one left by a crashed copy is reclaimed, and a copy that holds it but
  doesn't answer within 2 seconds is reported (exit code 1) instead of starting a second.
- (+006) **Global hotkeys on Wayland** (GNOME 50 and other desktops with the
  GlobalShortcuts portal): `translate_hotkey` and `speech_hotkey` are registered through
  `xdg-desktop-portal` instead of X11 key grabs, which on a Wayland session only saw keys
  while an XWayland window had focus. The first start shows the desktop's dialog to
  confirm (or change) the keys; afterwards they are changed in the system settings
  (GNOME: Settings > Apps > Tagent), and Settings > Hotkeys & Tray shows what is bound.
  The hotkeys translate or speak the text selected with the mouse (the PRIMARY
  selection), so no Ctrl+C is simulated and the clipboard is left alone. The portal
  needs the desktop entry: installed by the `.deb`, otherwise run
  `tagent-gui --install-desktop` once; without it the transcript says so. A double-press
  hotkey (`Ctrl+Ctrl`) can't be proposed to the portal: the dialog then asks for a key.
- (+005) **A test keeps the user book's `tagent-gui.json` page complete**: it fails when a
  key of the default settings isn't named on `docs/user/src/reference/tagent-gui-json.md`
  (skipped in a copy without `docs/user`, such as the crates.io package).
- (+001) **The OpenAI-compatible provider can serve dictionary lookups too** (OpenAI, Ollama,
  LM Studio, ...), from `tagent`'s new `openai` dictionary provider: offered in the
  Settings > Providers dictionary picker, and one profile can serve translation and
  dictionary at once. Its "Options…" panel gains `dictionary_prompt` (a multi-line editor
  pre-filled with the built-in prompt) and `response_format` (`json_schema` or
  `json_object`, for servers with structured output); "Test" now checks the dictionary
  too for an `openai` profile. A failed lookup still falls back to a plain translation,
  so "Test" is where a broken custom prompt shows.
- (0.14.0+038) **"Show in lists"** on Settings > Providers: a checkbox on each row hides that provider
  or profile from the provider pickers (Settings and the main window), so they stay short
  once there are many profiles. It only hides: a hidden entry keeps working wherever it is
  selected, and stays in a picker that currently selects it. `google`, which pickers fall
  back to, can't be hidden. Saved as `hidden_providers` in `tagent-gui.json`; "Reset to
  Defaults" keeps it.
- (0.14.0+038) A ⚠ on each row of the Providers tab whose profile lacks a required option.
- (0.14.0+037) **A "Providers" tab in Settings** for creating and deleting provider profiles, no
  hand-editing of `provider_options` in `tagent-gui.json` needed. It lists the built-in
  providers (`google`, `deepl`, `openai`) and every profile as `name (kind)`, each with
  "Options…" (the existing options panel) and "Test"; profiles also get "Delete". Under the
  list, a name field, a kind dropdown and "Add" create a profile, e.g. several
  OpenAI-compatible servers side by side (`ollama`, `openrouter`, ...). A name the app
  can't use (empty, characters other than `a-z`, `0-9`, `_`, `-`, a built-in provider's
  name, an existing profile) is explained under the field and keeps "Add" disabled. After
  "Add", the options panel opens for the new profile. Deleting a profile that a picker on
  the General tab selects switches that picker to `google`, and a note says so. As
  everywhere in Settings, nothing is written before OK and Cancel drops it all; the
  General pickers show added profiles and lose deleted ones right away. On OK the file is
  re-read first, so a hand-edit made meanwhile survives unless it touches the same
  profile. A profile with an unknown kind (hand-edited, or its provider compiled out) is
  listed with "Delete" only. Nothing on the tab is specific to a provider: it is all
  driven by `tagent`'s provider registry.
- (0.14.0+037) **"Test" for a provider profile**, on each row of the Providers tab and in the options
  panel (there with the panel's unsaved values). It builds the profile exactly as the app
  would (options of this dialog plus `TAGENT_<NAME>_<KEY>` environment overrides) and
  makes one call per thing the provider does: a translation of "Hello, world!" into the
  configured target language (German if that is English), a dictionary lookup of
  "hello", and a speech request for "Hello" whose audio is only measured, never played.
  One line per call, e.g. `translation: OK (0.9 s): Hallo, Welt!` or
  `translation: failed: <error>`; a missing required option fails before any network
  call. A test of a paid service costs a few characters or tokens.
- (0.14.0+036) **A multi-line editor for prompt options** in Settings > General's "Options…" panel. An
  option `tagent` declares as multi-line (today the `openai` kind's `translate_prompt`)
  gets a text editor under its label instead of a one-line field. With no value set, the
  editor shows the provider's built-in prompt, ready to edit in place. "Reset to default"
  puts the built-in text back. A prompt saved unchanged (or identical to the built-in one)
  stores nothing in `tagent-gui.json`. A ⚠ under the editor warns, without blocking OK,
  when the prompt has no `{to}`, since the model then isn't told the target language.
  Nothing in the panel is specific to a provider: it follows the option's declared
  `multiline` and `default`.
- (0.14.0+035) **An OpenAI-compatible translation provider** (OpenAI, Ollama, LM Studio, OpenRouter,
  vLLM, ...) is available via a profile, from `tagent`'s new `openai` provider: a
  `provider_options` entry with `"type": "openai"`, an `endpoint` (the base URL including
  `/v1`, e.g. `http://localhost:11434/v1`) and a `model`. Settings > General offers
  `openai` in the translation picker (and every profile of that kind); its "Options…" panel has `endpoint` and
  `model` as required fields and `api_key` as a password field. The `translate_prompt`
  option is a single-line field for now.
- (0.14.0+031) **Default language pair in Settings > General** ("Default source language" / "Default
  target language"), saved as language codes in `tagent-gui.json`'s new
  `source_language`/`target_language` (e.g. `"auto"`, `"ru"`). The main window starts with
  this pair; picking another language there, or ⇄, still holds for the run only. A new
  default (Settings OK or a hand-edit of the file) switches the main window to it, already
  for the translation that picks up the edit. Settings warns when the two are the same
  language.
- (0.14.0+031) An unknown code in the file is replaced (with a warning in the log) by `"auto"` for the
  source and the system default for the target; the file keeps it until the next save.
- (0.14.0+029) **Translation provider picker in the main window**, next to ⚙, with the same choices
  as Settings > General (built-in providers and translation profiles from
  `provider_options`). A pick is saved to `tagent-gui.json` at once and applies to the
  next translation, from the window and from the hotkey. The picker follows the config:
  a Settings save or a hand-edit of the file shows up in it, and a pick made while Settings
  is open updates Settings' own picker too. While the selected provider lacks a required
  option (e.g. DeepL without `api_key`), a ⚠ appears next to it; clicking it opens
  Settings.
- (0.14.0+028) **A ⚠ next to a provider picker** whose provider lacks a required option, with a line
  below the pickers naming it (e.g. `deepl: api_key required`). An environment variable
  such as `TAGENT_DEEPL_API_KEY` counts as set.
- (0.14.0+027) **The transcript header shows the providers in use**, in a `Providers:` block before the
  active hotkeys: `Translation: Google Translate`, `Dictionary: Google Dictionary`,
  `Speech: Google TTS`. Each is the provider's own name, so a profile shows as e.g.
  `DeepL (work)`. The dictionary line appears only with `show_dictionary`, the speech line
  only with `enable_text_to_speech`. A provider that can't be built (e.g. a DeepL profile
  without `api_key`) shows `unavailable (<reason>)`. Follows the config live: a Settings
  save or a hand-edit of `tagent-gui.json` updates it with the next translation.
- (0.14.0+026) **DeepL translation provider** in Settings > General's translation picker. Its
  "Provider options" show `api_key` (a password field, required; a Free key ends in `:fx`),
  `endpoint`, `timeout_secs` and `max_retries`; the key can also come from the
  `TAGENT_DEEPL_API_KEY` environment variable, or a named profile in `provider_options`
  with `"type": "deepl"`. DeepL is translation only; dictionary and speech stay on Google.
  Speaking `Auto`-source text with DeepL selected detects the language through DeepL,
  which bills up to 100 characters.
- (0.14.0+024) **`.deb` package**: each release now also attaches `tagent-gui_<version>-1_amd64.deb`
  (built with `cargo-deb` from `[package.metadata.deb]` in `Cargo.toml`). It installs
  `/usr/bin/tagent-gui`, the launcher entry, the icon in the `hicolor` theme, and the README
  and CHANGELOG; dependencies are computed from the binary. After installing it, the app is
  in the app grid and the dock shows its icon with no `--install-desktop` step.
- (0.14.0+024) The Linux release archive now carries `tagent-gui.desktop` and `tagent-gui.png` next to
  the binary. `assets/linux/tagent-gui.desktop` is the entry both packages ship; a test
  keeps it identical to the one `--install-desktop` writes (apart from `Exec=`).
- (0.14.0+023) **Linux desktop integration**: `tagent-gui --install-desktop` writes a launcher entry
  (`~/.local/share/applications/tagent-gui.desktop`, `Exec=` the running executable) and the
  app icon (`~/.local/share/icons/hicolor/512x512/apps/tagent-gui.png`, embedded in the
  binary), so GNOME's dock, Alt+Tab and the app grid show the Tagent icon instead of a
  generic one, and Tagent can be started from the app grid. `--uninstall-desktop` removes
  both files. Installing is explicit, never automatic; run it again after moving the binary.
  Both honor `$XDG_DATA_HOME`.
- (0.14.0+022) **Settings > General "Provider options"**: for each profile the three provider pickers
  select, one field per option its provider declares (for Google: `timeout_secs` and
  `max_retries`), with a short description; secret options (API keys) are password fields,
  required ones are marked `*`. The list follows the pickers as they change, typed values
  survive switching a picker back and forth, and nothing is written before OK. An empty
  field removes the option (the provider's default applies). A field an environment
  variable currently overrides says so. "Reset to Defaults" leaves provider options alone.
- (0.14.0+021) **Provider profiles and options** in `tagent-gui.json`: a new `provider_options` object,
  `{"<profile>": {"<key>": "<value>", ...}}`, passes options (`api_key`, `endpoint`,
  `model`, `timeout_secs`, `max_retries`, ...) to a provider, and an optional `"type"` entry
  makes the profile a named instance of a provider kind. `translate_provider`,
  `dictionary_provider` and `speech_provider` take a profile name; built-in names work as
  before. Profile names and keys are case-insensitive (saved back lowercase). A
  `TAGENT_<NAME>_<KEY>` environment variable overrides a value. Profiles are
  created by hand-editing; the Settings pickers offer them on every axis their kind supports,
  and saving Settings keeps them (read fresh at save time, so a hand-edit made while the
  dialog is open survives).
- (0.14.0+014) **Option to turn off the hotkey popup**: a new "Show popup on hotkey" checkbox on
  Settings > Popup (`show_popup` in `tagent-gui.json`, default `true`, applies at once).
  With it off, the global translate hotkey still translates the selection into the
  transcript, but no popup appears.
- (0.14.0+012) **Semantic highlighting in the hotkey popup**: the popup now uses the same role-tagged
  templates as the transcript. A dictionary hit shows part-of-speech labels, `[synonyms]`
  and the spelling-correction notice in their own colors, and an error shows in the error
  color. Before, only the `[Language]:` prompt was colored. The colors come from the popup's
  own background, and they update right away when the theme or popup colors change.
- (0.14.0+007) **The popup can be dragged again**, gated behind holding Ctrl (Ctrl+left-click-drag)
  instead of a plain click, so it no longer conflicts with right-click-to-copy over the same
  phrase/translation text -- `0.14.0+006` had disabled dragging entirely to make room for that
  feature. Works over the phrase/translation text itself; the popup's thin outer margin still
  doesn't support it.
- (0.14.0+005) **Right-click copy in the hotkey popup**, reusing the same "Show menu on right-click" setting
  as the transcript. With the menu off (default), right-click anywhere on the popup copies the
  translation, with a brief border flash confirming it. With the menu on, right-click opens a
  menu with "Copy phrase" and "Copy translation" (phrase omitted when the popup isn't showing
  it). The no-menu path always targets the translation -- copying the phrase there would need
  identifying which line was clicked without a new element competing with the popup's own
  drag-anywhere-on-its-surface behavior, and it's rarely wanted anyway (you already had the
  phrase; you selected it to trigger the popup).
  **Superseded the same day by `0.14.0+006` above** -- this version didn't work as intended;
  see that entry.
- (0.14.0+004) **"Show menu on right-click" setting** (Settings > General, default off): with it off, right-click
  on a transcript block copies it immediately instead of opening a one-item "Copy" menu -- a brief
  border flash confirms the copy since there's no menu-click to see it happen. With it on, right-click
  opens the "Copy" menu, same as before this change.
- (0.14.0+003) **Configurable prompt color**: the `[Language]:` prompt shown before the phrase and
  translation text -- in the input box, the transcript's own highlighted prefix (Stage 13),
  and (new) the hotkey popup's own prefix -- now has its own color picker, independent of
  the phrase/translation text colors. `prompt_color` (Settings > View) covers the main
  window; `popup_prompt_color` (Settings > Popup) covers the popup and, when left at "theme
  default", follows `prompt_color` first (falling further back to the raw theme default only
  if that's also empty) -- the same fallback chain `popup_color` already uses through
  `translation_color`. Each of the 14 non-"Default" color-scheme presets in Settings > View
  also picked up its own matching prompt accent.
- (0.14.0+003) The hotkey popup's own phrase/translation lines are now `StyledText` (were plain `Text`),
  so its `[Language]:` prefix can actually be highlighted in `popup_prompt_color` -- no other
  visual change to the popup; still no dictionary-structure highlighting there (out of scope,
  same as before).
- (0.14.0+002) **Semantic highlighting** in the transcript: a dictionary article's structure (part-of-speech
  labels, `[synonyms]`), a spelling-correction notice, the `[Language]:` prompt prefix, and error
  rows each get their own color, automatically derived from the block's own background (no new
  config or Settings controls). Colors re-render live if the desktop theme flips while `tagent-gui`
  is running (`Auto` theme) or the phrase/translation background is changed in Settings.
- (0.14.0+002) New `tagent-gui/src/styled.rs`: markdown templates with role-tagged `<font color="@role">`
  spans (colors substituted in at render time, never baked into a saved template), a mandatory
  `escape_markdown` every user- or provider-derived string goes through first, and
  `RoleColors::for_background` (WCAG-contrast-checked light/dark palettes). `dictionary.rs` gained
  a small role-tagged intermediate representation (`article_lines`) that both the existing plain
  formatter and the new template renderer derive from, so the two can't drift apart --
  `format_dictionary_entry`'s output (what the popup and history still use) is unchanged, pinned
  by a golden test.

### Changed
- (+020) **A fuller About tab** in Settings: besides the version, the `tagent` library's
  version, what the app does, the license, and links to the user guide, the source code
  and the issue tracker, which open in the browser.
- (+019) **The language lists show the codes.** The main window's two language lists and
  the default languages in Settings > General name each language with its code,
  `Russian (ru)`, the code the `[auto → ru]:` label shows; `Auto` stays as it is.
- (+017) **The 🔊 in a prompt moved before the closing bracket:** `[auto → ru 🔊]:`
  instead of `[🔊 auto → ru]:`, in the transcript and the popup, so the prompt starts
  with its label. Without a prompt, a block still starts with 🔊.
- (+013) **The prompts look like `tagent-cli`'s**: the phrase's names the language pair
  (`[auto → ru]:`, was `[Auto]:`), the translation's the provider that made it
  (`[deepl]:`, was `[Russian]:`), the dictionary provider for a dictionary entry. With
  providers switched during a run, every entry still says which one answered. The
  input box's label shows the pair too (`[auto → ru]:`). Same in the popup; the prompts
  still speak (the phrase in the source language, the translation in the target one).
  Uses `tagent::languages::pair_label` (`tagent` 0.19.0+006).
- (+006) **The app id is now `io.github.holgertkey.TagentGui`** (was `tagent-gui`): the
  GlobalShortcuts portal accepts only a reverse-DNS id with a matching desktop entry. The
  desktop entry and icon are renamed with it (`io.github.holgertkey.TagentGui.desktop`,
  `.png`). Not migrated: if you installed the old entry with `--install-desktop`, run
  `tagent-gui --uninstall-desktop` with the previous build (or delete
  `~/.local/share/applications/tagent-gui.desktop` and
  `~/.local/share/icons/hicolor/512x512/apps/tagent-gui.png`), then
  `tagent-gui --install-desktop` with this one.
- (+006) **Pressing the speech hotkey while something is speaking stops it** (it was
  ignored), on every platform. On Wayland it is the only global way to stop: Esc can't be
  observed there and works only while a Tagent window is focused.
- (+006) **On a Wayland session the windows run on XWayland**, so the popup can still
  place itself and stay on top. The pointer position is unknown over other apps there,
  so the popup opens at its remembered position or, without one, in the top-right corner
  of the screen, not next to the cursor.
- (+004) **The main window's provider picker is now a menu for all three axes.** The
  button next to ⚙ names the translation provider in effect (`ollama ▾`) and opens one
  menu with a Translation, a Dictionary and a Speech section, the provider in effect
  checked in each, plus "Providers…" (Settings on the Providers tab). A pick holds for
  this run only on its own axis, like the translation picker before: it is used by the
  Translate button, the hotkey and speech, the transcript header marks each overridden
  axis `(this session)`, and it ends when that axis's default changes in Settings or the
  file, when its profile disappears, or when the default is picked again. Entries hidden
  with "Show in lists" are left out (except the one in effect); an entry lacking a
  required option is marked `⚠ <keys>` and can still be picked. A turned-off axis
  (`show_dictionary`, `enable_text_to_speech`) keeps its section with "(off)". The ⚠ next
  to the button now covers the providers in effect on every turned-on axis.
- (+003) **Settings' "Test" names the dictionary lookup it makes**: `dictionary: OK (3.9 s):
  "hello" (en → ru): привет`. A miss read just "no entry", as if no word had been sent; it
  now says `no entry for "hello" (en → ru)` and that the provider's answers look unreliable,
  since every dictionary should know that word (a small language model sometimes answers
  with an empty `{}`). With `tagent` 0.19.0+004, a model's malformed JSON answer shows as
  "not valid JSON" with the parse error, instead of "not a JSON object".
- (+002) **The spelling-correction notice highlights the corrected word and no longer has a
  blank line after it**: in "Показан перевод слова violent" only the phrase keeps the
  notice color, the word is shown in the translation's own text color (`translation_color`,
  `popup_color` in the popup), and the dictionary article follows on the next line, in the
  transcript, the popup and the copied text.
- (+000) **Version 0.15.0**: the work since the 0.14.0 release (0.14.0+001 to
  0.14.0+041) goes out as 0.15.0, and its sections were merged into this one. From
  now on there is one changelog section per version; the `+BUILD` counter stays in the
  version and an entry names its build in parentheses.
- (0.14.0+041) **The `[Language]:` prompt of a transcript block is its speak button now**, shown as
  `[🔊 English]:` (just `🔊` when the prompt is turned off). Click it to hear the phrase or
  translation, click again to stop; it is tinted on hover and while it speaks. Replaces the
  separate 🔊 icon at the start of the block from 0.14.0+040, so the text is no longer
  indented and uses the full width of the transcript. With text-to-speech off, the glyph
  disappears from the prompts. Copied text, history and the popup don't get the glyph.
- (0.14.0+040) **The transcript's 🔊 speaker icons sit inside the phrase/translation blocks now**, at the
  start of the first line, with the text indented past them (wrapped lines too, like a list
  marker). They used to be buttons next to each block, and a button can't be shorter than
  the style's minimum height, so a one-line entry got buttons taller than its text and the
  row grew around them. One-line rows now keep the height of their text. Click, ⏹ while
  speaking and the dimmed state while another entry speaks work as before.
- (0.14.0+039) **"Test" for a provider profile is only in its "Options…" panel now**, no longer on each
  row of Settings > Providers. You test while filling in options, and the panel's Test
  already uses the values not yet saved; the rows are narrower, and the result shows next
  to the options it concerns.
- (0.14.0+038) **The translation, dictionary and speech pickers moved from General to the Providers
  tab**, above the list; their "Options…" buttons are gone, since every row of the list
  has one. General keeps the default languages and the other settings.
- (0.14.0+038) The ⚠ next to the main window's provider picker opens Settings on the Providers tab.
- (0.14.0+034) **Windows renders on the GPU again**: `tagent-gui` no longer forces Slint's software
  renderer (added in `0.14.0+011`) and uses Slint's default GPU renderer, as on the other
  platforms. `SLINT_BACKEND=winit-software` still selects the software renderer.
- (0.14.0+033) The system-locale lookup behind the default target language, and the check for a
  known language code, now use `tagent`'s `languages::language_for_locales` and
  `languages::language_code` (shared with `tagent-cli`). No visible change.
- (0.14.0+032) **Settings > General shows the default language pair in one row**, `Default languages:
  [Auto] → [Russian]`, below the provider pickers, instead of two rows above them.
- (0.14.0+031) **The default target language comes from the system's preferred languages**
  (`LANGUAGE`/`LC_ALL`/`LC_MESSAGES`/`LANG` on Linux, the preferred UI languages on
  Windows and macOS): the first one Tagent knows, else English. It was always Russian
  before, so an existing `tagent-gui.json` without `target_language` may start with a
  different target now; set it once in Settings > General.
- (0.14.0+030) **The main window's translation provider picker now switches for this run only**
  instead of saving to `tagent-gui.json`, like the language pickers next to it. The pick
  applies to everything in the running app (the Translate button, the hotkey, and language
  detection for speaking `Auto`-source text), and the transcript header marks it, e.g.
  `Translation: Google Translate (this session)`. Settings > General keeps showing and
  saving the default. The pick ends when the app exits, when the default changes (a
  Settings save or a hand-edit of the file; the picker then follows the new default), when
  its profile is removed from `provider_options`, or when the default itself is picked
  again.
- (0.14.0+028) **Provider options moved behind an "Options…" button** next to each of Settings >
  General's three provider pickers, instead of one long list under them. The button opens
  a panel over the Settings tabs with the fields of that provider only. When another
  picker selects the same profile (e.g. `google` for both dictionary and speech), the panel
  says so, since the options apply to both. The panel's OK keeps the edits and its Cancel
  drops them; nothing is saved until Settings' own OK, which is disabled while the panel is
  open.
- (0.14.0+025) **All 15 languages the `tagent` library knows are offered**, up from 5: the source and
  target dropdowns are built from `tagent::languages::LANGUAGES` (Chinese, Japanese,
  Korean, Italian, Portuguese, Dutch, Polish, Turkish, Arabic and Hindi are new), with
  Auto still offered as a source only. English/Russian/Spanish/French/German stay first,
  and Russian is still the default target.
- (0.14.0+023) On Linux the window class is now pinned to `tagent-gui` (`slint::set_xdg_app_id`:
  `WM_CLASS` on X11, the app id on Wayland) instead of following the executable's file name,
  so it matches the installed entry's `StartupWMClass=`.
- (0.14.0+020) **Google requests now retry once on a connection failure or an HTTP 502/503/504**
  (`tagent` 0.19.0's shared transport), within the same 10-second budget as before. A
  Google rate limit (HTTP 429) is reported as `rate limited by the provider` and is never
  retried, and HTTP errors read `HTTP 503 Service Unavailable` instead of
  `HTTP error: 503 Service Unavailable`. Network error messages no longer include the
  request URL, which contained the text being translated.
- (0.14.0+020) **Built against `tagent` 0.19.0**, whose `Error` is now `#[non_exhaustive]` and gained
  `Auth`, `RateLimited`, `QuotaExceeded`, `Unsupported` and `InvalidOptions` variants for
  keyed services. No behavior change: nothing returns them yet, and every new variant is
  shown with its own message.
- (0.14.0+019) **The dictionary layout now comes from the `tagent` library** (`tagent::article`, new in
  `tagent` 0.18.3) instead of a copy in `dictionary.rs`, and is shared with `tagent-cli`. The
  transcript, the popup and copied text are unchanged.
- (0.14.0+018) **The target-language dropdown no longer offers "Auto"**: auto-detection only makes sense
  for the source language, and picking it as the target used to be accepted by the dropdown
  and then fail with `Error: "Auto" is not a valid target language` on translate. The two
  dropdowns now have their own language lists, and that error is gone.
- (0.14.0+018) **The ⇄ swap button is disabled while the source language is "Auto"**, instead of silently
  replacing the target with English. With two concrete languages it swaps them as before.
- (0.14.0+015) **The popup drags with a plain left-button drag, from any point of it**: holding Ctrl is no
  longer needed (Ctrl+drag still works), and the thin margin around the phrase/translation
  text can be grabbed too, not just the text itself. Right-click still copies a line; a plain
  click without moving does nothing. The popup also no longer risks auto-hiding while the
  button is held down over the text.
- (0.14.0+013) **Doesn't hold the terminal (Linux/macOS)**: started from a terminal, `tagent-gui` now
  re-launches itself in its own session with no controlling terminal and returns the prompt
  at once. Closing the terminal or pressing Ctrl+C there no longer stops the app. Its
  diagnostics go to `tagent-gui.log` in the data directory
  (`~/.local/share/tagent-gui/` on Linux). A log bigger than 1 MiB is started over.
  `--foreground` (or `-f`) keeps the old attached behavior, with output in the terminal.
  Launches from outside a terminal (desktop launcher, autostart, systemd) behave as before.
  Windows already behaved this way.
- (0.14.0+006) **The popup can no longer be dragged.** Temporarily disabled to make room for the fix above:
  copying a specific line needs to know which one the cursor is over, which needs its own
  click-handling area on each line -- the same surface dragging already used for
  "drag from anywhere, including the text". The two would compete for the same clicks, so
  dragging is off for now rather than shipping a fix for one broken feature that quietly breaks
  another.
- (0.14.0+004) **Right-click copies immediately by default.** This changes the transcript's out-of-the-box
  right-click behavior introduced by the "Semantic highlighting" release above: previously right-click
  always opened a "Copy" menu; now it copies directly unless "Show menu on right-click" is turned on.
- (0.14.0+002) **The transcript is now view-only**: no mouse selection, no Ctrl+C. Right-click a phrase or
  translation block and choose "Copy" instead -- it copies just that block, as clean text (no
  `[Language]:` prefix, no markup; a dictionary hit copies the whole article). This is the
  tradeoff for the highlighting below: Slint's `StyledText` (needed to color parts of one
  wrapped paragraph differently) has no selection of its own. The input box, Settings, popup
  and tray are unaffected.

### Removed
- (0.14.0+017) **`xdotool` is no longer needed** at run time on Linux.
- (0.14.0+010) **The main window's own Esc handler** added in `0.14.0+008`. Since `0.14.0+009` the global
  keyboard hook sees Esc in Tagent's own windows too, so it was redundant.

### Fixed
- (+021) **The About tab's links did nothing on Wayland.** The browser was started without
  the Wayland display the app hides from itself to run on XWayland, and a browser that
  can't use X11 (Snap's Firefox) quietly failed to start. It now gets the display back.
- (+016) **The input box didn't have the keyboard focus when the window opened.** After
  a click on a button (⇄, 📋, Translate, ...) the focus stayed on that button, also after
  hiding the window to the tray and showing it again, so typing went nowhere until the
  input box was clicked. Every show of the window (at startup, from the tray, from a
  second start) now puts the focus in the input box and asks the window manager to
  activate the window.
- (+015) **Ctrl+V, Ctrl+C, Ctrl+X, Ctrl+A and Ctrl+Z did nothing in a non-Latin
  keyboard layout.** With a Russian (Greek, Hebrew, ...) layout active, the input box and
  the Settings fields ignored these shortcuts: Slint recognizes them by the character the
  key types (`м`, not `v`). A key that types a non-Latin character while Ctrl is held now
  counts as the Latin letter at its place on a US keyboard, as in browsers; Latin layouts
  (Dvorak included) are unchanged.
- (+011) **The main window kept its place only the first time it was shown.** Shown
  again from the tray (or by starting `tagent-gui` again) in the same run, it opened in
  the middle of the primary screen: hiding to the tray unmaps the window and the window
  manager places it anew, and the saved position was applied only at the first show.
  Now the window comes back with the size and position it was hidden with, every time,
  also with "Remember window size and position" off (that setting now only decides
  whether they are kept for the next start). A plain minimize is left to the desktop as
  before.
- (+010) **The menu showed a gear instead of the icon** after `--install-desktop` (GNOME): the
  shell read the new menu entry before it noticed the new icon, and kept the gear until
  the next session. The icon theme folder and the entry are now touched after writing, so
  GNOME picks both up at once; running `--install-desktop` again repairs an existing gear.
- (+008) **The 📋 button no longer makes GNOME ask for remote desktop access** on Wayland.
  It simulated Ctrl+C before reading the clipboard, which on a Wayland session goes
  through the remote desktop portal. The button now only reads the clipboard, on every
  platform. It never copied anything from other apps anyway: the click had already
  focused Tagent's window.
- (+007) **The hotkeys bound on Wayland are shown in English**: Settings > Hotkeys & Tray
  and the transcript header showed the desktop's own description, which GNOME localizes
  ("Нажмите <Alt>a" on a Russian system). The key is now taken out of it and written the
  way Tagent writes hotkeys (`Alt+A`).
- (0.14.0+034) **Windows redraw glitches caused by the software renderer**. It redraws only what changed
  since its last frame and isn't told when Windows discards a window's contents, so:
  translating the same phrase with the hotkey again left the popup invisible; the main
  window shown again from the tray came back with its language row blank; and the main
  window restored from the taskbar was drawn slowly and only partly.
- (0.14.0+017) **Hotkey copy with a non-Latin keyboard layout on Linux**: the simulated Ctrl+C no
  longer goes through `xdotool`, which switched the XKB group (keyboard layout) to the Latin
  one and back for every key it sent; with e.g. the Russian layout active the hotkey took up
  to several seconds to copy the selection. Ctrl+C is now sent directly through the X11
  XTest extension by physical key, without touching the layout. It first waits for the
  hotkey's own key to be released (while it's held, the hotkey's key grab would swallow the
  Ctrl+C), then releases only the modifiers that are actually still held. With a non-Latin
  layout the application receives the same Ctrl+C as when pressed by hand (Ctrl plus that
  layout's letter on the C key).
- (0.14.0+011) **Hang on a keyboard-layout switch (Windows)**: switching the layout could freeze
  `tagent-gui` for good, most readily with switchers that broadcast
  `WM_INPUTLANGCHANGEREQUEST` to all windows. The cause was a deadlock between the UI thread
  and the NVIDIA OpenGL driver's own thread over the IMM lock. `tagent-gui` now uses Slint's
  software renderer on Windows, which never loads the OpenGL driver; setting `SLINT_BACKEND`
  (e.g. `winit-femtovg`) still overrides it.
- (0.14.0+009) **Global hotkeys work while the Tagent window is focused (Windows).** With the main window
  focused, Alt+A / Alt+S just typed "a" / "s" into the input field, because the global keyboard
  hook received no key events at all there. Cause: Slint's winit backend registers the keyboard
  for raw input, and while a window of that process is in the foreground its own low-level hook
  gets nothing. `tagent-gui` now drops that raw keyboard registration right after creating its
  window (window keyboard input is unaffected), so both hotkeys, and Esc for speech, now work
  in Tagent's own windows too -- e.g. select text in the input field and press Alt+A.
- (0.14.0+008) **Esc now stops speech while the main window is focused on Windows.** Pressing Esc during
  playback started by a transcript speaker button (or by the speech hotkey) did nothing while the
  main window had focus, because the global keyboard hook receives no keystrokes aimed at
  `tagent-gui`'s own window. The main window now handles Esc itself as well; the global hook still
  covers every other window. Esc still reaches the focused control as before.
- (0.14.0+006) **Right-click copy in the popup, redesigned** -- `0.14.0+005`'s version (below) didn't work as
  built: with the menu on, it opened as soon as the popup appeared instead of on right-click, and
  was clipped out of view on a small popup; with it off, right-click copied nothing. Replaced
  with a simpler design instead of debugging the broken one further: no menu at all any more
  (the "Show menu on right-click" setting no longer affects the popup), right-click a specific
  line (phrase or translation) to copy just that one, same border-flash feedback as before.
- (0.14.0+001) **Transcript now scrolls to the newest entry**: adding an entry left the view one entry short
  of the bottom, because the scroll was computed from the content height before the new row had
  been laid out. It is now done in `app.slint` from `changed` handlers on the transcript's
  content and visible heights, so the view also stays at the end when the input box is resized or
  the text re-wraps. Covered by a headless regression test (`i-slint-backend-testing`, a new
  dev-dependency).

### Security
- (0.14.0+021) **`tagent-gui.json` is written with permissions `0600`** on Linux/macOS (on creation and
  on every save, tightening an existing file), since it can now hold API keys.

### Known issues
- (0.14.0+034) The `0.14.0+011` layout-switch hang can come back on machines with an NVIDIA OpenGL
  driver, with keyboard-layout switchers that broadcast `WM_INPUTLANGCHANGEREQUEST` to
  all windows (ccaps 0.10.1 and later post it to the foreground window only). If it
  happens, start `tagent-gui` with `SLINT_BACKEND=winit-software`.

## [0.14.0] - 2026-09-20

### Added
- **Windows executable icon**: `tagent-gui.exe` now carries the Tagent "a" icon
  (`assets/icons/tagent-gui.ico`, 16-256 px, made from `tray.png`) plus version info
  (product name, description, original filename), so Explorer, shortcuts and a pinned
  taskbar button no longer show the generic executable icon. Embedded by `build.rs` through
  the `winresource` build-dependency (Windows targets only). The window and tray icons are
  unchanged.

### Changed
- **Main window header**: the transcript header now reads `=== Tagent v<version> ===` and
  lists the active hotkeys (`Translation: Alt+A`, `Speech: Alt+S`), matching `tagent-cli`'s
  startup banner. Only hotkeys that were actually registered at startup are listed (a
  disabled or invalid one is left out), and the speech line also follows "Enable text-to-speech".
- **Windows: no terminal window on launch.** `tagent-gui.exe` is now a GUI-subsystem
  executable (`windows_subsystem = "windows"`, debug and release alike), so starting it from
  Explorer, a shortcut or autostart no longer opens a console behind the app. When it is
  started from a terminal instead, `platform::windows::console::attach_parent()` (new, uses
  `AttachConsole`) reconnects `println!`/`eprintln!` to that terminal, and leaves a stream
  alone if the caller redirected it (`tagent-gui.exe 2> log.txt`). Nothing changes on Linux
  and macOS. `cmd` and PowerShell don't wait for a GUI-subsystem program, so run it as
  `.\tagent-gui.exe | Out-Host` (or `cargo run -p tagent-gui`) to keep the output in order;
  see the README.
- The three provider dropdowns in Settings > "General" are now filled from the `tagent`
  library's own name lists (`TRANSLATION_PROVIDERS`/`DICTIONARY_PROVIDERS`/
  `SPEECH_PROVIDERS`, `tagent` `0.18.1`) each time the dialog opens, instead of lists
  hardcoded in `app.slint`. A backend added to `tagent` is offered in Settings
  automatically; the lists in `app.slint` are only placeholders now.
- **Now publishable to crates.io**: dropped `publish = false` and added the package metadata
  (license, authors, repository, keywords, categories); the `tagent` dependency carries a
  `version` next to its `path`. `README.md` rewritten: it still described a translate-only
  prototype with no dictionary, text-to-speech, hotkeys or tray.

## [0.14.0+051] - 2026-09-20

### Added
- **Dictionary provider and Speech provider dropdowns** on Settings > "General",
  beside the existing Translate provider one (now a label/dropdown grid, so the tab
  doesn't grow by two lines per axis). They set `dictionary_provider` and
  `speech_provider`, which were hand-editable only until now; like the other General
  settings they apply to the next translation/speech immediately, no restart. Only
  `google` is listed for each today. "Reset to Defaults" covers them.
  A hand-edited name the dropdown doesn't list is shown as the first entry and replaced
  by it on the next Settings save (same as `translate_provider` always did).

## [0.14.0+050] - 2026-09-20

### Added
- **`dictionary_provider`** (`tagent-gui.json`, default `"google"`, live-reloaded,
  hand-editable only — no Settings dropdown while `"google"` is the only backend), the
  dictionary counterpart of `translate_provider`/`speech_provider`. A Settings save
  carries it through unchanged.

### Changed
- **Dictionary lookup now has its own, independent provider** (`DictionaryProvider`
  in the `tagent` library, `0.18.0`; Stage 12): `get_dictionary_entry` moved off
  `TranslationProvider` onto a new `DictionaryProvider` trait with a
  `create_dictionary_provider()` factory. No user-visible change. The provider is built
  only when a single-word lookup is actually about to happen, and a bad
  `dictionary_provider` value never breaks translation: it logs
  `Dictionary provider unavailable (...)` to stderr and the word gets a plain translation.
  The dictionary formatting tests build their fixtures with the new
  `DictionaryEntry::new`/`PartOfSpeechEntry::new`/`Definition::new` constructors, since
  those structs are now `#[non_exhaustive]`.

## [0.14.0+049] - 2026-09-19

### Added
- **Draggable popup**: the hotkey popup can be moved with the mouse by dragging
  any part of it (the cursor turns into a move cursor while the button is held). Dragging restarts the auto-hide
  countdown, and the popup no longer auto-hides while the button is held.
- **`remember_popup_position`** (`tagent-gui.json`, default `false`,
  live-reloaded; checkbox "Remember position after dragging" on Settings >
  "Popup"): when on, the position where the popup was dropped is saved
  (`popup_position`) and later popups reappear there instead of next to the
  mouse cursor. With it off (the default) the popup is still draggable, but
  each new popup opens next to the cursor as before. The saved position is
  clamped back onto the current desktop before use, so a changed monitor layout
  can't strand the popup off-screen (`platform::window::virtual_screen_bounds`,
  new on Linux/Windows; unclamped on macOS).

## [0.14.0+048] - 2026-09-19

### Changed
- **Text-to-speech uses its own provider, independent of `translate_provider`**
  (Stage 11, `SpeechProvider`): `speech::speak()` now takes a
  `&dyn tagent::providers::SpeechProvider` instead of a `TranslationProvider`
  (`split_for_speech`/`speak_chunk` moved to the new trait in the `tagent`
  library, shared with `tagent-cli`). `start_speaking` builds the speech
  provider from the new `speech_provider` field and only constructs a
  translate provider when the source language is `"auto"` (for
  `detect_language`) — a concrete language no longer depends on
  `translate_provider` at all, and a translate provider that fails to construct
  on the `"auto"` path falls back to `"en"` with a warning instead of failing
  speech. Saving Settings preserves a hand-edited `speech_provider` rather than
  resetting it.

### Added
- **`speech_provider` `tagent-gui.json` field** (default `"google"`,
  live-reloaded, hand-editable): selects the text-to-speech backend. No Settings
  dropdown yet — with `"google"` the only registered backend it would have a
  single entry, same precedent as `translate_provider` shipping hand-editable for
  two stages before Settings grew a dropdown for it.

## [0.14.0+047] - 2026-09-18

### Added
- **Speech hotkey** (Stage 10 follow-up): a second global hotkey (default
  `Alt+S`, Linux and Windows only — macOS is a stub) that speaks the current
  text selection directly, with no translation step, via a new
  `[Speech]: ...` transcript row (with its own working 🔊 replay button). Esc
  cancels playback unconditionally — from any application, not just Tagent's
  own window — and this also cancels a transcript speaker button's playback,
  not just the hotkey's, since both share one "who's currently speaking"
  mechanism. New `speech_hotkey`/`enable_speech_hotkey` `tagent-gui.json`
  fields (defaults `"Alt+S"`/`true`, restart-required like `translate_hotkey`),
  with a second hotkey field + Record button + checkbox on Settings >
  "Hotkeys & Tray". `KeyboardHook::spawn` on Linux/Windows/macOS all grew a
  second hotkey slot plus an Escape-observation callback; Escape is never
  suppressed system-wide, only observed alongside the existing hotkey
  detection.

## [0.14.0+046] - 2026-09-18

### Added
- **Text-to-speech playback** (Stage 10): every transcript row gets its own
  🔊 speaker buttons — one for the original phrase, one for the translation
  (hidden for a failed translation, and for a Stage 9 dictionary hit reads
  only the primary translation line, never the full part-of-speech/synonym
  block). Clicking a button plays audio through the default output device via
  a new `tagent-gui/src/speech.rs` module (duplicated from `tagent-cli`'s own
  playback code, independent of it) and turns into a ⏹ that stops playback on
  a second click; only one button plays at a time app-wide, with every other
  row's button disabled meanwhile. New `enable_text_to_speech` `tagent-gui.json`
  field (default `true`, live-reloaded — no restart needed), with a matching
  checkbox on Settings > General. New `rodio` dependency (works on all three
  platforms, unlike the hotkey/popup/tray features — no OS-specific code
  needed).

## [0.14.0+045] - 2026-09-18

### Added
- **Dictionary lookup for single-word input** (Stage 9): typing or selecting a
  single word now shows definitions grouped by part of speech instead of a
  plain translation, with a spelling-correction notice when the provider
  silently corrected a misspelling — in both the main transcript and the
  hotkey-triggered popup. New `tagent-gui/src/dictionary.rs` module
  (duplicated from `tagent-cli`'s own dictionary formatting, independent of
  it). Two new `tagent-gui.json` fields, `show_dictionary` and `spell_check`
  (both default `true`, live-reloaded — no restart needed), with matching
  checkboxes on Settings > General.

## [0.14.0+044] - 2026-09-17

### Changed
- **Raised the popup's default border width from 0 to 1**
  (`default_popup_border_width` in `config.rs`).

## [0.14.0+043] - 2026-09-17

### Added
- **"Border width (px)" setting for the popup** (Popup tab, default `0` — no
  border): new `popup_border_width` config field, applied to the popup's
  outer panel `border-width` (`app.slint`); border color stays theme-derived
  (`Palette.border`) regardless.

### Fixed
- **Popup showed a thick colored margin around its content regardless of
  border settings** (`app.slint`): the outer panel `Rectangle`'s
  `background` was hardcoded to the raw theme color
  (`panel-background-theme-default`), never updated to track
  `popup-background` once that became Color-scheme-aware. Since
  `content-layout` has 8px padding around the inner phrase/translation
  boxes, that padding showed through in the *raw theme* color whenever it
  differed from the active scheme's color — reading as a border no matter
  what `popup_border_width` was set to. Fixed by binding the outer
  Rectangle's `background` to `popup-background` directly, so the whole
  popup (including the padding margin) renders as one consistent color.

## [0.14.0+042] - 2026-09-17

### Fixed
- **Scrolling the mouse wheel over the popup while its content already fit
  the max size closed it** (`app.slint`): a `Flickable` (which `ScrollView`
  wraps, added for the max-height/scrolling feature) grabs the mouse for the
  duration of any wheel gesture, which left `touch-area.has-hover` stuck at
  `false` afterward until the next real pointer motion — read by
  `hide-timer` as "the user left," closing the popup out from under someone
  who'd just scrolled and stopped to read. `touch-area` now wraps `ScrollView`
  (was a plain sibling) and has its own `scroll-event` handler that restarts
  the auto-hide timer on any wheel input it sees — including one the child
  `Flickable` itself ignored because there was nothing to scroll, which is
  exactly the case this bug showed up in (`ScrollView`'s own `scrolled`
  callback only fires on an actual viewport change, never true when content
  already fits, so it couldn't have been used for this). Verified with a
  throwaway example harness directly wiring `hide-requested` and polling
  `has-hover`: continuous scrolling every 0.5s over non-overflowing content
  no longer fires `hide-requested`, even though `has-hover` itself still
  goes stale after each scroll. No Rust-level test coverage — this is a
  `.slint` hit-testing/event-propagation defect with no surface in `main.rs`
  or `config.rs`.

## [0.14.0+041] - 2026-09-17

### Changed
- **Popup width now shrinks to fit short text instead of always sitting at
  `popup_max_width`** (`app.slint`): width is bound to the natural
  (unwrapped) single-line width of the longer of the phrase/translation
  text, plus a fixed horizontal-chrome allowance, clamped between a 150px
  floor and `popup_max_width` — text only wraps once its natural width
  would exceed the max. Uses each `Text`'s own `preferred-width`, which
  reflects its natural unwrapped size regardless of the `wrap`/`width` it's
  actually given. Since the phrase line is conditionally removed from the
  tree entirely when "Show phrase" is off (`if show-phrase: Rectangle {...}`),
  added an always-present, invisible `phrase-measure` twin Text so its width
  can still be queried (and correctly excluded from sizing) in that case.
  Verified with a throwaway example harness across short text, text long
  enough to hit the max-width wrap, and a hidden phrase.

## [0.14.0+040] - 2026-09-17

### Changed
- **Raised the popup's default max width/height from 360×400 to 600×600**
  (`default_popup_max_width`/`default_popup_max_height` in `config.rs`).

## [0.14.0+039] - 2026-09-17

### Added
- **"Max width (px)" / "Max height (px)" settings for the popup** (Popup
  tab): the popup window no longer grows unbounded with long translations.
  Width is now always exactly `popup_max_width` (text wraps to fit, same as
  the old hardcoded 360px, just configurable — default unchanged at 360).
  Height grows with content up to `popup_max_height` (new, default 400) and
  then becomes scrollable instead of growing further, via a `ScrollView`
  now wrapping the popup's content layout in `app.slint`. Verified with a
  throwaway example harness instantiating `TranslationPopup` directly with
  long phrase/translation text and a small max height, confirming the
  window stays capped at the configured size and the overflow scrolls.

## [0.14.0+038] - 2026-09-17

### Changed
- **Renamed the "Color scheme" placeholder from "Presets…" to "Custom"**
  (View tab): clearer now that "Default" is itself a real, selectable entry
  in the same list — "Custom" makes it obvious this one just means "your
  current colors don't match any preset," rather than looking like another
  variant of "Default".

## [0.14.0+037] - 2026-09-17

### Changed
- **Popup's "Theme default" now references the transcript's Translation
  color, not Phrase** (`apply_popup_style` in `main.rs`): `scheme_default_fg`/
  `scheme_default_bg` resolve against `config.translation_color`/
  `translation_background` instead of `phrase_color`/`phrase_background`,
  per explicit request. (In every built-in Color scheme preset the two
  backgrounds are identical, so this only visibly changes the resolved text
  color, not the background.)

## [0.14.0+036] - 2026-09-17

### Changed
- **Reworked +035's Popup "Color scheme" picker into an automatic fallback
  instead of a separate dropdown**: the independent "Color scheme" combo box
  added to the Popup tab in +035 is removed. Instead, the popup's existing
  "Theme default" checkboxes (Text color/Background) now resolve against the
  transcript's own `phrase_color`/`phrase_background` first (`apply_popup_style`
  in `main.rs`), falling back further to the raw Palette theme colors only
  when the transcript itself is also just following the theme. An unmodified
  popup now automatically matches whatever Color scheme is active on the
  View tab, with no picker of its own to keep in sync.

## [0.14.0+035] - 2026-09-17

### Added
- **"Default" entry in the "Color scheme" preset list** (View tab): resets
  the transcript's background/phrase/translation colors back to ""
  (theme-following) and Theme to Auto — same shape as every other named
  preset, just with no fixed colors of its own. `ColorScheme`'s `dark: bool`
  field became `theme: &'static str` (`"auto"`/`"light"`/`"dark"`) to
  represent it.
- **Independent "Color scheme" picker for the Popup tab**: a second combo
  box, using the same preset list, that fills the popup's own Text
  color/Background (from each scheme's `phrase_color`/`phrase_background`)
  and switches Theme to match, without touching the transcript's colors.
  Defaults, each time Settings opens, to whichever scheme is currently
  active for the transcript (not the popup's own saved colors) — e.g. if the
  transcript is on "Ayu Dark", the Popup tab's dropdown starts on "Ayu Dark"
  too, but can be changed independently from there (a dark transcript with a
  light popup, for instance).

## [0.14.0+034] - 2026-09-17

### Added
- **"Show prompt" / "Show phrase" toggles for the popup** (Settings > "Popup"
  tab): the popup can now hide its "[Auto]:"/"[Russian]:"-style prompt, or
  the original phrase line entirely (showing only the translation),
  independently of the transcript's own "Show prompt" (View tab). New
  `GuiConfig` fields `popup_show_prompt`/`popup_show_phrase`, both default
  `true` (today's behavior unchanged out of the box). Required threading raw
  (un-prompt-formatted) phrase/translation text through to the popup — it
  previously only ever received the transcript's already-formatted
  `TranscriptEntry` strings, which baked in the transcript's own prompt
  setting and always included the phrase — via a new `TranslationOutcome`
  passed alongside `TranscriptEntry` to `spawn_translation`'s `on_done`
  callback. `TranslationPopup`'s phrase line is now wrapped in an
  `if show-phrase:` block in `app.slint`, so hiding it also shrinks the
  popup window (its height is bound to the content layout's preferred size).

## [0.14.0+033] - 2026-09-17

### Added
- **Independent style settings for the hotkey-triggered popup** (Settings >
  new "Popup" tab): Font, Size, Text color, and Background, plus the
  "Auto-hide (seconds)" setting moved here from "Hotkeys & Tray". Previously
  the popup (`TranslationPopup` in `app.slint`) just reused the transcript's
  `phrase_font`/`translation_font`/etc. style — including two different text
  colors for the phrase and translation lines. The popup now has its own
  `GuiConfig` fields (`popup_font`, `popup_size`, `popup_color`,
  `popup_background`) applying one shared style to both lines instead.
  Defaults to `"monospace"`/13px/`""` (theme default), same as the
  transcript's own defaults, and "theme default" resolves live against
  whichever theme is active — including a running `Auto` toggle, now that
  `theme_poll_timer` re-applies `apply_popup_style` alongside `apply_style`.

## [0.14.0+032] - 2026-09-17

### Fixed
- **Transcript panel didn't follow a live OS theme switch under `Auto`**
  (`main.rs`): `apply_style` only ran at startup and on the window's first
  `show()` (see `0.14.0+031` above). If the user changed the system's
  dark/light preference while `tagent-gui` was already running, Palette-bound
  elements (the input bar's frame, field colors, the OS window decorations)
  updated immediately on their own, but the baked snapshots `apply_style`
  writes (`panel-background`, phrase/translation colors) stayed frozen at
  whatever they'd last resolved to — leaving the transcript panel visibly out
  of sync with the rest of the window. Fixed with a 1s repeating
  `slint::Timer` that re-calls `apply_style` for as long as `theme` is
  `"auto"`; a no-op when nothing has actually changed.

## [0.14.0+031] - 2026-09-17

### Fixed
- **Transcript header unreadable under `Auto` theme on Linux** (`main.rs`):
  `apply_style` snapshots several `Palette`-derived colors (`panel-background`,
  phrase/translation text and background colors) into plain, non-live
  properties at startup. Because winit doesn't deliver Linux system theme
  detection synchronously, that single call could land before the real
  scheme resolved and permanently bake in the wrong colors — unlike
  `panel-foreground`, which is live-`Palette`-bound and self-corrects, so the
  header text could end up unreadable (e.g. light text baked in against a
  background that a moment later resolves dark) rather than just flashing
  for a frame. Fixed by calling `apply_style` a second time via a deferred
  `slint::Timer::single_shot` ~150ms after the first call, the same settle
  delay `show_window_restoring_geometry` already uses for the analogous
  winit/X11 sizing race.

## [0.14.0+030] - 2026-09-16

### Added
- **"Reset to Defaults" button** (General tab): fills every setting on
  every tab — provider, theme, colors, fonts, hotkey, popup delay,
  start-minimized, remember-window-geometry — with `GuiConfig::default()`'s
  values, in-memory only. Nothing is written to `tagent-gui.json` until OK
  is clicked; Cancel discards the reset like any other in-session edit.
  Saved window geometry (`window_geometry`) is deliberately not part of
  this — it's captured window state, not a user-facing setting.

## [0.14.0+029] - 2026-09-16

### Fixed
- **Settings dialog's "Cancel" button did nothing** (`app.slint`, `main.rs`):
  a `StandardButton`'s `kind` only controls its label and platform-specific
  position, not any auto-close behavior — unlike "OK", which explicitly
  saves and calls `dialog.hide()` from its own `clicked` handler, "Cancel"
  had no handler at all, so clicking it was a no-op (the dialog stayed
  open, and the only way to close it without saving was the window's own
  OS-level close button). Added a `cancel-requested` callback, fired from
  "Cancel"'s `clicked`, that just calls `dialog.hide()` — discarding
  whatever was changed in the current dialog session without touching
  `tagent-gui.json`, same as clicking OK does but for saving.

## [0.14.0+028] - 2026-09-16

### Fixed
- **Recording a hotkey close to (or the same as) the currently-active one
  captured `Ctrl+C` instead** (`main.rs`): the global hotkey kept firing for
  real while the Settings dialog's "Record" capture was open. Its own
  `ClipboardManager::get_text_with_copy` step simulates a real Ctrl+C
  keypress to grab the current selection — which, since the Settings
  dialog still held keyboard focus during recording, landed right back on
  the "Record" capture itself, overwriting whatever key was actually
  pressed with `"Ctrl+C"`. The global hotkey now suppresses itself while a
  recording is in progress (self-clearing after 30s if the dialog is ever
  closed uncleanly mid-recording, so it can't get stuck disabled). Residual
  limitation, not fixed here: the exact key combination that's already the
  *active* global hotkey is grabbed system-wide (`XGrabKey` on Linux) before
  it ever reaches the Settings window, so recording that exact combination
  still won't register anything — type it manually instead, or pick a
  different hotkey first.

## [0.14.0+027] - 2026-09-16

### Changed
- **Default `translate_hotkey` changed from `Alt+Q` to `Alt+A`** (`config.rs`):
  applies to `GuiConfig::default()` and the fallback used when an existing
  `tagent-gui.json` omits `translate_hotkey`. Existing config files with an
  explicit `translate_hotkey` value are unaffected.

## [0.14.0+026] - 2026-09-16

### Fixed
- **`HotkeyParser::validate_hotkey` didn't restrict double-press hotkeys at
  all** — `"Q+Q"`, `"A+A"`, `"5+5"`, or any other ordinary key doubled was
  silently accepted as valid, even though the same key pressed *once* alone
  (`"Q"`) was correctly rejected (single keys are F1-F12 only). Found via
  the new "Record" button, which makes it easy to double-press an ordinary
  letter by accident. Double-press is now restricted the same way single
  keys already were: only F1-F12 or a modifier key (Ctrl, Alt, Shift, Win)
  may be double-pressed — matching the documented examples (`Ctrl+Ctrl`,
  `F8+F8`, `Shift+Shift`, `Alt+Alt`). Covers both the manually typed field
  and the "Record" button, since both go through the same validator.

## [0.14.0+025] - 2026-09-16

### Added
- **"Record" button for the global hotkey field** (Stage 8 follow-up):
  press it and then press the actual key combination instead of typing the
  string by hand — captures via Slint's `FocusScope`, converts the key
  press(es) into the same string grammar the field already accepts, and
  writes it into the field (still editable manually afterward, same
  validation as before). Supports plain keys, modifier combos, and
  double-press patterns (press the same bare key twice quickly, e.g.
  `F8+F8`); Escape cancels recording. A key the app doesn't recognize
  (most commonly a non-Latin keyboard layout) shows a dedicated message
  suggesting a Latin layout or manual entry, instead of a generic parser
  error.

## [0.14.0+024] - 2026-09-16

### Added
- **Settings controls for the global hotkey and popup auto-hide delay**
  (Stage 8): the "Hotkeys & Tray" tab now has a text field for
  `translate_hotkey` (Stage 5) with inline validation — an invalid hotkey
  string shows the parser's own error message and disables the dialog's OK
  button — and a spinbox for `popup_auto_hide_seconds` (Stage 6, `0` means
  "use the default, 3s"). Both fields previously required hand-editing
  `tagent-gui.json`. A new hint at the bottom of the tab clarifies that the
  hotkey change needs a restart to take effect, while the popup delay and
  the existing "Start minimized"/"Remember window size and position" toggles
  don't.

## [0.14.0+023] - 2026-09-15

### Fixed
- **"Remember window size and position" didn't actually restore anything when
  `start_minimized` was on** (the default): the restore only ever worked when
  the window was shown at startup, since the very first `show()` — which is
  when the restore runs — normally happens from the tray's "Show Tagent"
  click instead, delivered via a D-Bus/ksni callback while the event loop is
  already running. In that context the synchronous `set_size`/`set_position`
  calls were silently dropped and the window stuck at whatever size the
  window manager's own initial placement negotiation picked, no matter how
  long it was left open. Confirmed live by driving the tray icon's
  `org.kde.StatusNotifierItem.Activate` directly over D-Bus. Fixed by
  re-issuing the same `set_size`/`set_position` calls a second time, ~150ms
  later via `slint::Timer::single_shot`, after the window manager's initial
  negotiation has had a chance to settle.

## [0.14.0+022] - 2026-09-15

### Added
- **Remember window size and position**: `remember_window_geometry` setting
  (`tagent-gui.json`, default on, editable via a new checkbox in Settings >
  "Hotkeys & Tray"). When on, the main window's position/size are saved when
  it's hidden to the tray or the app quits, and restored the next time it's
  shown (once per run — a later re-open from the tray leaves the window
  exactly as you last had it).

### Fixed
- **Real, intermittent bug found while testing the above**: on this
  project's X11/mutter setup, the main window occasionally (observed ~2 of 7
  fresh launches) opened at a much smaller size than its configured
  480×480 default — a winit/X11 initial-window-sizing race, not tied to the
  new geometry feature (reproduced even with it fully bypassed). Fixed by
  explicitly re-asserting the intended size right after `.show()` whenever
  there's no saved geometry to restore instead — 6/6 clean runs after the
  fix, 0/7 before it.

## [0.14.0+021] - 2026-09-15

### Changed
- Default main window size changed from 560×420 to 480×480 (now square). Still
  resizable; only the launch-time default changed.

## [0.14.0+020] - 2026-09-15

### Added
- **System tray integration (Stage 7)**: a persistent tray icon (`TrayIcon` in
  `app.slint`, built on Slint's own `SystemTrayIcon` element — no new crate
  dependency) with "Show Tagent" / "Settings…" / "Quit" entries; left-clicking
  the icon also shows the window.
- `start_minimized` field in `tagent-gui.json` (default `true`) controls
  whether the app launches straight into the tray or with the window shown;
  editable via a new checkbox in Settings > "Hotkeys & Tray". Read once at
  startup, restart required to take effect, same as `translate_hotkey`.

### Changed
- **Closing the main window (the OS close button) now hides it to the tray
  instead of quitting the app.** "Quit" in the tray menu is the only way to
  fully exit from now on. This is a real behavior change for every existing
  user, not just a new opt-in feature.

### Dependencies
- `slint`/`slint-build` bumped 1.14.1 → 1.17.1 (minimum version with
  `SystemTrayIcon`). Building `tagent-gui` now additionally requires the
  `fontconfig` development package (`libfontconfig1-dev` on
  Debian/Ubuntu) to be installed on the system, via `pkg-config` — a
  transitive requirement of the upgraded font-matching stack, not a new
  direct dependency of this project.

## [0.14.0+019] - 2026-09-14

### Fixed
- **Global hotkey suppression on Linux depended on the active keyboard
  layout** (`platform/linux/xgrab.rs`): `XGrabManager` resolved the hotkey's
  base key to an X11 keycode by converting it to an ASCII/Latin `KeySym` and
  calling `XKeysymToKeycode`, which searches the *currently active* keyboard
  mapping across all groups. On a layout with no Latin group at all (e.g. a
  pure Russian layout, as opposed to a combined `us,ru` one), the Latin
  keysym isn't bound to any keycode, `XKeysymToKeycode` returned 0, and the
  grab silently failed — hotkey *detection* (via `rdev`, already
  keycode-based, so already layout-independent) kept working, but the
  keystroke was no longer *suppressed*: it leaked through into whatever
  application had keyboard focus instead of being consumed by `tagent-gui`.
  Replaced `vk_to_keysym` + `XKeysymToKeycode` with `vk_to_x11_keycode`, a
  direct VK-code-to-hardware-keycode table using the same standard
  evdev-based keycode numbering `rdev`'s own internal Linux backend already
  keys off of for detection (cross-checked against `rdev` 0.5.3's own
  table) — a hardware keycode identifies a physical key position, not a
  character, so the grab no longer depends on which keysym the active
  layout/group binds to that position. Detection and suppression now agree
  on the exact same physical-key identification, regardless of keyboard
  layout. Same fix ported independently to `tagent-cli`'s copy of this file
  (see [`tagent-cli/CHANGELOG.md`](../tagent-cli/CHANGELOG.md)).

## [0.14.0+018] - 2026-09-14

### Fixed
- Popup was invisible after the `+017` position fix (correctly placed next to
  the cursor, but never actually seen). Cause: focus was restored to the
  previous app *immediately* after showing the popup, and on Linux that also
  raises the restored app (`XMapRaised`) — since the popup now sits right
  where the cursor is, i.e. right over the app just used, raising that app
  put it right back on top of the popup. Fixed by deferring the focus
  restore to when the popup actually hides (matching `tagent-cli`'s own
  `hide_terminal_and_restore`), instead of doing it right after `show()`.
  Reopens a narrower version of the focus-stealing edge case from Stage 6's
  original design (a hotkey re-trigger *while the popup is still visible*
  can copy from the popup instead of the real source app) — accepted, same
  tradeoff `tagent-cli` already lives with for its own popup.

## [0.14.0+017] - 2026-09-14

### Fixed
- Stage 6 popup appeared at the screen's top-left corner instead of next to
  the mouse cursor. Cause: `Window::set_position` was called *before*
  `popup.show()`, and on this X11 setup that call is silently ignored when no
  OS-level window exists yet. Fixed by positioning *after* `show()` instead.

## [0.14.0+016] - 2026-09-14

### Added
- Popup window (Stage 6 of the development plan): the global hotkey (Stage 5)
  now also shows a small, no-frame, always-on-top popup next to the mouse
  cursor with the just-translated phrase and translation, in addition to the
  existing transcript update. Purely informational — no buttons, no
  click-to-dismiss — it closes on its own after a configurable delay
  (`popup_auto_hide_seconds` in `tagent-gui.json`, default `3`, live-reloaded;
  `0` is treated as the default rather than "never auto-hide", since this
  popup has no manual close affordance) unless the cursor is resting over it,
  in which case it keeps re-checking once a second until the cursor leaves.
  Hotkey-triggered only — not shown for the main window's Translate
  button/Enter key. The popup never holds keyboard focus, even while visible:
  focus is captured before it's shown and handed back immediately after, so a
  second hotkey press while it's still on screen still copies from the real
  source application rather than from the popup itself.
- New per-OS `platform::window` module (`cursor_position`/`foreground_window`/
  `set_foreground_window`), trimmed from `tagent-cli`'s `WindowManager` and
  used only by the popup's cursor placement and focus save/restore — no new
  dependency, `tagent-gui` already had X11 (`xlib`)/`windows` crate access
  from Stage 4/5.

## [0.14.0+015] - 2026-09-13

### Added
- Global hotkey support (Stage 5 of the development plan): a configurable
  hotkey (default `Alt+Q`, same string format as `tagent-cli`'s
  `TranslateHotkey` — `F1`-`F12`, `Modifier+Key`, or `Key+Key` double-press)
  copies the current selection and translates it straight into the
  transcript, without needing the `tagent-gui` window focused first. New
  hand-editable `translate_hotkey` field in `tagent-gui.json` (default
  `"Alt+Q"`); a bad or dangerous value logs a warning and disables the
  hotkey rather than failing to start. Takes effect only on restart — no
  Settings UI for it yet. **Linux and Windows only**; macOS remains a stub,
  matching `tagent-cli`'s own posture for this feature. Implementation
  (`platform/{linux,windows,macos}/{keyboard,keycodes}.rs`, `xgrab.rs` on
  Linux, `HotkeyType`/`HotkeyParser` in `config.rs`) is ported independently
  from `tagent-cli`'s own hotkey code — no dependency on `tagent-cli`
  introduced.

### Known limitation
- The Windows implementation (`WH_KEYBOARD_LL` hook with Alt swallow-and-
  replay, mirroring `tagent-cli`'s own hard-won design) has been verified
  only via `cargo check`/`cargo test --no-run --target
  x86_64-pc-windows-gnu` from Linux — it compiles and links, including its
  unit tests, but has not been run on real Windows or under `wine` (neither
  available in the development environment). Treat as unverified in
  practice until confirmed on an actual Windows machine.

## [0.14.0+014] - 2026-09-13

### Added
- Clipboard support (Stage 4 of the development plan): a "📋" button next to
  Translate pulls the current system clipboard contents into the input box.
  Backed by a new per-OS `ClipboardManager` in `tagent-gui/src/platform/`
  (Linux: `arboard` + `xdotool`; Windows: `clipboard-win` + `SendInput`; macOS:
  stub, not yet implemented), ported independently from `tagent-cli`'s own
  `ClipboardManager` — no dependency on `tagent-cli` introduced. Runs off the UI
  thread so the window doesn't freeze during the underlying copy.

### Known limitation
- Because clicking "📋" gives `tagent-gui` window focus first, its simulated
  Ctrl+C targets `tagent-gui` itself rather than whatever was focused right
  before the click — unlike a future global hotkey, which wouldn't need to
  steal focus to fire. In practice this makes the button behave as "paste
  whatever the clipboard already holds," not "grab the current selection with
  no prior manual copy." See `docs/ARCHITECTURE.md` for the full explanation.
  Expected to be superseded once Stage 5 (global hotkeys) lands.

## [0.14.0+013] - 2026-09-12

### Added
- Seven more entries in the "Color scheme" preset picker (Settings > View):
  Tokyo Night, Catppuccin Mocha, Night Owl, Ayu Dark (dark), and GitHub
  Light, Gruvbox Light, Catppuccin Latte (light) — 14 presets total now.

### Fixed
- Reopening Settings always showed the "Presets…" placeholder in the "Color
  scheme" dropdown, even right after applying and saving one of the presets
  — each Settings dialog is a fresh instance, so its `color-scheme-index`
  never carried over from the previous session. It now shows the matching
  preset's name instead, whenever the five colors currently in effect
  (`background_color`/`phrase_*`/`translation_*` in `tagent-gui.json`)
  exactly equal one of the presets.

## [0.14.0+012] - 2026-09-12

### Added
- "Color scheme" preset picker in Settings > View: seven popular schemes
  (Solarized Dark, Solarized Light, Dracula, Nord, Gruvbox Dark, Monokai, One
  Dark). Picking one immediately fills in Background color, phrase text/
  background, and translation text/background to that scheme's colors and
  switches Theme to match (dark or light), so the window chrome and the
  transcript stay consistent. It's a one-shot bulk-fill, not a persisted
  setting of its own — the five color fields it sets remain individually
  editable afterward, same as if set by hand.

## [0.14.0+011] - 2026-09-12

### Added
- "Background color" setting in Settings > View: sets the shared background
  for both main panels (the transcript log and the input box), via the same
  "theme default or pick a color" field used for the phrase/translation
  colors. Persisted as `background_color` in `tagent-gui.json` (empty =
  follow the theme). Phrase/translation backgrounds left at "theme default"
  now follow this color too (custom or theme), so a custom app background
  and per-line "theme default" backgrounds always stay visually consistent
  instead of the latter secretly meaning "the raw, un-customized theme
  color".

### Fixed
- The input bar's own frame (behind the "[Auto]:" language prompt, the
  typing field, and the Translate button) shared the same `panel-background`
  property as the transcript log, so a custom "Background color" bled into
  the gaps around those controls and looked like they'd been recolored too.
  That frame now always uses the raw theme color instead; only the
  transcript log's reading area picks up the custom background.

## [0.14.0+010] - 2026-09-12

### Changed
- Reworked "Show prompt" (Settings > View, phrase/translation) back down to
  just a visibility toggle, dropping the separate prompt size/color settings
  added in +009. Slint's plain `Text`/`TextInput` can't mix two styles within
  one wrapped paragraph, so giving the prompt its own size/color required
  splitting it into a separately positioned element next to the text — which
  then either hanging-indented wrapped continuation lines under the text
  (rather than wrapping flush from the row's left margin like a normal
  paragraph) or, when top-aligned to fix a vertical mismatch at differing
  sizes, still didn't read as "one line" the way a single-style line does.
  The prompt is back to being part of the same string as the text (one
  font/color for the whole line, prompt included), toggled on/off by baking
  it into the string when the entry is created rather than rendered as its
  own styled element. `phrase_prompt_size`/`phrase_prompt_color` and the
  matching `translation_*` fields are gone from `tagent-gui.json`;
  `phrase_show_prompt`/`translation_show_prompt` remain.
- `TranscriptEntry` is back to one `phrase`/`translation` string per line
  (was briefly split into separate prompt/text fields in +009).

## [0.14.0+009] - 2026-09-12

### Added
- "Show prompt" setting in Settings > View, separately for the phrase and the
  translation: whether the `[Auto]:`/`[Russian]:`-style label in front of
  each line is shown at all (default: shown). Persisted as
  `phrase_show_prompt`/`translation_show_prompt` in `tagent-gui.json`. An
  entry with no prompt of its own (an error line) never shows one, regardless
  of this setting.

## [0.14.0+008] - 2026-09-12

### Added
- "Blocks spacing (px)" setting in Settings > View: controls the vertical gap
  between one phrase/translation pair and the next in the transcript, in
  pixels (default 20). Persisted as `block_spacing_px` in `tagent-gui.json`.
- "Phrases spacing (px)" setting in Settings > View: controls the vertical
  gap between a phrase and its own translation, within one pair (default 2).
  Persisted as `phrases_spacing_px` in `tagent-gui.json`.

### Fixed
- A phrase and its translation still had a visible gap between their text at
  `phrases_spacing_px: 0`, reading as a stray blank line. Cause:
  each row carried its own fixed vertical padding regardless of the spacing
  setting, so that padding put a floor under the gap no setting could remove.
  All vertical padding on the phrase/translation rows is now removed, so the
  spacing setting is the sole contributor to that gap and 0 means visually
  flush, whether the two rows share a background or not.

## [0.14.0+007] - 2026-09-12

### Fixed
- The HEX field in the phrase/translation color picker (Settings > View)
  didn't update while dragging the R/G/B sliders — it only reflected typed
  input, since Slint has no built-in way to format numbers as hex. Each
  slider now fires a `rgb-changed` callback that `main.rs` uses to reformat
  the field's current color into `"#RRGGBB"` and write it back into the HEX
  text as you drag.

## [0.14.0+006] - 2026-09-12

### Fixed
- The phrase/translation color-picker popup (Settings > View) closed itself on
  every slider drag instead of staying open, because `PopupWindow`'s default
  `close-policy` is `close-on-click` — any click, including one on a slider
  inside the popup, counted as "close". Set explicitly to
  `close-on-click-outside` so the popup only closes when clicking elsewhere.

## [0.14.0+005] - 2026-09-12

### Added
- Independent transcript styling for the phrase (original text) and its
  translation in the `View` tab: font family (Monospace/Sans Serif/Serif),
  font size, text color, and background color, each configurable separately
  for the two roles. Colors default to "Theme default" (follows the active
  theme) or can be set via a HEX field plus an in-app RGB-slider color
  picker. Persisted as `phrase_font`/`phrase_size`/`phrase_color`/
  `phrase_background` and the matching `translation_*` fields in
  `tagent-gui.json` (empty color string = theme default).

### Changed
- The transcript is now rendered from a structured list of entries (one
  phrase/translation pair each) instead of a single flat text blob, so the
  two lines can carry independent styling. Each line remains its own
  read-only, selectable/copyable text field; selecting text no longer spans
  across entries in one continuous drag the way the old single-blob
  transcript did.

## [0.14.0+004] - 2026-09-12

### Added
- `About` tab in the Settings dialog, showing the application name and current
  version (`env!("CARGO_PKG_VERSION")`).

## [0.14.0+003] - 2026-09-11

### Added
- Theme setting: a new `View` tab in the Settings dialog with `Auto`/`Light`/`Dark`,
  persisted as `theme` in `tagent-gui.json` (default `"auto"`, following the
  system setting). Applied live on save — no restart needed — to both the main
  window and any Settings dialog opened afterward.
- Full theme support, not just widget chrome: the transcript and input panels
  (previously hardcoded near-black regardless of system theme) now derive their
  colors from `Palette`'s semantic roles (`background`, `alternate-background`,
  `border`, `foreground`, `selection-*`, `control-*`), so `Light` actually looks
  light, not just the buttons/dropdowns/dialog frame.

### Changed
- `GuiConfig` gained a `theme` field (`#[serde(default)]`, so existing
  `tagent-gui.json` files without it still load fine, defaulting to `"auto"`).

## [0.14.0+002] - 2026-09-11

### Fixed
- Settings dialog no longer opens at an oversized default size with its OK/Cancel
  buttons stretched into full-height vertical bars — it had no explicit
  `preferred-width`/`preferred-height`, so it fell back to Slint's default window
  size with the small amount of real content pinned in a corner. Now sized
  explicitly (420×300) with OK/Cancel laid out as a normal row at the bottom.

### Added
- Settings dialog content is now organized into tabs (`General`, `Hotkeys & Tray`)
  instead of a single flat panel — laid out ahead of the settings that will
  populate the second tab in a later stage (hotkey string, start-minimized-to-tray,
  popup auto-hide delay), so adding a new settings category later is a new `Tab {
  }` block, not a redesign. `General` holds the one setting that exists today
  (`translate_provider`); `Hotkeys & Tray` is a placeholder pending that stage.

## [0.14.0+001] - 2026-09-11

### Added
- Settings dialog: a ⚙ button (top-right of the main window) opens a dialog to
  change `translate_provider` from a dropdown of known providers (currently just
  `"google"`), instead of hand-editing `tagent-gui.json`. OK saves and closes;
  Cancel (or the dialog's own close button) discards the change. The file stays
  hand-editable to any provider string regardless — the dropdown is a convenience,
  not a validation gate.
- `GuiConfigManager::update()`: applies a config change in memory immediately and
  persists it to disk, refreshing the tracked modification time so the write
  doesn't trigger a redundant reload on the next translation.

## [0.14.0+000] - 2026-09-11

### Added
- Own configuration file, `tagent-gui.json` (`~/.config/tagent-gui/tagent-gui.json`
  on Linux/macOS, `%APPDATA%\tagent-gui\tagent-gui.json` on Windows), holding
  `translate_provider`. Plain, pretty-printed JSON meant to be hand-editable — there
  is no Settings window yet. A missing file gets a fresh default written
  (`"google"`); a present-but-invalid file is left untouched on disk and the app
  falls back to its last valid in-memory config, logging a warning.
- Live-reload: the config file's modification time is checked before each
  translation, so a hand-edit takes effect on the next translation without
  restarting the app — no separate reload action needed.

### Changed
- Established `tagent-gui` as a fully independent application from `tagent-cli`:
  own interface, own configuration, own feature set, own versioning — built only
  on the `tagent` library. This file starts tracking notable changes from this
  point forward.
- Versioning now uses the same `MAJOR.MINOR.PATCH+BUILD` format and increment
  rules as `tagent-cli`, with its own independent counter (no shared version
  number, no `build.rs` auto-sync).

### Removed
- The inline `tagent-cli.conf` reader for `TranslateProvider`. **No migration
  path**: an existing `TranslateProvider` setting in `tagent-cli.conf` is no
  longer read by `tagent-gui` — set `translate_provider` in the new
  `tagent-gui.json` instead (it's created with the `"google"` default on first
  run if missing).

## [0.13.0] - 2026-08-05

### Added
- Initial `tagent-gui` prototype: Slint desktop GUI, translate-only, built directly
  on the `tagent` library. Not tracked by this changelog prior to this entry.
