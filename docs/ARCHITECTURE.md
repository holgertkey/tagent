# Architecture

This document goes deeper than `CLAUDE.md` on subsystems where a short summary would
hide important detail: the workspace's three-crate split, the platform abstraction
layer, the `tagent-gui` crate, and the build-time version-sync mechanics. Read
`CLAUDE.md` first for the high-level overview; come here when you need the mechanism,
not just the shape.

## Workspace layout: `tagent`, `tagent-cli`, `tagent-gui`

The root `Cargo.toml` is a virtual manifest (`[workspace] members = ["tagent",
"tagent-cli", "tagent-gui"]`, no `[package]` of its own) over three crates with a
one-directional dependency graph — both binaries depend on the library, never the
reverse:

```
tagent          (library)   -- providers (trait + Google Translate impl), languages
                                (name/code mapping), error (unified Error type). No
                                app/UI/platform code, no INI parsing, no config file
                                of its own.
tagent-cli      (binary)    -- today's application: hotkeys, interactive terminal, CLI,
                                config file, history, clipboard, platform integration.
                                Package name and [[bin]] name both "tagent-cli"
                                (tagent-cli/Cargo.toml), so the built executable lands
                                at target/release/tagent-cli. Depends on
                                tagent = { path = "../tagent" }.
tagent-gui      (binary)    -- Slint desktop GUI prototype. Depends on tagent only —
                                deliberately not on tagent-cli, to avoid pulling in
                                rustyline/rdev/x11/arboard/ctrlc and the whole
                                platform/ tree just for a translate box.
```

`tagent-cli` has no `[lib]` target — nothing needs one now that `providers` moved out
and `tagent-gui` depends on `tagent` directly, so it was deleted rather than kept
"just in case" (its old `pub mod config; pub mod platform; ...` tree lived in
`tagent-cli/src/lib.rs` before this split).

### The `tagent` library crate

`tagent/src/lib.rs` has `#![warn(missing_docs)]` (this crate is "the engine" referred
to by that convention in `CLAUDE.md` — `tagent-cli` and `tagent-gui` are applications
built on it, not libraries themselves, so the attribute lives here now instead of on
the old single-crate `tagent`).

