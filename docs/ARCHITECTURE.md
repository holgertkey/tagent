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

- **`providers`** — the `TranslationProvider` trait and `create_provider()` factory,
  moved essentially unchanged from the old `src/providers/`. Every method returns
  `Result<_, tagent::error::Error>` instead of the old `Box<dyn Error + Send + Sync>`.
  `GoogleTranslateProvider` also implements the trait's two TTS methods,
  `split_for_speech(text) -> Vec<String>` and `async fn speak_chunk(text, lang) ->
  Result<Vec<u8>, Error>` — this two-method split (rather than one `speak()` returning
  every chunk's audio up front) exists specifically so `tagent-cli`'s playback loop can
  keep fetching and playing chunks one at a time, matching the pre-refactor behavior of
  audio starting after the first chunk's round-trip instead of after all of them.
  `split_for_speech` returns short input (≤100 chars, Google TTS's per-request limit)
  verbatim as a single chunk without running it through sentence-splitting, preserving
  exact punctuation for the common case.
- **`languages`** — `name_to_code()` / `code_to_name()`, a straight move of what used
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

- **UI framework**: [Slint](https://slint.dev/), via `tagent-gui/ui/app.slint` and the
  `slint`/`slint-build` crates. `tagent-gui/build.rs` is a single line:
  `slint_build::compile("ui/app.slint").unwrap();`.
- **Dependency on the `tagent` library**: `tagent-gui/Cargo.toml` depends on
  `tagent = { path = "../tagent" }` — the pure library crate (see "Workspace layout"
  above), not `tagent-cli`. Unlike before this crate existed, there is no
  `binary-resources` feature to disable here: `tagent` never runs `winres`, so there's
  nothing Windows-resource-related to guard against.
- **Reading `TranslateProvider` without `ConfigManager`**: since `tagent` has no config
  module, `tagent-gui/src/main.rs` has its own small `read_translate_provider()`
  function — opens `tagent-cli.conf` via `dirs::config_dir()`, scans for `[Provider]` /
  `TranslateProvider = ...`, defaults to `"google"` on any miss. This is a deliberate,
  narrowly-scoped exception to reusing `tagent-cli`'s logic: pulling in `ConfigManager`
  would mean pulling in all of `tagent-cli` (rustyline, rdev, x11, arboard, ctrlc, the
  whole `platform/` tree), just to read one string.
- **How translation works**: `main()` calls `read_translate_provider()` once at startup
  and captures the result. The `translate-requested` Slint callback spawns a plain OS
  thread with its own fresh `tokio::runtime::Runtime`, calls
  `tagent::providers::create_provider(&translate_provider)`, calls
  `provider.translate_text(...)`, then marshals the result back onto the Slint UI thread
  via `slint::invoke_from_event_loop`. A `to == "auto"` request is rejected before
  spawning the thread (both the target `ComboBox` and the ⇄ swap button can otherwise
  produce one), appending an in-transcript error instead of calling the provider.
  Language names from the UI are resolved to codes via `tagent::languages::name_to_code`.
- **Transcript pane** (`transcript-scroll` / `transcript-text` in `app.slint`): a
  read-only, multi-line `TextInput` (not a plain `Text`), so its content is
  mouse-selectable and copyable. `scroll_transcript_to_bottom()` in `main.rs` sets
  `transcript-viewport-y` to the negative overflow after every new entry so the pane
  auto-scrolls to the latest translation.
- **Input box** (`input-field` in `app.slint`): a multi-line `TextInput` inside its own
  `ScrollView`, wrapped in a resizable container — a 6px drag handle above the box lets
  the user set `input-user-height` between `input-min-height` (32px) and
  `input-max-height` (220px); the box also grows automatically with wrapped content up
  to that cap. `key-pressed` submits on Enter and inserts a newline on Shift+Enter.
  `forward-focus: input-field` on the window root means the input field has focus as
  soon as the window opens, so typing or pasting works without clicking into it first.
- **Scope**: a bare-bones translate-only prototype — no dictionary-entry display, no
  spell-check notices, no TTS button, no clipboard integration, no hotkeys, no history
  logging. `app.slint` hardcodes a 6-language list (Auto/English/Russian/Spanish/French/German),
  much smaller than the ~16 languages `config.rs` supports for CLI/interactive mode.

### Known gaps in `tagent-gui`

- **`tagent-cli.conf` is read once at startup, not live-reloaded.** Unlike `tagent-cli`
  (`ConfigManager::check_and_reload()`, called before every translation), `tagent-gui`
  snapshots `translate_provider` in a local variable in `main()` and never re-reads the
  file, so editing `tagent-cli.conf` while the GUI is running has no effect until restart.
- **Everything else in `tagent-cli.conf` is still ignored** — language list, hotkeys,
  history logging, colors, TTS settings, dictionary/spell-check toggles. Only
  `TranslateProvider` (via the inline reader above) and `tagent::languages` are
  consulted; the rest is either hardcoded (6-language list) or simply unsupported (no
  history, no hotkeys).
- **No dictionary/spell-check/TTS UI** — it calls `TranslationProvider::translate_text`
  directly rather than going through `Translator`'s richer orchestration and formatting.

## `build.rs`: version sync

`tagent-cli/build.rs` runs on every `cargo build` of the `tagent-cli` package (it does
not run for `tagent`, the library, which has no build script; nor for `tagent-gui`,
which has its own single-line `build.rs` that only compiles the Slint UI — see above)
and:

1. Reads the version from `tagent-cli/Cargo.toml` (`CARGO_PKG_VERSION`, format
   `MAJOR.MINOR.PATCH[+BUILD]`).
2. `sync_version_in_docs()`: pattern-matches and rewrites version strings in
   `tagent-cli/README.md` (its own package-local README, since the move to a
   three-crate workspace — see "Workspace layout" above), `../CLAUDE.md`, and
   `../CHANGELOG.md` — the latter two paths are relative to `tagent-cli/` (the
   package's manifest dir, where `build.rs` actually runs from), *not* the workspace
   root, since those two files live at the workspace root, one level up from the
   package. (The thin root `README.md` and the new `tagent/README.md` /
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
   (`../assets/icons/taa_256.ico`, also relative to `tagent-cli/`) and version resource
   via `winres`.

There is no GUI-specific version sync step: an earlier Tauri-based `tagent-gui`
prototype had one (writing into `tagent-gui/src-tauri/Cargo.toml` etc.), but it was
removed once `tagent-gui` moved to Slint and that Tauri layout stopped existing.
`tagent-gui`'s own version is whatever is in `tagent-gui/Cargo.toml`
(currently `0.13.0`, unlinked from `tagent-cli`'s `0.13.0+003`) and is not synced by
anything. The `tagent` library crate's version (`1.0.0`) is likewise standalone, plain
semver with no `+BUILD` suffix — that convention is specific to `tagent-cli`'s
dev-iteration tracking. `1.0.0` was chosen deliberately: it needs to sort above
whatever version the old single-crate `tagent` application last published to
crates.io, so that `cargo install tagent` / `cargo add tagent` resolve to the library
once it is actually published (not done as part of this restructuring), not the old app.

## Other known gaps worth knowing about

- **`[Colors]` and `[Speech]` config sections** (`SourcePromptColor`, `TargetPromptColor`,
  `DictionaryPromptColor`, `EnableTextToSpeech`, `SpeechHotkey`, `EnableSpeechHotkey`)
  exist in `config.rs` and are used by CLI/interactive/keyboard-hook code, but have no
  equivalent in `tagent-gui`.
