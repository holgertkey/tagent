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
- **How translation works**: the `translate-requested` Slint callback spawns a plain OS
  thread with its own fresh `tokio::runtime::Runtime`, calls
  `providers::create_provider("google")` (**hardcoded** — see gap below), calls
  `provider.translate_text(...)`, then marshals the result back onto the Slint UI
  thread via `slint::invoke_from_event_loop`.
- **Scope**: a bare-bones translate-only prototype — no dictionary-entry display, no
  spell-check notices, no TTS button, no clipboard integration, no hotkeys, no history
  logging. `app.slint` hardcodes a 6-language list (Auto/English/Russian/Spanish/French/German),
  much smaller than the ~16 languages `config.rs` supports for CLI/interactive mode.

### Known gaps in `tagent-gui`

- **Provider is hardcoded to `"google"`**, bypassing `config.rs`'s `TranslateProvider`
  setting entirely. If a second provider is ever added (see `CLAUDE.md`'s "Adding a New
  Translation Provider" section), the GUI will not pick it up without a separate change
  in `tagent-gui/src/main.rs`.
- **No `tagent.conf` integration** beyond reusing `ConfigManager::language_to_code()` as
  a pure helper function — no config file is read or written by the GUI, so its language
  list and provider choice can drift from what CLI/interactive mode use.
- **No dictionary/spell-check/TTS UI** — it calls `TranslationProvider::translate_text`
  directly rather than going through `Translator`'s richer orchestration and formatting.

## `build.rs`: version sync and a stale Tauri leftover

The root `build.rs` runs on every `cargo build` and:

1. Reads the version from `Cargo.toml` (`CARGO_PKG_VERSION`, format `MAJOR.MINOR.PATCH[+BUILD]`).
2. `sync_version_in_docs()`: pattern-matches and rewrites version strings in
   `README.md`, `CLAUDE.md`, and `CHANGELOG.md` (skips the write if the value is already
   current, to avoid needless rebuilds/timestamp churn).
3. `sync_version_in_gui()`: strips the `+BUILD` suffix (GUI tooling doesn't support it)
   and syncs the base `MAJOR.MINOR.PATCH` into `tagent-gui/src-tauri/Cargo.toml`,
   `tagent-gui/ui/package.json`, and `tagent-gui/src-tauri/tauri.conf.json`.
4. On Windows only, when the `binary-resources` feature is active, embeds the app icon
   and version resource via `winres`.

**Step 3 targets a Tauri-based `tagent-gui` prototype that no longer exists.** The
current `tagent-gui` is Slint-based (see above) and has no `src-tauri/` directory or
`ui/package.json` at all — `git log -- tagent-gui` shows the Tauri prototype was
replaced by the Slint implementation, but this build-script code was never cleaned up.
It is harmless (`update_gui_cargo_version`/`update_version_in_file` both no-op via a
`Path::exists()` guard when the target file is missing), but it can mislead a reader
into thinking Tauri is involved in the current build. If you are touching `build.rs`
version-sync logic, this is safe to delete; if you are just reading it, ignore the
`sync_version_in_gui` function entirely.

## Other known gaps worth knowing about

- **`TranslationProvider::detect_language`** is implemented on `GoogleTranslateProvider`
  (`src/providers/google.rs`) but has no call site in `translator.rs` — the orchestrator
  does not currently use it. Confirm this is still true before removing or relying on it;
  it may be wired up in a future change without a doc update here.
- **`[Colors]` and `[Speech]` config sections** (`SourcePromptColor`, `TargetPromptColor`,
  `DictionaryPromptColor`, `EnableTextToSpeech`, `SpeechHotkey`, `EnableSpeechHotkey`)
  exist in `config.rs` and are used by CLI/interactive/keyboard-hook code, but have no
  equivalent in `tagent-gui`.