- **`providers`** — three independent provider axes: the `TranslationProvider` trait +
  `create_provider()` factory (translate, `detect_language`), moved essentially
  unchanged from the old `src/providers/` (minus the dictionary method, see "Dictionary
  Provider Architecture" below), the `DictionaryProvider` trait +
  `create_dictionary_provider()` factory, and the `SpeechProvider` trait +
  `create_speech_provider()` factory (see "Speech Provider Architecture" below). Every
  method returns `Result<_, tagent::error::Error>` instead of the old `Box<dyn Error +
  Send + Sync>`. `GoogleSpeechProvider` implements `SpeechProvider`'s two TTS methods,
  `split_for_speech(text) -> Vec<String>` and `async fn speak_chunk(text, lang) ->
  Result<Vec<u8>, Error>` — this two-method split (rather than one `speak()` returning
  every chunk's audio up front) exists specifically so `tagent-cli`'s playback loop can
  keep fetching and playing chunks one at a time, matching the pre-refactor behavior of
  audio starting after the first chunk's round-trip instead of after all of them.
  `split_for_speech` returns short input (≤100 chars, Google TTS's per-request limit)
  verbatim as a single chunk without running it through sentence-splitting, preserving
  exact punctuation for the common case.
- **Speech Provider Architecture** (Stage 11, 2026-09-19) — TTS used to be two extra
  methods on `TranslationProvider`, so which backend *spoke* was hard-coupled to
  which one *translated*, even though Google's `translate_tts` endpoint has nothing
  structurally to do with `translate_a/single` beyond sharing a Rust struct. They are
  now two independent traits selected by two independent config keys
  (`TranslateProvider`/`translate_provider` × `SpeechProvider`/`speech_provider`,
  any combination), done deliberately *now* while only `"google"` implements either —
  `SpeechProvider`'s shape (exactly `split_for_speech` + `speak_chunk` + `name`) was
  already fully known, so the redo risk is low and migrating one provider is cheaper
  than migrating N later. (Dictionary lookup was held back at the time because its
  shape — language pairs, data-model richness, credentials — still had open questions;
  Stage 12 answered enough of them to build the same seam, see "Dictionary Provider
  Architecture" below.)
  - `GoogleSpeechProvider` is a separate struct from `GoogleTranslateProvider` (same
    `google.rs` file, but its own `reqwest::Client` — constructing it never touches
    anything translate-related). `split_for_speech`'s 100-char chunking is *Google's*
    per-request limit, not a general constraint: a backend with no limit returns
    `vec![text.to_string()]`.
  - `detect_language` stays on `TranslationProvider` (auto-detection is a translate
    capability an OS-native/third-party speech backend has no reason to implement). So
    speaking `"auto"`-source text is the one case genuinely needing both providers —
    a permanent, correct asymmetry. To avoid reintroducing the coupling through the
    back door, **the translate provider is constructed lazily, only when the source
    language is `"auto"`** (`SpeechManager::resolve_speech_language` in `tagent-cli`
    — used by `speak_text_full` and both platforms' `speak_clipboard` — and an inline
    equivalent in `tagent-gui`'s `start_speaking`). A concrete source language never
    builds a translate provider at all; if construction fails on the `"auto"` path
    (which nothing else on that call chain has already proven works, unlike before),
    it logs a warning and falls back to `"en"`, matching `resolve_source_language`'s
    own detection-failure fallback. `resolve_source_language` itself is unchanged.
  - Removing two methods from a public trait is source-breaking for any implementor.
    `tagent` is pre-1.0 (`0.17.0`), where a breaking change is what a minor bump is
    for; this one is recorded in `tagent/CHANGELOG.md` as part of the `0.17.0`
    baseline, and in both apps' changelogs.
- **Dictionary Provider Architecture** (Stage 12, 2026-09-20) — the same split for
  dictionary lookup: `get_dictionary_entry` left `TranslationProvider` (`tagent`
  `0.17.0` → `0.18.0`, no compatibility shim) for a `DictionaryProvider` trait
  (`lookup(word, from, to)` + `name()`), a `create_dictionary_provider()` factory
  (`"google"` only) and `GoogleDictionaryProvider`. The three axes —
  `TranslateProvider` × `DictionaryProvider` × `SpeechProvider` in `tagent-cli.conf`,
  `translate_provider` × `dictionary_provider` × `speech_provider` in `tagent-gui.json` —
  combine freely. Zero user-visible change; the point is that the next backend is "add a
  file + one `match` arm" instead of a cross-crate refactor.
  - **The trait keeps the positional `word + from + to` signature; extensibility lives on
    the return type.** A 2026-09-19 survey of online bilingual backends (Microsoft
    Translator Dictionary Lookup, Wiktionary-based APIs, Yandex, ABBYY Lingvo, PONS,
    Lexicala) showed they all take those three inputs and differ in what they *return*
    (confidence, gender prefix, IPA, forms, examples) and in credentials. So
    `DictionaryEntry`, `PartOfSpeechEntry` and `Definition` are `#[non_exhaustive]`
    with constructors (`::new`, `DictionaryEntry::with_corrected_word`); new fields can
    arrive later as a non-breaking patch. Struct literals outside `tagent` stopped
    compiling at `0.18.0` (`tagent-gui`'s dictionary tests and the `custom_provider`
    example were the only ones).
  - **The trait's documentation is the contract every backend inherits**: `Definition::text`
    is a translation into `to` and `synonyms` are words in `from` (the field names read
    like a monolingual dictionary; they are not); part-of-speech labels are lowercase
    English full words, because both apps localize through a table keyed on them
    (`get_full_part_of_speech`), so a backend normalizes tags or foreign labels inside
    the provider; `Ok(None)` means "no entry" (a miss, an unsupported pair, input the
    backend can't handle), never `Some` with an empty list, and `Error::NotFound` stays
    unused; `from == "auto"` is valid, and a backend that can't look up with it returns
    `Ok(None)` (making one work would need the caller to resolve `"auto"` first with
    `resolve_source_language`, the way speech does — to be decided when the first such
    backend arrives); `corrected_word` *should* be set only when a different word was
    looked up, but callers keep comparing it with their input case-insensitively, so
    Google's "always `Some`" behavior remains legal.
  - `GoogleDictionaryProvider` is a separate struct with its own `reqwest::Client`
    (same `google.rs` file, as `GoogleSpeechProvider`), not `GoogleTranslateProvider`
    implementing a second trait: selecting `dictionary_provider = "google"` with a
    different `translate_provider` instantiates nothing translate-related. The
    two-request spell-suggestion retry (`json[7]`) and the positional
    `parse_dictionary_response` moved verbatim, except that the parser now takes the
    caller's word.
  - **`DictionaryEntry::word` was fixed, not re-documented.** Its doc said "as supplied
    by the caller" but the parser filled it from `json[0][0][0]` — Google's *translation*
    of the input (`"жестокий"` for `violent`). It is now the caller's original input,
    also on the retry path (where `corrected_word` holds the suggestion). No shipped
    output changed: its only consumer was the `!cli_mode` branch of
    `Translator::format_dictionary_entry`, which is dead (its one caller passes
    `cli_mode = true`).
  - **Failure isolation: a bad `DictionaryProvider` value never breaks translation.**
    `tagent-cli`'s `Translator::build` still `?`s the translate provider but builds the
    dictionary provider non-fatally — one warning (`Dictionary provider unavailable
    (...); dictionary lookups disabled`) and `dictionary_provider: None`; its
    `get_dictionary_entry` (signature unchanged, so `cli.rs`/`interactive.rs` are
    untouched) then returns `Err` before any network call, and the callers' existing
    fallback to plain translation runs. `tagent-gui`'s `spawn_translation` builds it only
    inside the `show_dictionary && is_single_word` branch and, on `Err`, warns to stderr
    and takes the plain-`translate_text` path (no `join!`). An `Err` or `Ok(None)` from a
    lookup that did run still reuses the translation fetched by the same `join!`, never a
    second request.
  - **Deliberately left out**: a second backend or any enrichment of Google's parse (its
    `ex`/`md`/`ss`/`rw`/`rm`/`ld` blocks stay unparsed); credentials/options plumbing
    (`create_dictionary_provider(name)` stays name-only; a keyed backend adds an
    additive `..._with(name, &options)` later); a shared "translate + dictionary +
    fallback" helper in `tagent` (the two apps' shapes differ and the shared part is a few
    lines); monolingual or offline dictionaries (online, bilingual only, so `lookup` takes
    both `from` and `to`); new `Error` variants; a Settings dropdown for
    `dictionary_provider` (a one-entry dropdown, same as `speech_provider`); and a fix
    for `Translator::get_dictionary_entry` fetching the translation twice on a dictionary
    miss (its `join!` result is discarded and the caller translates again — pre-existing,
    unrelated to the seam).
  - Mechanical trap worth remembering: `create_ini_content` is one positional `format!`
    and `[Dictionary]` is mid-template, so the new placeholder and its argument had to be
    inserted together (Stage 11's `[Speech]` block was last, so its argument just
    appended). `test_generated_config_roundtrips_every_field_with_distinct_values` sets
    every `Config` field to a non-default value and reads it back, so a shifted value
    can't hide by coinciding with a default.
- **`languages`** — `name_to_code() / `code_to_name()`, a straight move of what used
  to be `ConfigManager::language_to_code()` / `code_to_language()`. This is
  translation-domain data (a name ↔ BCP-47 code table), not app config, which is what
  makes it safe for `tagent-gui` to depend on without pulling in `ConfigManager`.
- **`error`** — `tagent::error::Error`, a `thiserror`-based enum (`Network`, `Api`,
  `NotFound`, `EmptyText`, `TextTooLong { len, max }`, `Decode`, `UnknownProvider`)
  used across the provider boundary. `tagent-cli` still uses `Box<dyn Error + Send +
  Sync>` internally as before; `Error`'s `?` conversion into that boxed type is
  automatic since it implements `std::error::Error + Send + Sync`, so no `From` impls
  were needed at the seam.
- **`resolve_source_language(provider, text, from)`** (in `providers`) — resolves
  `"auto"` to a concrete code via `provider.detect_language()`, falling back to `"en"`
  and logging to stderr on failure rather than propagating an error. This preserves the
  old `SpeechManager::detect_speech_language`'s best-effort behavior exactly (never
  surface a language-detection failure as a speech error to the user).

## Platform abstraction layer

`tagent-cli/src/platform/mod.rs` is **not** a `trait`/`dyn` abstraction. It is a `#[cfg(target_os = "...")]`-gated
module tree plus re-exports:

```rust
#[cfg(target_os = "windows")] pub mod windows;
#[cfg(target_os = "linux")]   pub mod linux;
#[cfg(target_os = "macos")]   pub mod macos;

#[cfg(target_os = "linux")]
pub use self::linux::clipboard::ClipboardManager;
// ...same for keyboard::KeyboardHook, keycodes, signals, window::{WindowHandle, WindowManager}
```

Each platform module independently defines a struct with the same name and the same
inherent methods (`ClipboardManager::new/get_text/set_text/...`,
`KeyboardHook::new/start`, `WindowManager::new/show_terminal/...`). The compiler only
ever sees one platform's implementation compiled in at a time, so there is no vtable,
no trait object, and no dynamic dispatch — call sites in `translator.rs`, `interactive.rs`,
`cli.rs`, and `main.rs` just `use crate::platform::{ClipboardManager, KeyboardHook, ...}`
and the right implementation is selected at compile time by `target_os`.

**Implication for contributors**: adding a method to one platform's `ClipboardManager`
does not require a shared trait definition anywhere — just add matching methods (same
name, same signature) to the other two platforms' structs, or callers written against
one platform will fail to compile on the others.

### Feature parity matrix

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Clipboard get/set | ✅ `arboard` | ✅ `clipboard-win` | ❌ stub, always errors |
| Auto-copy selection (simulated Ctrl+C) | ✅ `xdotool` (X11/XWayland only) | ✅ `SendInput` | ❌ stub, always errors |
| Global hotkeys | ✅ `rdev` + `XGrabKey` (X11/XWayland only) | ✅ `WH_KEYBOARD_LL` hook | ❌ stub, prints a notice and idles |
| Show/hide/focus terminal window | ✅ Xlib | ✅ Win32 (`GetConsoleWindow` etc.) | ❌ stub, all no-ops |
| Pure Wayland (no XWayland) | ⚠️ interactive/CLI only — clipboard auto-copy and hotkeys are disabled with an explanatory message | n/a | n/a |

The Linux "X11 full-featured / pure-Wayland degraded" split described in `CLAUDE.md` is
still accurate. **macOS is a separate, larger gap**: it is essentially a no-op skeleton
across clipboard, keyboard hook, and window management, with zero macOS-specific crate
dependencies (no `[target.'cfg(target_os = "macos")'.dependencies]` section exists in
`tagent-cli/Cargo.toml` at all). It compiles and runs, but only interactive/CLI mode
actually works — anyone picking up macOS support starts from these three stub files.

### Linux specifics: `xgrab.rs`

`rdev::listen` only *observes* raw key events — it does not stop them from reaching the
focused application. `tagent-cli/src/platform/linux/xgrab.rs` uses X11's `XGrabKey` to
grab the configured hotkey combo at the X server level so the keystroke is consumed by
tagent instead of leaking through. Notable details:

- **Layout-independent by construction**: the base key is grabbed by its raw X11
  hardware keycode (`vk_to_x11_keycode`, standard evdev-based numbering — the same
  positional identification `rdev` itself uses internally for detection, cross-checked
  against `rdev` 0.5.3's own keycode table), not by converting to a keysym and calling
  `XKeysymToKeycode` (fixed 2026-09-14: that approach silently failed to grab at all on
  a keyboard layout with no Latin group, e.g. a pure Russian layout — `XKeysymToKeycode`
  found no keycode for the Latin keysym, so the hotkey kept *triggering* via `rdev`
  (already keycode-based) but stopped being *suppressed*, leaking the keystroke into
  whatever app had focus). Modifier masks (`ControlMask`/`Mod1Mask`/`ShiftMask`/
  `Mod4Mask`) are unaffected either way — those are fixed protocol-level bits, not
  layout-resolved.
- Grabs all CapsLock/NumLock modifier-mask variants, plus a duplicate grab under
  `Mod5Mask` to also catch AltGr-mapped right-Alt.
- **Cannot** grab `HotkeyType::DoublePress` hotkeys — `XGrabKey` has no double-tap
  concept, so those hotkeys rely purely on `rdev`'s passive listening and are never
  consumed at the X server level (another app briefly sees the keystroke too).
- Installs a custom X11 error handler so a `BadAccess` (another app already grabbed
  that combo) logs a warning and continues, instead of calling `exit()` and killing
  the process's X connection for the whole session.
- `platform::linux::signals::setup()` calls `xlib::XInitThreads()` before any other
  Xlib call, because `WindowManager` and `XGrabManager` open independent X11 `Display`
  connections from different threads — skipping this causes intermittent Xlib crashes.

## `tagent-gui`: Slint desktop GUI

`tagent-gui` is a **separate binary crate** in the Cargo workspace, not a mode of the
`tagent-cli` binary. There is no flag or code path in `tagent-cli`'s own
`main.rs`/`cli.rs` that launches it.

**Concept (decided 2026-08-15, see the local, untracked `.debug/tagent-gui development plan.md`): `tagent-gui` is a fully
independent application from `tagent-cli`** — own interface, own configuration (its
own `tagent-gui.json`; see "Reading `translate_provider`" below), own feature set (no
obligation to reach parity with `tagent-cli`), and own versioning/changelog
(`tagent-gui/CHANGELOG.md`, independent of `tagent-cli/CHANGELOG.md`, and using the same `MAJOR.MINOR.PATCH+BUILD` format/increment rules as
`tagent-cli` but its own independent counter — decided 2026-09-11). The only thing the
two share is the `tagent` library. This sharpens, rather than changes, the dependency
rule already in place below (`tagent-gui` depends on `tagent` only, never on
`tagent-cli`).

- **UI framework**: [Slint](https://slint.dev/), via `tagent-gui/ui/app.slint` and the
  `slint`/`slint-build` crates. `tagent-gui/build.rs` is a single line:
  `slint_build::compile("ui/app.slint").unwrap();`.
- **Dependency on the `tagent` library**: `tagent-gui/Cargo.toml` depends on
  `tagent = { path = "../tagent" }` — the pure library crate (see "Workspace layout"
  above), not `tagent-cli`. Unlike before this crate existed, there is no
  `binary-resources` feature to disable here: `tagent` never runs `winres`, so there's
  nothing Windows-resource-related to guard against.
- **Reading `translate_provider` from its own config file**: since `tagent` has no
  config module, `tagent-gui/src/config.rs` implements a small, independent JSON
  config of its own — `GuiConfig { translate_provider: String }`, serialized with
  `serde`/`serde_json` to `tagent-gui.json` at `dirs::config_dir().join("tagent-gui")`.
  This avoids reusing `tagent-cli`'s `ConfigManager` directly: pulling that in would
  mean pulling in all of `tagent-cli` (rustyline, rdev, x11, arboard, ctrlc, the whole
  `platform/` tree), just to read one string. Unlike `tagent-cli.conf`'s INI format
  (commented, meant to be self-documenting), `tagent-gui.json` is plain JSON with no
  comment support — but it's still meant to be hand-editable (there's no Settings
  window yet), not just a machine-written cache: `load_from_path()` leaves an existing
  but unparseable file untouched on disk rather than overwriting it, logging a warning
  and falling back to defaults in memory for that run instead. A missing file gets a
  freshly written default (`{"translate_provider": "google"}`, pretty-printed).
  `GuiConfigManager` wraps this with mtime-based live-reload
  (`check_and_reload()`), mirroring `tagent-cli`'s
  `ConfigManager::check_and_reload()`: a reload that fails to parse keeps the
  last-known-good in-memory config rather than reverting to the default (the
  default-on-corruption behavior is specific to the very first load).
- **How translation works**: `main()` creates one `GuiConfigManager` (`GuiConfig` +
  its file's last-seen mtime), wrapped in `Arc<Mutex<_>>` so both the
  `translate-requested` and `settings-requested` callbacks can share it (via
  separate `.clone()`s of the `Arc`, one moved into each closure — see the
  Settings dialog bullet below). The `translate-requested` Slint callback calls
  `check_and_reload()` and clones the current `translate_provider` synchronously (on
  the UI thread, before spawning any work) — so a hand-edited config file is picked up
  on the *next* translation, no restart needed — then spawns a plain OS thread with its
  own fresh `tokio::runtime::Runtime`, calls
  `tagent::providers::create_provider(&translate_provider)`, calls
  `provider.translate_text(...)`, then marshals the result back onto the Slint UI thread
  via `slint::invoke_from_event_loop`. A `to == "auto"` request is rejected before
  the reload/spawn (both the target `ComboBox` and the ⇄ swap button can otherwise
  produce one), appending an in-transcript error instead of calling the provider.
  Language names from the UI are resolved to codes via `tagent::languages::name_to_code`.
- **Transcript pane** (`transcript-scroll` in `app.slint`, `phrase-line`/`translation-line`
  per row): **view-only since Stage 13** (2026-09-22) -- each block is a `StyledText`
  (Slint's rich-text element, a CommonMark subset plus `<font color="...">`), not a
  `TextInput`, so there is no mouse selection or Ctrl+C in the transcript any more. It
  stays pinned to its end: `scroll-to-transcript-end()` in `app.slint` sets
  `transcript-viewport-y` to the negative overflow from `changed` handlers on
  `transcript-viewport-height` / `transcript-visible-height`, so the pane auto-scrolls to
  the latest translation. This is deliberately not done from Rust right after
  `push_transcript_entry` changes the model: the new `for` row is only instantiated on the
  next layout pass, so a height read at that moment is stale and the view stops one entry
  short. `push_transcript_entry_scrolls_to_the_end` (`i-slint-backend-testing`, headless)
  guards it -- and, being driven purely by layout `changed` handlers rather than anything
  `TextInput`-specific, needed no change for the Stage 13 `StyledText` swap.
  - **Highlighting (`tagent-gui/src/styled.rs`)**: each block carries a *template* --
    markdown where a colored span is `<font color="@role">...</font>`, the role name
    (`prompt`/`pos`/`synonym`/`notice`/`error`, never a literal color) after `@` -- and a
    separately stored, already-rendered `styled-text` value the `StyledText` element
    actually binds to. Keeping both lets a theme flip re-render existing rows without
    needing their original text again. Every user- or provider-derived string is run
    through `styled::escape_markdown` before being placed in a template (backslash-escapes
    all ASCII punctuation, turns leading/trailing whitespace and blank lines into NBSP so
    Markdown doesn't reinterpret them) -- this is also what keeps a literal `color="@pos"`
    embedded in hostile input from ever matching `render_template`'s naive
    find-and-replace substitution of a role token for its actual hex color.
    `RoleColors::for_background` picks a light- or dark-background palette (both
    WCAG-AA-contrast-checked in tests) from the *resolved* `phrase-background`/
    `translation-background` (not the raw OS theme), so highlighting stays legible even
    under a customized background. `dictionary.rs` gained a small role-tagged
    intermediate representation (`article_lines`, a `Vec<Line>` of role-tagged `Span`s)
    that both `format_dictionary_entry` (`to_plain`, unchanged output, pinned by a golden
    test -- still what the popup and history-adjacent `translation_raw` use) and the new
    `to_template` derive from, so plain and styled output can't drift apart.
  - **Live restyle**: `main.rs`'s `restyle_transcript`, called at the end of
    `apply_style` (which already runs on settings change, config live-reload, first show,
    and the `Auto`-theme poll timer), re-renders every row's `styled-text` fields against
    the current colors -- but only when they actually changed since the last call
    (`role_colors_changed`, a pure function with its own tests), so the once-a-second
    theme poll doesn't re-parse and re-lay-out every row for nothing. Rows are written
    back individually with `set_row_data`, not `set_transcript_entries` (which rebuilds
    the model and would reset the scroll position).
  - **Copying**: right-click a block replaces the lost selection, in one of two modes
    controlled by `show_context_menu` (`tagent-gui.json`, Settings > General "Show menu on
    right-click", default `false`, live-reloaded -- added 2026-09-22, a day after the
    highlighting/copy-menu feature itself, once a single-item menu turned out to be pure
    friction over just copying directly). Either way the target is `copy-block-requested(index,
    is_phrase)`, whose Rust handler reads that row's `phrase-copy`/`translation-copy` (plain
    text, no prefix, no markup -- a dictionary hit copies the whole article) and writes it via
    `ClipboardManager::set_text` on a spawned thread, same as the existing 📋 button --
    deliberately a different callback from `copy-requested` (the unrelated "paste clipboard
    into input" button).
    - **Menu on** (`ContextMenuArea` + `Menu` + `MenuItem`, one pair per phrase/translation
      `Rectangle`): right-click opens a "Copy" menu; clicking it fires `copy-block-requested`.
    - **Menu off (default)**: the `ContextMenuArea` is swapped for a plain `TouchArea`
      (`if show-context-menu: ContextMenuArea {...}` / `if !show-context-menu: TouchArea
      {...}`, mutually exclusive per block) whose `pointer-event` fires `copy-block-requested`
      directly on a right-button-up, with no menu shown at all. Since nothing else indicates
      the copy happened, `copy-flash-index`/`copy-flash-is-phrase` (AppWindow properties, set
      right before firing the callback) drive a 220ms `animate`d `border-width` flash
      (`prompt-accent`-colored) on the copied `Rectangle`, reset by a `flash-timer := Timer`
      element. The menu path never touches this state, since the menu's own click-to-close is
      already visible feedback on its own.
- **Configurable prompt color** (2026-09-22): the `[Language]:` prompt shown before phrase
  and translation text has its own color, independent of the phrase/translation text colors
  -- `prompt_color` (`tagent-gui.json`, Settings > View) for the main window, resolved in
  `apply_style` the same way `phrase_color`/`translation_color` are (an empty value falls
  back to `prompt-accent-theme-default`, `app.slint`'s own `Palette.color-scheme`-branching
  default, formerly hardcoded as `prompt-accent` before this feature). It drives two things:
  the input box's own `[Lang]:` label (`prompt-accent`, read directly as a `color`) and, via
  `color_to_hex` and `styled::RoleColors::new`'s `prompt` parameter, the transcript's
  `Role::Prompt` highlighting (Stage 13) -- which is why `RoleColors` changed from a `Copy`
  struct with two fixed light/dark presets to a `Clone`-only one carrying a resolved `String`:
  `pos`/`synonym`/`notice`/`error` still auto-derive from each block's background luminance,
  but `prompt` is this one shared, user-set value instead. `popup_prompt_color` is the
  popup's own independent counterpart (Settings > Popup), chaining through `prompt_color`
  first and the popup's own theme default last -- the same fallback shape `popup_color`
  already uses through `translation_color`. Making this visible in the popup required
  converting its `phrase-line`/`translation-line` from plain `Text` to `StyledText` too
  (initially prompt-only; since `tagent-gui` `0.14.0+012` the popup renders the transcript's
  full role-tagged templates -- `TranslationOutcome::translation_body_template`, wrapped by
  `popup_templates` and kept on the popup as `phrase-template`/`translation-template` so
  `restyle_popup`, called from `apply_popup_style`, re-renders them on a theme/color change);
  since whether `StyledText.preferred-width` reports natural unwrapped width the same way
  `Text`'s does (the property `content-natural-width` relies on to size the popup) was
  unverified, width measurement stayed on two invisible plain-`Text` twins
  (`phrase-measure`, and a new `translation-measure`) rather than the now-`StyledText`
  `phrase-line`/`translation-line` themselves, sidestepping the question rather than
  answering it. Each of the 14 non-"Default" presets in `COLOR_SCHEMES` (Settings > View's
  "Color scheme" dropdown) also picked up its own prompt accent, picked from that palette's
  own well-known accent set -- most clear 4.5:1 contrast against both the scheme's own
  background and its slightly darker phrase/translation background, but Solarized Dark/Light
  and Catppuccin Latte fall a little short (3.5-4.1) against the darker one specifically; every
  color in each of those three palettes' own accent sets was checked by hand and none does
  better there without abandoning the palette's own look, so this was accepted rather than
  substituting an inauthentic color.
- **Right-click copy in the popup** (2026-09-22, redesigned same day). The first version
  (`show_context_menu`-gated: a popup-wide `ContextMenuArea`+`Menu` when on, an extra branch in
  `touch-area`'s own `pointer-event` when off) did not survive contact with the real app: with
  the menu on, it opened as soon as the popup appeared rather than on right-click, and was
  clipped out of view on a small popup; with it off, right-click copied nothing. Root cause not
  chased down -- replaced outright with the user's own simpler proposal instead of debugging the
  broken version further:
  - **No menu at all any more**, regardless of the transcript's own `show_context_menu`
    setting -- that setting has no effect on the popup now.
  - **Per-block `TouchArea`s**, mirroring the transcript's own design exactly: one nested inside
    each of the phrase/translation `Rectangle`s, reacting to `PointerEventKind.up` +
    `PointerEventButton.right` to fire `popup-copy-requested(is_phrase)` and set
    `copy-flash-active`/`copy-flash-is-phrase` (a `border-width` flash on whichever block was
    clicked, `popup-prompt-accent`-colored, same as the transcript's own copy-flash-index but
    split into two properties since there's no index -- exactly one phrase and one translation
    block).
  - **Dragging the popup was temporarily disabled**, then restored the same day gated behind
    Ctrl: a `TouchArea` nested over the phrase/translation text captures pointer events for
    that screen region regardless of which button a callback reacts to, so a *plain*
    left-click-drag there would compete with right-click-copy for the same surface -- the exact
    conflict this whole redesign exists to avoid. Rather than ship that half-working compromise,
    dragging was first removed outright (`touch-area`'s `pointer-event` handler and its
    `mouse-cursor` "move" binding deleted, `drag-started`/`drag-moved`/`drag-ended` left declared
    and `wire_popup_drag` in `main.rs` left fully wired to them, unused), then brought back into
    the *same* two per-block `TouchArea`s that already handle copy: `PointerEventKind.down` +
    `PointerEventButton.left` (+ `event.modifiers.control` until 0.14.0+014) calls `drag-started()`; `moved` always
    calls `drag-moved()`; any `up`/`cancel` not matched by the copy branch always calls
    `drag-ended()` -- both unconditional calls are safe no-ops when `wire_popup_drag`'s own
    in-progress-drag state is `None`, the same shape the pre-disable code already had.
  - **Plain left-button drag from any point (0.14.0+015)**: the Ctrl gate turned out to be
    unnecessary -- copy reacts only to the right button and nothing else uses a plain left
    press (the text isn't selectable), so the two never compete even inside one `TouchArea`.
    The gate was dropped (modifiers are now ignored, so Ctrl+drag still works) and the same
    left-down/`moved`/up-or-cancel handler was added to `touch-area` itself, which only ever
    sees presses over the thin margin (`content-layout`'s 8px padding plus the border) -- so
    the popup drags by any point of its surface. `touch-area` also still does hover tracking
    (auto-hide) and the wheel-scroll-hover-loss workaround (`scroll-event`). A private
    `drag-pressed` property, set while the left button is held on a block, joins
    `touch-area.pressed` in `hide-timer`'s engagement check, since `touch-area.pressed` is
    false for a press its nested block `TouchArea`s took.
  - `phrase-copy`/`translation-copy` (`TranslationPopup` properties) are set directly from
    `TranslationOutcome.phrase_raw`/`translation_raw` in `show_popup` -- both already the raw,
    unprompted text the transcript's own `*-copy` fields are built to match, so no extra
    derivation was needed. The clipboard write itself (`wire_popup_copy`, a function alongside
    `wire_popup_drag`) mirrors `on_copy_block_requested` but is simpler: the popup shows exactly
    one phrase/translation pair, so there's no row/index to look up.
- **Input box** (`input-field` in `app.slint`): a multi-line `TextInput` inside its own
  `ScrollView`, wrapped in a resizable container — a 6px drag handle above the box lets
  the user set `input-user-height` between `input-min-height` (32px) and
  `input-max-height` (220px); the box also grows automatically with wrapped content up
  to that cap. `key-pressed` submits on Enter and inserts a newline on Shift+Enter.
  `forward-focus: input-field` on the window root means the input field has focus as
  soon as the window opens, so typing or pasting works without clicking into it first.
- **Settings dialog** (`SettingsDialog` in `app.slint`, gear `⚙` button at the right
  edge of `AppWindow`'s top row): a second `export component ... inherits Dialog`
  in the *same* `app.slint` file — supported since Slint 1.7, no second `build.rs`
  entry needed. Uses `std-widgets`' built-in `Dialog`/`StandardButton { kind: ok /
  cancel; }` rather than a hand-rolled `Window`, so button layout/ordering follows
  the platform convention for free — sets `preferred-width`/`preferred-height`
  explicitly (420×300); without it the dialog fell back to Slint's default window
  size, an early Stage 3 bug fixed the same day it shipped. Its content sits inside
  a `TabWidget` (`General` for the provider; `View` for theme/style; `Hotkeys &
  Tray`, added empty at Stage 3 and filled in across Stages 7-8 with the
  start-minimized/remember-geometry checkboxes and the hotkey/popup-delay
  controls) rather than a flat panel — laid out ahead of need since Settings is
  expected to grow more categories over future stages, so a new category is a
  new `Tab { }` block, not a redesign.
  `TabWidget`'s `Tab` children, like `Dialog` itself, are core-language-adjacent:
  importing `Tab` from `std-widgets.slint` explicitly is rejected the same way
  importing `Dialog` is — only `TabWidget` itself is imported. Its `providers`
  array property (default `["google"]`) is the *only* place the known-provider
  list is defined — mirroring
  `AppWindow`'s `languages` array — and `main.rs` reads it back via
  `dialog.get_providers()` to resolve the current `translate_provider` to a
  `ComboBox` index rather than hard-coding its own copy of the list. `main()`
  creates a fresh `SettingsDialog` instance each time the gear is clicked (not a
  long-lived one re-shown), matching the upstream multi-window example; Slint keeps
  a shown window alive internally once `.show()` is called, so only a `Weak`
  handle needs to survive inside the `on_save_requested` closure to call `.hide()`
  after saving (same pattern `AppWindow`'s own callbacks already use). The `ok`
  `StandardButton` gets an explicit `clicked` handler that resolves
  `providers[provider-index]` to a string *inside Slint* and passes it to Rust via
  a custom `save-requested(string)` callback — mirroring how `translate-requested`
  already receives resolved language strings rather than indices — so Rust never
  needs its own copy of the provider list to interpret the callback argument. The
  `cancel` `StandardButton` has no Rust-side handler at all: `Dialog`'s documented
  default ("the cancel button rejects a Dialog and closes it when clicked") is
  sufficient. Saving goes through `GuiConfigManager::update()` (`config.rs`), which
  applies the change in memory immediately (regardless of whether the disk write
  succeeds) and refreshes the tracked mtime so the write doesn't trigger a
  self-inflicted reload on the next `check_and_reload()`. If the current
  `translate_provider` isn't one of `providers` (e.g. a hand-edited, not-yet-listed
  value), the dialog falls back to preselecting index 0 rather than erroring —
  accepted, since only `"google"` is a supported value today. `dictionary_provider` and
  `speech_provider` have their own dropdowns beside it (2026-09-20, `dictionary-providers`
  / `speech-providers` in `app.slint`; the three axes are independent), seeded and read
  through the shared `combo_index`/`combo_selection` helpers in `main.rs`, with the same
  fall-back-to-index-0 behavior — so a hand-edited unknown value is replaced by the first
  entry the next time Settings is saved. The lists come from `tagent`
  (`TRANSLATION_PROVIDERS`/`DICTIONARY_PROVIDERS`/`SPEECH_PROVIDERS`, `0.18.1`), set on the
  dialog by `seed_dialog_fields` each time it opens, so a backend added to `tagent` is
  offered in Settings with no edit to `app.slint` (its own lists are placeholders). These
  are plain name lists next to the factories, not a registration mechanism — the factories
  stay closed `match`es — and a test checks that every listed name is accepted by its
  factory, which is what keeps a list from drifting from the `match` it describes.
- **Theme** (`GuiConfig.theme`, `"auto"`/`"light"`/`"dark"`; `View` tab in
  `SettingsDialog`): switches via `std-widgets`' `Palette.color-scheme`
  (`ColorScheme.unknown`/`.light`/`.dark`), but **not** by calling
  `.global::<Palette>()` from Rust — that would require naming Slint's
  `ColorScheme` type in Rust, and the only path that resolves to
  (`slint::private_unstable_api::re_exports::ColorScheme`, confirmed by grepping
  the macro-generated `app.rs` in `target/`) is exactly what its name says: not a
  stable public API to depend on. Instead, both `AppWindow` and `SettingsDialog`
  each define their own `public function apply-theme(theme: string)` that does the
  `Palette.color-scheme = theme == "light" ? ColorScheme.light : ...` assignment
  *inside* `.slint`, generating a plain Rust method (`invoke_apply_theme(&self,
  theme: SharedString)`) with no enum type crossing the language boundary at all.
  Each window needs its *own* call: globals aren't shared between top-level
  components (confirmed via Slint's own docs/discussions — same reason
  `GuiConfigManager` is passed to both the `translate-requested` and
  `settings-requested` closures rather than read off one shared Slint global), so
  `main.rs` calls `invoke_apply_theme` three times: once on `AppWindow` at
  startup, once on each freshly-created `SettingsDialog` (so it opens already
  matching the active theme instead of the system default), and once more on
  `AppWindow` from inside `on_save_requested` to apply a newly-picked theme live.
  The custom-drawn transcript/input panels (previously fixed hex colors —
  `#0c0c0c`/`#2a2a2a`/`#d4d4d4`/`#151515`/`#264f78`) now read `Palette`'s semantic
  role properties instead (`background`/`alternate-background`/`border`/
  `foreground`/`alternate-foreground`/`selection-background`/
  `selection-foreground`/`control-background`/`control-foreground`), which already
  resolve correctly for all three `color-scheme` values with no manual branching —
  Palette's role properties are *resolved* colors, not raw scheme flags, unlike
  `color-scheme` itself. The one exception is the `"[Lang]:"` prompt highlight
  (`prompt-accent`), a decorative color with no matching Palette role: it branches
  directly on `Palette.color-scheme == ColorScheme.light`, which means it can't
  distinguish "explicitly auto, system is light" from "explicitly dark" — an
  accepted minor limitation, not worth a bigger fix for one decorative color.
  **Known limitation with `Auto` specifically**: on Linux, a freshly created
  window (at app startup, and each time a `SettingsDialog` opens) briefly paints
  in a light scheme before repainting in the correct one — an upstream Slint/
  winit limitation ([`slint-ui/slint#4392`](https://github.com/slint-ui/slint/issues/4392):
  winit doesn't deliver Linux theme detection synchronously), not something
  fixable in `apply-theme` itself. Explicit `Light`/`Dark` need no detection and
  aren't expected to flash. For properties bound live to `Palette` (like
  `panel-foreground`), this really is just a cosmetic single frame, since they
  repaint on their own once the real scheme resolves — accepted as-is. But
  `apply_style` (`tagent-gui/src/main.rs`) additionally *snapshots* several
  `Palette`-derived colors into plain, non-live properties (`panel-background`,
  `phrase-color`/`phrase-background`, `translation-color`/
  `translation-background`) so they can be independently overridden from
  Settings. Those don't self-correct: if `apply_style`'s one and only call
  lands before the real scheme resolves, the wrong colors are permanent, not
  single-frame — e.g. the transcript header ending up unreadable (light text
  baked in against what a moment later becomes a dark-resolved background, or
  vice versa), not just flashing. A fixed delay timed from window *creation*
  isn't enough to correct this, confirmed live: with `start_minimized` (the
  default), the window can sit unmapped for a long time before the user's
  first "Show Tagent", and system theme detection here appears tied to the
  window actually having an on-screen surface, not wall-clock time since
  creation. Fixed (`0.14.0+031`) by moving the re-apply into
  `show_window_restoring_geometry` instead: it now calls `apply_style` again
  immediately after the window's first real `.show()`, plus the same 150ms
  deferred retry `show_window_restoring_geometry` already used for the
  analogous winit/X11 sizing race — anchored to first-show, not creation.
  Verified live via the `busctl Activate` trick (see the Tray bullet below)
  plus a screenshot of the actual window.

  A second, related bug (`0.14.0+032`): none of the above helps if the user
  changes the OS-level dark/light preference *while `tagent-gui` is already
  running* — `apply_style` was never called again after the first show, so
  the same baked snapshots stayed frozen even though Palette-bound elements
  (the input bar's frame, `field-background`, the OS-drawn window
  decorations) kept following the live system change on their own, leaving
  the transcript panel visibly out of sync with the rest of the window.
  Fixed with a 1-second repeating `slint::Timer` in `main()`
  (`theme_poll_timer`) that re-calls `apply_style` for as long as
  `config.theme == "auto"` — polling rather than event-driven, since Slint
  doesn't expose a "system theme changed" callback and (per the reasoning
  above about `apply-theme`) this project avoids reaching for Slint's
  private `ColorScheme` type from Rust to build one. A no-op when nothing
  has actually changed. Verified live the same way: toggled
  `org.gnome.desktop.interface color-scheme` via `gsettings set` while the
  app was running and screenshotted the window before/after.
- **Clipboard** (`tagent-gui/src/platform/`, Stage 4, shipped 2026-09-13): a
  `ClipboardManager` per OS (`platform/{linux,windows,macos}/clipboard.rs`), behind
  `#[cfg(target_os = "...")]` re-exports in `platform/mod.rs` — the same
  directory-per-OS, no-`trait` shape as `tagent-cli/src/platform/mod.rs`, chosen
  deliberately so Stage 5's hotkey code could drop `keyboard.rs`/`keycodes.rs`
  into the same per-OS directories later without a restructure — which is exactly
  what happened (see the Hotkey bullet below). `get_text`/`set_text`/
  `copy_selected_text`/`get_text_with_copy` are ported near-verbatim from
  `tagent-cli`'s own `ClipboardManager` (Linux: `arboard` + a process-lifetime
  `static CLIPBOARD` + `xdotool`-simulated Ctrl+C; Windows: `clipboard-win` +
  `SendInput`/`WM_CANCELMODE`/`WM_COPY`-to-focused-control fallback; macOS: a
  stub, every method `Err`, matching `tagent-cli`'s own macOS posture). `set_text`
  is currently unused (`#[allow(dead_code)]`, kept for API parity and future use,
  e.g. copying a translation result back to the clipboard) — clipboard support
  only wires up reading. A "📋" button in the input row (`copy-requested`
  callback) calls `ClipboardManager::get_text_with_copy()` on a background
  `std::thread::spawn` (both platforms' `copy_selected_text` block for real
  wall-clock time — up to ~250ms on Linux, ~900ms on Windows — so this must not
  run on Slint's own event loop thread), then applies the result via
  `slint::invoke_from_event_loop`, mirroring `on_translate_requested`'s existing
  thread-hop pattern exactly. On success it replaces `input-text`; on failure
  (e.g. the Wayland guard, or `xdotool` missing) it pushes a `TranscriptEntry {
  phrase: "[Clipboard]", ... }` through the existing `push_transcript_entry`
  error-display convention, rather than inventing a new one. **Known limitation,
  inherent to a button-triggered (as opposed to global-hotkey-triggered)
  design**: clicking "📋" necessarily gives `tagent-gui`'s own window input focus
  first, so the simulated Ctrl+C inside `copy_selected_text` targets `tagent-gui`
  itself, not whatever window/selection was active immediately before the click.
  In practice the button behaves as "pull whatever the system clipboard already
  holds into the input box" (useful on its own, e.g. after a manual Ctrl+C
  elsewhere) rather than "grab the current selection with no prior Ctrl+C
  needed" — that stronger capability now exists via the Stage 5 global hotkey
  below, which doesn't require a focus change to fire. Not a bug in the button
  itself; documented here so it isn't mistaken for one.
- **Global hotkey** (`tagent-gui/src/platform/{linux,windows,macos}/{keyboard,keycodes}.rs`
  + `xgrab.rs` on Linux, Stage 5, shipped 2026-09-13): default `Alt+A`, configured
  via the `translate_hotkey` field in `tagent-gui.json` — hand-editable, and (Stage
  8, shipped 2026-09-16) also editable from Settings > "Hotkeys & Tray", which
  validates the string live via `HotkeyParser` and disables OK while it's invalid.
  Either way, a change only takes effect after restarting `tagent-gui` (no
  live-reload of the OS-level grab itself). `config::HotkeyType`/`HotkeyParser` are ported from
  `tagent-cli/src/config.rs` verbatim (same string grammar: `F1`-`F12` single
  keys, `Modifier+Key` combos, `Key+Key` double-press). As of Stage 10 follow-up
  (below), `KeyboardHook` tracks two hotkeys (translate + speech) plus Escape
  observation, matching `tagent-cli`'s own two-hotkey shape — but each OS's
  `keycodes.rs` still has no `KEY_STATES`/`set_key_state`/`is_key_pressed`
  poll-based ESC-tracking, unlike `tagent-cli`'s: `tagent-gui`'s Escape
  cancellation rides the same passive key-event stream `KeyboardHook` already
  runs for hotkey detection, rather than a separate poll. **Linux**:
  `platform::KeyboardHook::spawn(translate_hotkey, speech_hotkey, on_translate_trigger,
  on_speech_trigger, on_escape)` grabs both configured hotkeys via
  `xgrab::XGrabManager` (ported from `tagent-cli` near-verbatim — `XGrabKey`
  with CapsLock/NumLock variants and an AltGr/Mod5 fallback for Alt combos;
  Escape is never grabbed) and runs two `HotkeyState` instances (translate,
  and optionally speech) fed by an `rdev::listen` thread over a plain
  `std::sync::mpsc` channel — no `tokio` needed here, unlike `tagent-cli`'s
  `tokio::select!`-based loop, since there's no second async task to interleave
  with. **Windows**: a `WH_KEYBOARD_LL` hook with the same process-global
  `OnceLock` statics and Alt-only swallow-and-replay mechanism as `tagent-cli`'s
  (see that module's own doc comment, copied into `keyboard.rs` here too, for
  the five hard-won invariants from `tagent-cli`'s past failed attempts) — now
  two hotkeys (`TRANSLATE_HOTKEY`, `SPEECH_HOTKEY`) plus Escape, matching
  `tagent-cli`'s own two-hotkey statics, and with `TRANSLATOR: OnceLock<Arc<Translator>>`
  replaced by `ON_TRANSLATE_TRIGGER`/`ON_SPEECH_TRIGGER`/`ON_ESCAPE: OnceLock<Box<dyn Fn() + Send + Sync>>`
  so this module stays as ignorant of `tagent`/providers/Slint as `ClipboardManager` already is.
  **Both platforms' hook code is fully app-agnostic**: `KeyboardHook::spawn`
  only ever calls the `on_trigger` closure `main.rs` supplies — the "only one
  translation at a time" guard, clipboard read, provider call, and transcript
  push all live in that closure (and in `spawn_translation`, extracted out of
  `on_translate_requested` specifically so the button and hotkey paths share one
  implementation instead of two), not inside the platform code, unlike
  `tagent-cli`'s own `keyboard.rs` files which couple that guard to the hook
  directly. On Windows specifically, `on_trigger` must return near-instantly
  (a slow `WH_KEYBOARD_LL` callback gets silently unhooked by Windows) — it only
  does an atomic swap plus `slint::invoke_from_event_loop`, never clipboard I/O
  or network calls directly. **macOS**: a stub (`KeyboardHook::spawn` logs once
  and does nothing), matching `tagent-cli`'s own macOS posture — Linux+Windows
  only for this whole hotkey/tray/popup cluster, per the Concept doc. **Verification
  caveat**: the Windows port compiles and its pure decision-function unit tests
  build cleanly under `cargo check`/`cargo test --no-run --target
  x86_64-pc-windows-gnu`, but neither the app nor its tests have been *run* on
  Windows or under `wine` (unavailable in this environment) — confirmed
  compiling only, not confirmed correct in practice yet.
- **Popup window** (`TranslationPopup` in `app.slint`, `platform::window` module,
  Stage 6, shipped 2026-09-14): the global hotkey above now also shows a small,
  cursor-positioned popup with the phrase/translation, in addition to (not instead
  of) the existing transcript update — hotkey-triggered only, not shown for the
  Translate button/Enter key. `TranslationPopup inherits Window` directly (`no-frame:
  true; always-on-top: true;`), a real top-level OS window rather than Slint's
  builtin `PopupWindow` *element* (used elsewhere in `app.slint` by
  `ColorPickerField`'s color swatch — an anchored, parent-relative overlay with no
  frame/always-on-top/position control, not usable here); named `TranslationPopup`
  specifically to avoid colliding with that builtin element name. `main()` creates
  **one persistent instance** at startup (not one per trigger, unlike
  `SettingsDialog`, since the popup fires on every hotkey trigger) and reuses it:
  `show_popup()` in `main.rs` re-texts, repositions, and re-shows it each time.
  - **Sizing**: fixed `width: 360px`, reactive `height: content-layout.preferred-height`
    — bound directly to the content `VerticalLayout`'s computed height rather than
    left unbound and hoped to auto-fit, so re-showing the same instance with
    shorter/longer text resizes the actual OS window via Slint's normal reactive
    layout recompute (the same mechanism AppWindow's transcript rows already use),
    with no explicit `Window::set_size()` call needed from Rust.
  - **Positioning**: `platform::window::cursor_position()` (new per-OS free
    function, `XQueryPointer` on Linux / `GetCursorPos` on Windows — trimmed from
    `tagent-cli`'s `WindowManager::is_mouse_over_terminal`, which needed the same
    query internally) feeds `popup.window().set_position(slint::PhysicalPosition::new(x
    + 16, y + 16))`, called *before* `popup.show()`. No monitor-edge clamping —
    accepted limitation for this stage.
  - **Hover-based auto-hide, entirely self-contained in `.slint`**: a `hide-timer :=
    Timer { ... }` *element* (Slint 1.8+ builtin, distinct from the Rust
    `slint::Timer` API) inside `TranslationPopup` itself, polling
    `touch-area.has-hover` (a `TouchArea` layered under the content) — not
    `tagent-cli`'s `XQueryPointer`/window-geometry approach, which only exists
    because that code tracks a terminal window from *outside* it; here the popup
    owns its content directly, so Slint's own hover tracking is enough, no platform
    code involved for this part at all. `main.rs` sets `auto-hide-seconds` and calls
    the `start-hide-timer()` public function; the timer's own `triggered` callback
    waits the configured delay, then re-checks hover once a second (mirroring
    `tagent-cli`'s `hide_terminal_and_restore` polling loop in `translator.rs`) until
    the cursor leaves, at which point it fires a `hide-requested()` callback that
    `main.rs` handles by calling `popup.hide()` — nothing else, no focus juggling at
    that point (see below). Re-triggering while a previous popup is still counting
    down calls the same `start-hide-timer()` again, which resets the deadline via
    the `Timer` element's own `restart()` (documented to reschedule relative to
    *now* even if already running) rather than stacking a second pending hide.
  - **Why the timer lives in `.slint`, not as a `slint::Timer` in `main.rs`**: the
    hotkey path's `on_done` callback (`spawn_translation`'s completion hook, now
    typed `Box<dyn FnOnce(&TranscriptEntry) + Send>` since Stage 6 needs the
    resolved entry to populate the popup) is moved through a background
    `std::thread::spawn` before being called back on the UI thread — and
    `slint::Timer` is `!Send` (confirmed via its own docs), so it cannot be
    captured into that closure at all, regardless of whether it would only ever
    actually run on the UI thread. `slint::Weak<TranslationPopup>` has no such
    restriction (component weak handles are `Send`, the same reason `AppWindow`'s
    weak already crosses this exact boundary for the transcript push), so it's what
    `show_popup()` actually carries across; the `Timer` itself never leaves
    `app.slint`.
  - **Focus handling**: `popup.show()` takes OS focus on most window managers.
    `show_popup()` captures the current foreground window via
    `platform::window::foreground_window()` *before* calling `popup.show()` and
    stores it in the `POPUP_RESTORE_TARGET` thread-local; `on_hide_requested`
    hands focus back via `set_foreground_window()` when the popup auto-hides.
    Restoring right after `show()` was the original design (so a hotkey re-trigger
    while the popup is on screen would still copy from the real source app), but
    on Linux `set_foreground_window` also raises the target via `XMapRaised`, and
    since the popup sits right over the app the user was just using, that put the
    source app straight back on top of it and hid it. The narrower gap this
    reopens (a re-trigger while the popup is still visible can copy from the
    popup) is the same one `tagent-cli`'s terminal popup already lives with.
  - **`popup_auto_hide_seconds`** (`GuiConfig`, default `3`, live-reloaded — read
    fresh via `check_and_reload()` on every hotkey trigger, unlike `translate_hotkey`
    which is parsed once at startup): `0` is clamped to the default
    (`GuiConfig::popup_auto_hide_seconds_or_default()`) rather than meaning "never
    auto-hide" — unlike `tagent-cli`'s `auto_hide_terminal_seconds: 0`, safe there
    because the terminal has a normal frame the user can close manually, this popup
    is `no-frame` and deliberately has no close affordance, so `0` would otherwise
    leave it stuck on screen for the process's life. Settings > "Hotkeys & Tray"
    (Stage 8, shipped 2026-09-16) exposes this as a `0`-`60` spinbox with a
    "(0 = default 3s)" hint, keeping that same normalization rather than fighting
    it — the dialog shows the raw stored value, not `3`, so a hand-edited `0`
    isn't silently rewritten just by opening and re-saving Settings.
  - **Dragging and remembered position**: the popup's `TouchArea` reports
    `drag-started`/`drag-moved`/`drag-ended` (no payload) and `wire_popup_drag()` in
    `main.rs` moves the window from Rust, since Slint can't set a window's position
    from `.slint`. Pointer positions from Slint are relative to the popup, which
    moves under the pointer with every step, so each step instead reads the
    *global* cursor (`platform::window::cursor_position`) and places the popup at
    `position at press + (cursor now − cursor at press)` — no feedback, no jitter.
    A 3px threshold (`popup_position::DRAG_THRESHOLD_PX`) separates a drag from a
    click, so a shaky click neither nudges the popup nor saves a position. Each
    press/move calls `start-hide-timer()` and the hide timer also treats
    `touch-area.pressed` as engagement, because hover tracking isn't reliable while
    the button is held (the same pointer grab behind the wheel-scroll fix). Dragging
    always works; `remember_popup_position` (default off — cursor placement, as
    before) only controls whether `drag-ended` saves the last position to
    `popup_position`, re-reading the setting from the live config at that moment.
    `show_popup()` then prefers that saved position over the cursor, clamped with
    `popup_position::clamp_to_bounds` against `platform::window::virtual_screen_bounds`
    (X11 default screen size; `SM_*VIRTUALSCREEN` on Windows; `None` on macOS, i.e.
    unclamped) since the monitor layout may have changed and the no-frame popup has
    no way to be recovered by hand. The bounds are the *bounding box* of all
    monitors, so a saved point in the empty corner of an L-shaped layout is not
    caught; and cursor placement itself is still unclamped, as before. Saving the
    Settings dialog reads `popup_position` fresh from the live config rather than
    from the copy taken when the dialog opened, so a drag made while the dialog
    was open isn't overwritten.
  - **`platform::window` module** (`tagent-gui/src/platform/{linux,windows,macos}/window.rs`):
    three free functions — `cursor_position`, `foreground_window`,
    `set_foreground_window` — rather than a struct with a cached `Display`/handle
    like `tagent-cli`'s `WindowManager`, since each is called once per hotkey
    trigger, not a hot path. Linux opens/closes its own short-lived X11 `Display`
    connection per call; `foreground_window`/`set_foreground_window` read/send
    `_NET_ACTIVE_WINDOW`, ported from `tagent-cli`'s `get_active_window`/
    `send_active_window_message`. Windows uses `GetCursorPos`/`GetForegroundWindow`/
    `SetForegroundWindow`/`IsIconic`+`SW_RESTORE`, all already available from
    Stage 4/5's `windows` crate feature set — no new dependency on either platform.
    macOS is a stub (`None`/`None`/`Ok(())`) matching `keycodes.rs`'s rationale: the
    hotkey itself never fires there, so these functions are never actually called,
    but exist with the same signatures so `main.rs`'s wiring stays OS-independent.
- **System tray** (`TrayIcon` in `app.slint`, Stage 7, shipped 2026-09-15): a
  persistent tray icon built on Slint's own **built-in `SystemTrayIcon` element**
  (Linux: StatusNotifierItem over D-Bus, via the `ksni` crate Slint pulls in
  transitively — confirmed by its appearance in `Cargo.lock` after the version
  bump below, not just by reading Slint's docs; Windows: shell notification area;
  macOS: `NSStatusItem`), not the external `tray-icon` crate the roadmap originally
  assumed — no new GTK/D-Bus/tray dependency of `tagent-gui`'s own. Required
  bumping `slint`/`slint-build` from 1.14.1 to 1.17.1 (`cargo update -p slint -p
  slint-build --precise 1.17.1` — both had to move together in one command, since
  each pins an exact-matching `i-slint-compiler` version and updating only one at
  a time leaves the two in conflict), the minimum version `SystemTrayIcon` exists
  in. The bump also pulled in a new build-time system dependency,
  `libfontconfig1-dev` (via `pkg-config`, needed by the upgraded `fontique`/
  `fontdb` font-matching stack) — not a Rust crate, a system package, so it won't
  show up from `cargo` alone if it's missing.
  - `AppWindow` also gets an `icon: @image-url(...)` property now, pointing at
    the same PNG the tray icon uses — the window previously had no
    window/taskbar icon at all.
  - **Executable icon (Windows)**: `tagent-gui/build.rs` embeds
    `assets/icons/tagent-gui.ico` (7 PNG-compressed sizes, 16-256 px, downscaled from
    `tray.png`) and a version resource through `winresource` (the maintained fork of
    `winres`, a Windows-only build-dependency), so Explorer, shortcuts and a pinned
    taskbar button show the app icon. It is independent of the window/tray icon above,
    which reaches the OS at runtime from `tray.png`. Unlike `tagent-cli`, there is no
    `binary-resources` feature gate: that gate existed to avoid a duplicate VERSION
    resource when a GUI crate depended on the CLI crate, and `tagent-gui` never does.
    To change the icon, regenerate the `.ico` from the new master PNG (all sizes in one
    file) and rebuild.
  - **Known limitation, confirmed by the user right after this stage shipped**:
    on GNOME (this dev machine's desktop), the dock/taskbar still shows a
    generic gear icon for the running app instead of this one. Confirmed via
    Slint's own source (`i-slint-backend-winit`) that the `icon` property
    *does* correctly reach the OS — it's forwarded to
    `winit::window::Window::set_window_icon`, which sets `_NET_WM_ICON` on
    X11. The gap is on GNOME Shell's side: its dock matches a running window's
    `WM_CLASS` against an *installed `.desktop` file*'s `Icon=`/
    `StartupWMClass=` fields for the taskbar/dash representation, and falls
    back to a generic icon when no match exists — it doesn't use
    `_NET_WM_ICON` for that purpose. Neither `tagent-gui` nor `tagent-cli` has
    ever shipped a `.desktop` file, so this is a pre-existing gap, not
    something this stage introduced or could fix by itself; a real fix needs
    `slint::set_xdg_app_id(...)` (for a stable `WM_CLASS`) plus an installed
    `.desktop` file — packaging/installation work, out of scope for Stage 7.
    Decided with the user (2026-09-15) to leave this documented rather than
    patch it now; revisit as part of a future packaging/installation stage.
    The system tray icon itself (`TrayIcon`, above) is unaffected — it's a
    separate GNOME UI surface (the status area, via StatusNotifierItem/D-Bus)
    that carries its own icon data directly, no `.desktop` lookup involved.
  - `TrayIcon inherits SystemTrayIcon`, declared in `app.slint` like
    `Dialog`/`Timer` — `Menu`/`MenuItem`/`MenuSeparator` turned out to be core
    language elements needing no `std-widgets` import, confirmed by the plain
    `SystemTrayIcon`-referencing component compiling as written (same kind of
    check Stage 3 had to do for `Dialog`). One instance is constructed in
    `main()` and kept alive for the process's whole lifetime, same as `popup`
    (Stage 6) — per the element's own docs, the icon appears as soon as the
    instance exists and an event loop is running, and disappears when it's
    dropped, so nothing calls `.show()`/`.hide()`/`.set_visible()` on it.
    Deliberate: Slint's own changelog lists a `show()`/`hide()`-on-
    `SystemTrayIcon` fix as `[1.18.0] - Unreleased` — not in the 1.17.1 this
    project pins — so those methods are avoided entirely rather than risked.
  - Menu: "Show Tagent" (left-click does the same) calls
    `window.show()`; "Settings…" calls `window.invoke_settings_requested()` —
    firing the *existing* callback already registered via
    `window.on_settings_requested(...)` (main.rs) rather than duplicating that
    ~300-line body. This relies on Slint's standard generated `invoke_<name>`
    method for an ordinary `callback` (not the `public function` mechanism
    `invoke_apply_theme`/`invoke_start_hide_timer` elsewhere in this file use);
    confirmed only by the call compiling in this session, not by seeing it fire
    at runtime (no live launch — see below). "Quit" calls
    `slint::quit_event_loop()` and is now the **only** way to fully exit.
  - `AppWindow`'s own close button is redirected via
    `window.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow)`
    to hide instead of quit — a real behavior change for existing users, not
    just an addition.
  - `main()`'s tail changed from `window.run()?` (show + run + quit-on-hide) to
    an explicit `if !start_minimized { window.show()?; }` followed by
    `slint::run_event_loop_until_quit()?` — the loop must keep running even
    with `AppWindow` fully hidden (from `start_minimized` at launch, or from
    the user closing it later), which `run_event_loop()` (quits when the last
    window closes) would not do.
  - `GuiConfig.start_minimized: bool` (default `true`) controls only the
    *next* launch, read once at startup like `translate_hotkey`, not
    live-reloaded. Unlike `translate_hotkey`/`popup_auto_hide_seconds` (Stages
    5/6, hand-edit-only at the time this stage shipped — Stage 8 later gave
    both a `SettingsDialog` control too), this field got a real
    checkbox in `SettingsDialog`'s "Hotkeys & Tray" tab immediately, in this
    same stage — a `default: true` field with no in-app way to turn it back
    off would otherwise compound with the tray-icon-might-not-appear risk
    below into a genuine dead end.
  - **Known environment limitation, not a bug**: Slint's Linux tray backend
    needs a StatusNotifierItem host (e.g. GNOME's "AppIndicator and
    KStatusNotifierItem Support" extension) — plain X11 system trays aren't
    supported. On a desktop with no such host running, the icon simply never
    appears, silently. The escape hatch is the global hotkey (Stage 5), which
    never depended on window or tray visibility and still works with both
    invisible.
  - **Platform scope note**: the hotkey (Stage 5) and popup (Stage 6) stay
    Linux + Windows only, matching `tagent-cli`'s macOS posture — but the tray
    itself also works on macOS, since Slint's element is genuinely
    cross-platform and macOS's manual Translate-button flow already works
    today. A deliberate revision of the "Linux + Windows only" scope stated
    for this cluster when Stage 7 was first drafted, not an oversight.
  - **Verification note**: same "no live launch" policy as Stages 5/6
    ([[feedback_gui_automation_risk]]) — build/test/clippy/fmt (Linux) plus
    `cargo check`/`cargo test --no-run --target x86_64-pc-windows-gnu` all
    passed, but the tray icon's actual appearance, the close-to-tray/restore
    round trip, and Quit actually exiting were not observed running in this
    session.
- **Remember window size/position** (`GuiConfig.remember_window_geometry`/
  `window_geometry`, `main.rs`'s `show_window_restoring_geometry`/
  `save_window_geometry`, Stage 7 follow-up, shipped 2026-09-15): captured on
  hide-to-tray (`on_close_requested`) and on Quit (only if the window is
  actually visible at that moment, so a never-shown or already-hidden window
  doesn't overwrite a good saved value with nothing meaningful), restored once
  per run the first time the window is shown — later re-opens from the tray
  leave the window exactly as the user last had it, rather than snapping back
  to the saved value on every show. `set_size`/`set_position` are both called
  *after* `.show()`, following the same rule Stage 6's popup already
  established (setting them before `.show()` is silently ignored on this
  project's X11 setup, since no OS-level window exists yet).
  - **A real bug found and fixed while verifying this, unrelated to the
    feature's own logic**: this session, unlike Stages 5/6, *did* launch the
    real binary live (with the global hotkey deliberately disabled via an
    isolated config — `"translate_hotkey": "DISABLED"` fails to parse and
    logs a warning, same graceful-degradation path already documented above
    — so `XGrabKey` was never invoked; a deliberate, narrower exception to
    the "no live launch" policy, not a reversal of it). Across repeated fresh
    launches, the main window intermittently (2 of the first 7 observed)
    opened at a much smaller size (`458×188`) than its configured `480×480`
    default — a winit/X11 initial-window-sizing race with mutter, reproduced
    even with this feature's own restore logic completely bypassed (a plain
    `window.show()?` hit it too), so it predates this feature and isn't
    specific to it. Fixed by having `show_window_restoring_geometry`
    explicitly re-assert a `DEFAULT_WINDOW_SIZE` constant (`480×480`, kept in
    sync with `AppWindow`'s `preferred-width`/`preferred-height` in
    `app.slint` by a doc-comment cross-reference, not a shared source of
    truth) via `set_size()` whenever there's no saved geometry to restore
    instead — confirmed 6/6 clean launches after the fix (0/7 before it).
    This likely explains the plain square-window-size change (`0.14.0+021`)
    appearing not to take effect when the user first tested it, and quite
    possibly also explains Stage 3's older, vaguer "the window manager
    appears to override preferred-width/preferred-height regardless" note —
    same underlying race, not a hard WM override as that note assumed.
  - **`save_window_geometry` persisting on a real close**: confirmed working
    end-to-end via the user's own live testing (four consecutive real
    launches with debug logging) — every run's "captured" geometry on
    close/quit correctly appeared as the next run's loaded `window_geometry`.
  - **A second, distinct bug found via that same live testing, fixed in
    `0.14.0+023`**: the restore only actually worked when
    `show_window_restoring_geometry` ran *before* `run_event_loop_until_quit()`
    (i.e. the `!start_minimized` startup path). With `start_minimized` on
    (the default), the window's first-ever `show()` instead happens from
    inside the already-running event loop, via the tray's "Show Tagent"
    click — delivered as a D-Bus/ksni callback. In that context the
    synchronous `set_size`/`set_position` calls were silently dropped: the
    window stuck at the same race-default size (`458×188`) described above
    and never settled, even given seconds of dwell time. Confirmed by
    reproducing the tray-click path directly — driving the tray icon's
    `org.kde.StatusNotifierItem.Activate` method over D-Bus (`busctl --user
    call ... Activate ii 0 0`) — versus the plain startup path, which settled
    correctly within ~0.5s in the same test harness. Fixed by having
    `show_window_restoring_geometry` re-issue the same `set_size`/
    `set_position` calls a second time, ~150ms later, via
    `slint::Timer::single_shot` (confirmed safe here — no `Send` bound
    required, unlike Stage 6's popup hide-timer which specifically avoided
    `slint::Timer` because its callback crosses a `Send`-bounded background
    thread). Verified via the same D-Bus-driven reproduction: saved-geometry
    restore and the no-saved-geometry default both land correctly through
    the tray-click path after the fix.
- **Dictionary lookup** (`tagent-gui/src/dictionary.rs`, Stage 9, shipped
  2026-09-18): single-word input now takes a dictionary path instead of a plain
  translation, when `GuiConfig.show_dictionary` (default `true`) is on. Fully
  duplicated from `tagent-cli`'s `translator.rs`/`config.rs` rather than shared
  (Open Question 3 in the development plan, resolved 2026-08-15 — `tagent-gui`
  never depends on `tagent-cli`): `is_single_word`, `correction_notice`, and
  `format_dictionary_entry` (plus its private `get_full_part_of_speech` table
  for all 7 target languages) all live in the new module. **Key design
  decision**: rather than a new UI surface, the dictionary result becomes the
  *content* of the existing `translation` string — both `TranscriptEntry.translation`
  (main window) and `TranslationOutcome.translation_raw` (the Stage 6 popup)
  already carry arbitrary text through the same `format_line`/wrap-and-display
  pipeline a plain translation uses, so `spawn_translation` (`main.rs`) is the
  only place the dictionary-vs-plain-translation branch lives; both of its
  callers (the button/Enter path and the Stage 5 global hotkey) get it for
  free, and `TranscriptEntry`/`TranslationOutcome`/`show_popup`/the popup's
  `.slint` layout needed no changes at all. Two pieces of earlier work had
  already anticipated this: the Stage 6 popup's `ScrollView`+`popup-max-height`
  cap and its fully independent `popup_font`/`popup_size`/`popup_color`/
  `popup_background` style (both doc-commented "Stage 9" ahead of time) were
  built to carry a multi-line dictionary block, not just a one-line
  translation.
  - Inside `spawn_translation`'s async block: when `show_dictionary` is on and
    `dictionary::is_single_word` accepts the (trimmed) text, `provider.translate_text`
    and `dictionary_provider.lookup` run concurrently via `tokio::join!`
    (mirroring `tagent-cli`'s `Translator::get_dictionary_entry`). On
    `Ok(Some(entry))`, the block's text is `format_dictionary_entry`'s output,
    with `spell_check`'s correction notice (`dictionary::correction_notice`)
    prepended when the provider silently corrected a misspelling. On a
    dictionary miss or lookup error, the fallback is the `translate_result`
    already sitting in hand from the same `join!` — **not** a second network
    call, unlike `tagent-cli`'s own fallback (which re-fetches because its
    dictionary lookup and its regular-translation path are two separate
    functions with no shared result to reuse — `spawn_translation` has both
    results as the identical `Result<String, tagent::error::Error>` type from
    one `join!`, so the miss case is just `_ => translate_result`).
  - `format_dictionary_entry` deliberately omits the looked-up word as its own
    line (unlike `tagent-cli`'s `cli_mode: false` branch, which is otherwise
    unused dead code in that crate) — `tagent-gui`'s two-pane phrase/translation
    layout already shows the word on the phrase line above, so restating it
    would be redundant. Named tradeoff: with `popup_show_phrase` off, the
    popup then shows *only* the translation block, so the looked-up word never
    appears there — accepted, since the block's own header line (the plain
    translation, or the first definition's text when that translation
    request itself failed) is already the useful answer on its own. Each
    popup toggle is still reasoned about independently rather than as a
    combination, same class of tradeoff already flagged once for Stage 5's
    hotkey vs. Record capture interaction.
  - **Trim fix in scope, not new scope**: `spawn_translation` now trims `text`
    unconditionally as its first step. Before this, the global hotkey path
    passed the clipboard text through `ClipboardManager::get_text_with_copy()`
    **untrimmed** (only its emptiness check was trimmed), while the button/
    Enter path already trimmed before constructing its `TranslationRequest` —
    an asymmetry invisible before Stage 9 (nothing compared the raw string to
    anything) but one this feature would otherwise have surfaced as a real
    bug: an untrimmed selection like `"violent\n"` still passes
    `is_single_word` (which itself strips non-alphabetic edge characters), so
    it takes the dictionary path, and comparing `"violent"` (from the
    provider) against `"violent\n"` (the untrimmed original) would then
    spuriously report a spelling correction that never happened.
  - `show_dictionary`/`spell_check` are two new `GuiConfig` fields (both
    default `true`, matching `tagent-cli`'s `ShowDictionary`/`SpellCheck`),
    read under the same `check_and_reload()` lock as `translate_provider`/
    `show_prompt` at both `spawn_translation` call sites — live-reloaded, no
    restart needed, same as those two. Settings UI: two checkboxes on the
    General tab, next to the provider dropdown, with an inline hint noting the
    live-reload and that `spell_check` has no effect while `show_dictionary`
    is off (kept as two independent checkboxes rather than one disabling the
    other).
- **Text-to-speech playback** (Stage 10, shipped 2026-09-18): every transcript
  row gets two 🔊 speaker buttons — one for the phrase, one for the translation
  (hidden when `entry.translation-is-error` or the entry has no
  `translation-speech`, e.g. a failed translation). `TranscriptEntry` grew four
  fields to carry what speech needs that the existing `phrase`/`translation`
  display strings can't (already prompt-formatted, and never had a language
  code at all): `phrase-speech`/`translation-speech` (raw, unprompted text —
  for a Stage 9 dictionary hit, `translation-speech` is just the primary
  line via a new `dictionary::primary_line` helper, extracted out of
  `format_dictionary_entry`'s own header-line logic so both agree — never the
  full part-of-speech/synonym block) and `from-code`/`to-code` (the resolved
  provider codes; `from-code` may be `"auto"`, `to-code` never is). New
  `tagent-gui/src/speech.rs` module (`speak()`, ported from `tagent-cli`'s
  `speech.rs` minus the terminal-specific Esc monitor/label printing — a
  global Esc poll would swallow Esc app-wide in a windowed app, and
  `tagent-gui`'s own Linux `platform::keycodes` deliberately has no
  `is_key_pressed`) plays audio via the same `rodio` `OutputStreamBuilder` →
  `Sink` shape `tagent-cli` already uses, with `rodio = { version = "0.21",
  features = ["symphonia-mp3"] }` as a new, platform-independent dependency
  (unlike the hotkey/popup/tray features, this needs no OS-specific code —
  works on macOS too). Only one button plays at a time app-wide: `AppWindow`'s
  `speaking-entry-index`/`speaking-is-phrase` properties are shared, single
  state (not per-entry), so every other row's button disables itself while one
  plays; the active button turns into "⏹" and a second click on it sets a
  shared `Arc<Mutex<Option<Arc<AtomicBool>>>>` stop flag that the playback
  thread polls between chunks. `from-code == "auto"` is resolved lazily at
  speak-click time via `providers::resolve_source_language` (a no-op
  pass-through otherwise; since Stage 11 the translate provider it needs is
  only constructed for `"auto"` — see "Speech Provider Architecture" above), rather than trying to recover what Google actually
  detected at translation time (`translate_text` doesn't hand that back). New
  `enable_text_to_speech` `GuiConfig` field (default `true`, matching
  `tagent-cli`'s `EnableTextToSpeech`, live-reloaded), gating a `tts-enabled`
  window property re-set at every point that already reads config
  (`on_translate_requested`, the hotkey path, Settings save) plus once at
  startup; Settings checkbox on the General tab next to Stage 9's two.
- **Speech hotkey** (Stage 10 follow-up, shipped 2026-09-18): a second global
  hotkey (default `Alt+S`; `tagent-cli`'s default was `Alt+E` at the time and
  was changed to `Alt+S` too in `tagent-cli` `0.16.0+001`) that speaks the current selection directly, with
  Esc cancellation and the event logged to the transcript. `KeyboardHook`
  (Linux/Windows/macOS) grew from one hotkey to two plus an Escape-observation
  callback:
  - **Invariant: Escape is only ever observed, never grabbed.** On Linux,
    `XGrabManager::grab_hotkey` is only ever called with the two *configured*
    hotkeys; Escape rides the same passive `rdev` event stream already running
    for hotkey detection, never touching `XGrabKey`. On Windows,
    `keyboard_hook_proc`'s Escape branch calls the new escape callback and
    always falls through to `CallNextHookEx` — it must never `return
    LRESULT(1)`, or Escape would stop reaching every other application on the
    system for as long as `tagent-gui` runs. Also must still trigger
    `replay_pending_swallowed_modifiers()` if a combo modifier (Alt) is
    mid-swallow -- Escape arriving is exactly "some other, non-trigger key
    arrived while Alt was held."
  - **Windows statics**: `HOTKEY`/`ON_TRIGGER` renamed
    `TRANSLATE_HOTKEY`/`ON_TRANSLATE_TRIGGER`; new
    `SPEECH_HOTKEY: OnceLock<Option<HotkeyState>>` (always set, to `Some`/
    `None`, never left empty), `ON_SPEECH_TRIGGER`, `ON_ESCAPE`.
    `COMBO_MODIFIER_VKS` becomes the **union** of both hotkeys' modifiers
    (`union_combo_modifiers`, split out as its own pure function so the union
    logic is unit-testable without touching any `OnceLock` or spawning a real
    hook thread) — for the requested defaults (`Alt+A`, `Alt+S`) the union is
    still just `{Alt}`, but the code computes it generically.
    `HotkeyState::handle`'s `trigger_fn: fn()` parameter is a bare function
    pointer (existing design, can't capture "which `ON_*_TRIGGER`"), so
    `trigger_translate`/`trigger_speech`/`trigger_escape` are three separate
    top-level `fn`s rather than one shared closure.
  - **`tagent-gui/src/main.rs`**: `on_speak_requested`'s "nothing is
    currently speaking, start a new playback" branch was extracted into a
    standalone `start_speaking(window, config_manager, speech_stop_flag,
    weak, SpeakRequest { .. })` function (params grouped into a struct once
    the extraction pushed the count past clippy's `too_many_arguments`
    threshold, mirroring `TranslationRequest`'s own reason for existing) —
    the speech hotkey's trigger handler pushes a new `[Speech]: <text>`
    transcript entry for whatever it grabbed from the clipboard, then calls
    `start_speaking` with that entry's own freshly-assigned index and
    `is_phrase: true`, exactly as if the user had clicked that row's own 🔊
    button. This means Esc cancelling *any* currently-speaking entry (not
    just hotkey-triggered ones) and re-pressing the speech hotkey while
    something is already speaking being a no-op (deliberately asymmetric
    with the transcript buttons' toggle-to-stop -- the user asked
    specifically for Esc-cancellation) both fall out of reusing Stage 10's
    single shared `speech_stop_flag`/`speaking-entry-index` state, rather
    than being separately implemented. `enable_text_to_speech` is read fresh
    at trigger time (a deliberate improvement over `tagent-cli`'s own
    speech-hotkey path, which only checks its equivalent gate once, at
    startup); `speech_hotkey`/`enable_speech_hotkey` themselves stay
    restart-gated for *registration*, like `translate_hotkey`.
    `recording_started_at`'s Settings-recording suppression (Stage 8
    follow-up) now guards the speech trigger too, not just translate --  the
    same interference bug applies symmetrically to a second hotkey.
  - **`app.slint`**: `HotkeyRecorder` (already a generic, reusable component,
    not translate-specific) gets a second instantiation on the "Hotkeys &
    Tray" tab. `is-recording`/`hotkey-edited`/`hotkey-unrecognized` renamed
    `is-recording-translate`/`translate-hotkey-edited`/
    `translate-hotkey-unrecognized` for symmetry with the new
    `is-recording-speech`/`speech-hotkey-edited`/`speech-hotkey-unrecognized`;
    `map-key-to-token` stays a single, shared callback (a stateless key-name
    lookup with no per-field behavior); `recording-changed(bool)` stays a
    single callback, now fired with "is *either* field currently recording."
  - New `TranscriptEntry` for a `[Speech]` row deliberately does **not**
    reuse `info_transcript_entry` (which zeroes `phrase_speech` too) --
    `phrase_speech` is set to the spoken text itself, giving the row a
    genuinely working replay button; `translation`/`translation_speech` stay
    empty with `translation_is_error: true` to hide the (inapplicable)
    translation button.
- **Scope**: a bare-bones translate-only prototype — no history logging (TTS
  playback shipped at Stage 10, above). `app.slint` hardcodes a 6-language
  list (Auto/English/Russian/Spanish/French/German), much smaller than the
  ~16 languages `config.rs` supports for CLI/interactive mode.
- **Detaching from the terminal** (`tagent-gui/src/detach.rs`, `#[cfg(unix)]`,
  0.14.0+013): a Linux/macOS launch from a terminal would otherwise hold it until
  Quit. First thing in `main` (before any thread exists, since it may `exit`),
  `detach_from_terminal()` re-spawns `current_exe()` with the same arguments plus
  `--foreground`, in a new session (`libc::setsid` in `pre_exec`, so no controlling
  terminal: no SIGHUP on terminal close, no Ctrl+C), with stdin from `/dev/null`
  and stdout/stderr appended to `dirs::data_dir()/tagent-gui/tagent-gui.log`
  (truncated first once it is over 1 MiB), then the parent exits 0. The appended
  `--foreground` is how the child knows not to detach again, and users pass it (or
  `-f`) to stay attached. It only detaches when a standard stream is a terminal, so
  desktop launchers, autostart and systemd units (which track the PID they started)
  are unaffected. A failed re-spawn warns and carries on in the foreground. Output goes
  to a file rather than staying on the terminal because `eprintln!` panics on a
  write error, and a tty whose terminal has been closed returns EIO. Windows needs none of this, since
  `windows_subsystem = "windows"` already makes shells not wait for the app.

### Known gaps in `tagent-gui`

- **Empty tray menu after a slow start (Linux, GNOME)** — known, left as is
  (2026-09-24). Occasionally the tray icon shows but right-click opens nothing,
  so the only way to quit is `pkill tagent-gui`. Restarting fixes it. It happens
  when startup is slow, typically the first launch of a freshly built binary
  (cold disk cache), and it predates the terminal detach. It can be reproduced by
  evicting the binary from the page cache (`posix_fadvise(POSIX_FADV_DONTNEED)`)
  before launching.
  - **Cause**: Slint 1.17's `ksni` backend
    (`i-slint-core/items/system_tray/ksni.rs`) registers the
    StatusNotifierItem with an **empty** menu and fills it in right afterwards via
    `Handle::update`. `ksni` does emit `com.canonical.dbusmenu.LayoutUpdated` for
    that. But if GNOME's `ubuntu-appindicators` extension reads the layout inside
    the gap, it keeps the empty menu. The app side is fine the whole time:
    `busctl --user call org.kde.StatusNotifierItem-<pid>-1 /MenuBar
    com.canonical.dbusmenu GetLayout iias 0 -- -1 0` returns the full menu.
  - **Why no workaround**: re-registering the icon once after startup (dropping
    and recreating the item, which then carries the built menu from the start) was
    tried and rejected. It needs a guessed delay, it makes the icon flicker on
    every launch, and it depends on Slint internals. An unbound `visible` on
    `SystemTrayIcon` is compiled as a constant, so `TrayIcon::hide()` panics with
    "Constant property being changed". The proper fix is upstream, where Slint
    would register the item only once the menu is built. Reported as
    [slint-ui/slint#13624](https://github.com/slint-ui/slint/issues/13624)
    (2026-09-24). Once a Slint release fixes it, bump `slint`/`slint-build`
    together and remove this entry.

- **Language list and history logging aren't configurable at all yet** — no
  field exists for either (the 6-language list stays hardcoded).
  `tagent-cli.conf` is not read at all any more (no migration path — see the
  "own configuration" concept in the development plan). (`translate_hotkey`
  and `popup_auto_hide_seconds` used to be listed here as hand-edit-only —
  Stage 8, shipped 2026-09-16, gave both a "Hotkeys & Tray" tab control; TTS
  settings used to be listed here too — Stage 10, shipped 2026-09-18, gave
  `enable_text_to_speech` a General-tab checkbox.) `speech_provider` (Stage 11,
  2026-09-19) is currently hand-edit-only: no Settings dropdown while `"google"` is
  the only registered speech backend, same precedent as `translate_provider` itself
  before Stage 3. `dictionary_provider` (Stage 12, 2026-09-20) was hand-edit-only in the
  same way. Both got General-tab dropdowns on 2026-09-20, next to the translate-provider
  one (see the Settings dialog section above), so no `GuiConfig` field is carried through
  a Settings save any more for lack of a control.
- **macOS has no way to copy transcript text** since Stage 13 (2026-09-22) made the
  transcript view-only: `platform::macos::clipboard::ClipboardManager::set_text` is a stub
  that always returns `Err`, so the new right-click "Copy" menu silently fails there (a
  warning is logged) where it works on Linux/Windows. Before Stage 13, Ctrl+C worked in the
  transcript's plain `TextInput` via Slint's own built-in clipboard handling, independent of
  `ClipboardManager`; that path is gone along with the `TextInput`. Accepted and documented
  rather than fixed (`tagent-gui development plan.md`'s Stage 13 open item 1) -- macOS is a
  stub platform throughout the GUI already.
- It calls `TranslationProvider::translate_text`, `DictionaryProvider::lookup` and
  `SpeechProvider::split_for_speech`/`speak_chunk` directly rather than going through
  `tagent-cli`'s `Translator`/`SpeechManager` orchestrators; dictionary/
  spell-check display shipped at Stage 9 (2026-09-18) and text-to-speech
  playback at Stage 10 (2026-09-18), both via `tagent-gui`'s own independent
  modules (`dictionary.rs`, `speech.rs`).

## `build.rs`: version sync

`tagent-cli/build.rs` runs on every `cargo build` of the `tagent-cli` package (it does
not run for `tagent`, the library, which has no build script; nor for `tagent-gui`,
which has its own single-line `build.rs` that only compiles the Slint UI — see above)
and:

1. Reads the version from `tagent-cli/Cargo.toml` (`CARGO_PKG_VERSION`, format
   `MAJOR.MINOR.PATCH[+BUILD]`).
2. `sync_version_in_docs()`: pattern-matches and rewrites version strings in
   `tagent-cli/README.md` (its own package-local README, since the move to a
   three-crate workspace — see "Workspace layout" above), `CHANGELOG.md` (also
   package-local; it used to be the workspace-root `CHANGELOG.md` until each crate got
   its own changelog), and `../CLAUDE.md` — paths are relative to `tagent-cli/` (the
   package's manifest dir, where `build.rs` actually runs from), *not* the workspace
   root, so `CLAUDE.md` is the one file that lives one level up from the package.
   (The thin root `README.md` and the new `tagent/README.md` /
   `tagent-gui/README.md` are version-agnostic signposts — none of them contain a
   version string, so none are build.rs sync targets.) Skips the write if the value is
   already current, to avoid needless rebuilds/timestamp churn. The `CHANGELOG.md`
   pattern explicitly skips over a `## [Unreleased]` header — see
   `update_version_in_file`'s `Unreleased]` guard — so it never overwrites that
   section's content when scanning forward for the next `] - ` (regression-tested in
   `build.rs`'s own `#[cfg(test)]` module). **Silent failure mode**: if any of these
   relative paths is wrong, `update_version_in_file` just returns `Ok(())` and skips
   that file — no build error, no warning. Verify a version-bump build actually touched
   the docs by diffing them, not by the build succeeding.
3. On Windows only, when the `binary-resources` feature is active, embeds the app icon
   (`assets/icons/taa_256.ico`, inside the `tagent-cli/` package itself) and version resource
   via `winres`.

There is no GUI-specific version sync step: an earlier Tauri-based `tagent-gui`
prototype had one (writing into `tagent-gui/src-tauri/Cargo.toml` etc.), but it was
removed once `tagent-gui` moved to Slint and that Tauri layout stopped existing.
`tagent-gui`'s own version is whatever is in `tagent-gui/Cargo.toml`
(currently `0.14.0`) and is not synced by anything. As of the 2026-08-15 independence decision (see "Concept" at the top of the
`tagent-gui` section above), this is deliberate rather than merely unaddressed:
`tagent-gui` versions on its own track — `MAJOR.MINOR.PATCH+BUILD` like `tagent-cli`, but with
its own independent counter, and the `+BUILD` is stripped at release — and logs its history
in its own [`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md), separate from
[`tagent-cli/CHANGELOG.md`](../tagent-cli/CHANGELOG.md), which `tagent-cli/build.rs`
syncs into. Every crate has its own changelog next to its `Cargo.toml`; there is no
workspace-root one. The `tagent` library crate's version (`0.18.1`) is likewise
standalone, plain semver with no `+BUILD` suffix, with history in
[`tagent/CHANGELOG.md`](../tagent/CHANGELOG.md). It is deliberately pre-1.0: the API is
still moving (three provider traits, more providers to come), and under semver's `0.y.z`
rules a minor bump is the place for breaking changes, so `0.17` → `0.18` for a breaking
change and `0.17.0` → `0.17.1` for a compatible addition or fix. It is bumped manually.
`1.0.0` waits until the API settles and the crate is published. `0.17.0` still sorts
above `0.12.0`, the last version the old single-crate `tagent` application published to
crates.io (checked 2026-09-19), so `cargo add tagent` will resolve to the library rather
than the old app once it is published (not done yet).

## Other known gaps worth knowing about

- **`[Colors]` and `[Speech]` config sections** (`SourcePromptColor`, `TargetPromptColor`,
  `DictionaryPromptColor`, `EnableTextToSpeech`, `SpeechHotkey`, `EnableSpeechHotkey`)
  exist in `config.rs` and are used by CLI/interactive/keyboard-hook code, but have no
  equivalent in `tagent-gui`.
