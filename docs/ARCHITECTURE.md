# Architecture

This document goes deeper than `CLAUDE.md` on subsystems where a short summary would
hide important detail: the platform abstraction layer, the `tagent-gui` crate, and the
build-time version-sync mechanics. Read `CLAUDE.md` first for the high-level overview;
come here when you need the mechanism, not just the shape.

## Platform abstraction layer

`src/platform/mod.rs` is **not** a `trait`/`dyn` abstraction. It is a `#[cfg(target_os = "...")]`-gated
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
the workspace `Cargo.toml` at all). It compiles and runs, but only interactive/CLI mode
actually works — anyone picking up macOS support starts from these three stub files.

### Linux specifics: `xgrab.rs`

`rdev::listen` only *observes* raw key events — it does not stop them from reaching the
focused application. `src/platform/linux/xgrab.rs` uses X11's `XGrabKey` to grab the
configured hotkey combo at the X server level so the keystroke is consumed by tagent
instead of leaking through. Notable details:

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

`tagent-gui` is a **separate binary crate** in the Cargo workspace
(`Cargo.toml: [workspace] members = [".", "tagent-gui"]`), not a mode of the main
`tagent` binary. There is no flag or code path in `tagent`'s own `main.rs`/`cli.rs`
that launches it.

- **UI framework**: [Slint](https://slint.dev/), via `tagent-gui/ui/app.slint` and the
  `slint`/`slint-build` crates. `tagent-gui/build.rs` is a single line:
  `slint_build::compile("ui/app.slint").unwrap();`.
- **Dependency on the main crate**: `tagent-gui/Cargo.toml` depends on
  `tagent = { path = "..", default-features = false }` — it links against the `tagent`
  **library** crate directly (imports `tagent::{config::ConfigManager, providers}`),
  not a subprocess. `default-features = false` turns off the `binary-resources`
  feature so `tagent-gui`'s build doesn't trigger the Windows icon/version-resource
  embedding meant for `tagent.exe`.
- **How translation works**: `main()` reads `tagent.conf` once at startup via
  `ConfigManager::get_default_config_path()` / `ConfigManager::new()` and captures
  `translate_provider` from it. The `translate-requested` Slint callback spawns a plain
  OS thread with its own fresh `tokio::runtime::Runtime`, calls
  `providers::create_provider(&translate_provider)`, calls `provider.translate_text(...)`,
  then marshals the result back onto the Slint UI thread via
  `slint::invoke_from_event_loop`. A `to == "auto"` request is rejected before spawning
  the thread (both the target `ComboBox` and the ⇄ swap button can otherwise produce
  one), appending an in-transcript error instead of calling the provider.
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

- **`tagent.conf` is read once at startup, not live-reloaded.** Unlike the main
  `tagent` binary (`ConfigManager::check_and_reload()`, called before every
  translation), `tagent-gui` snapshots `translate_provider` in a local variable in
  `main()` and never re-reads the file, so editing `tagent.conf` while the GUI is
  running has no effect until restart.
- **Everything else in `tagent.conf` is still ignored** — language list, hotkeys,
  history logging, colors, TTS settings, dictionary/spell-check toggles. Only
  `TranslateProvider` and the `language_to_code()` helper are consulted; the rest is
  either hardcoded (6-language list) or simply unsupported (no history, no hotkeys).
- **No dictionary/spell-check/TTS UI** — it calls `TranslationProvider::translate_text`
  directly rather than going through `Translator`'s richer orchestration and formatting.

## `build.rs`: version sync

The root `build.rs` runs on every `cargo build` (of the `tagent` binary/library; it does
not run for `tagent-gui`, which has its own single-line `build.rs` that only compiles
the Slint UI — see above) and:

1. Reads the version from `Cargo.toml` (`CARGO_PKG_VERSION`, format `MAJOR.MINOR.PATCH[+BUILD]`).
2. `sync_version_in_docs()`: pattern-matches and rewrites version strings in
   `README.md`, `CLAUDE.md`, and `CHANGELOG.md` (skips the write if the value is already
   current, to avoid needless rebuilds/timestamp churn). The `CHANGELOG.md` pattern
   explicitly skips over a `## [Unreleased]` header — see `update_version_in_file`'s
   `Unreleased]` guard — so it never overwrites that section's content when scanning
   forward for the next `] - ` (regression-tested in `build.rs`'s own `#[cfg(test)]`
   module).
3. On Windows only, when the `binary-resources` feature is active, embeds the app icon
   and version resource via `winres`.

There is no GUI-specific version sync step: an earlier Tauri-based `tagent-gui`
prototype had one (writing into `tagent-gui/src-tauri/Cargo.toml` etc.), but it was
removed once `tagent-gui` moved to Slint and that Tauri layout stopped existing.
`tagent-gui`'s own version is whatever is in `tagent-gui/Cargo.toml`
(currently `0.13.0`, unlinked from the workspace root's `0.13.0+002`) and is not synced
by anything.

## Other known gaps worth knowing about

- **`[Colors]` and `[Speech]` config sections** (`SourcePromptColor`, `TargetPromptColor`,
  `DictionaryPromptColor`, `EnableTextToSpeech`, `SpeechHotkey`, `EnableSpeechHotkey`)
  exist in `config.rs` and are used by CLI/interactive/keyboard-hook code, but have no
  equivalent in `tagent-gui`.
