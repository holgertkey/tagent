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

**Concept (decided 2026-08-15, see [`.debug/tagent-gui development
plan.md`](../.debug/tagent-gui%20development%20plan.md)): `tagent-gui` is a fully
independent application from `tagent-cli`** — own interface, own configuration (its
own `tagent-gui.json`; see "Reading `translate_provider`" below), own feature set (no
obligation to reach parity with `tagent-cli`), and own versioning/changelog
(`tagent-gui/CHANGELOG.md`, independent of the root `CHANGELOG.md` which belongs to
`tagent-cli`, and using the same `MAJOR.MINOR.PATCH+BUILD` format/increment rules as
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
- **Settings dialog** (`SettingsDialog` in `app.slint`, gear `⚙` button at the right
  edge of `AppWindow`'s top row): a second `export component ... inherits Dialog`
  in the *same* `app.slint` file — supported since Slint 1.7, no second `build.rs`
  entry needed. Uses `std-widgets`' built-in `Dialog`/`StandardButton { kind: ok /
  cancel; }` rather than a hand-rolled `Window`, so button layout/ordering follows
  the platform convention for free — sets `preferred-width`/`preferred-height`
  explicitly (420×300); without it the dialog fell back to Slint's default window
  size, an early Stage 3 bug fixed the same day it shipped. Its content sits inside
  a `TabWidget` (`General`, holding everything that exists today; `Hotkeys & Tray`,
  a placeholder `Text` pending the fields Stage 8 adds) rather than a flat panel —
  laid out ahead of need since Settings is expected to grow more categories over
  future stages, so a new category is a new `Tab { }` block, not a redesign.
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
  accepted, since only `"google"` is a supported value today.
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
  aren't expected to flash. Accepted as-is (see the development plan's theme
  section for the full reasoning) rather than worked around with extra persisted
  state for a cosmetic, single-frame issue.
- **Clipboard** (`tagent-gui/src/platform/`, Stage 4, shipped 2026-09-13): a
  `ClipboardManager` per OS (`platform/{linux,windows,macos}/clipboard.rs`), behind
  `#[cfg(target_os = "...")]` re-exports in `platform/mod.rs` — the same
  directory-per-OS, no-`trait` shape as `tagent-cli/src/platform/mod.rs`, chosen
  deliberately so Stage 5's hotkey code can drop `keyboard.rs` into the same
  per-OS directories later without a restructure. `get_text`/`set_text`/
  `copy_selected_text`/`get_text_with_copy` are ported near-verbatim from
  `tagent-cli`'s own `ClipboardManager` (Linux: `arboard` + a process-lifetime
  `static CLIPBOARD` + `xdotool`-simulated Ctrl+C; Windows: `clipboard-win` +
  `SendInput`/`WM_CANCELMODE`/`WM_COPY`-to-focused-control fallback; macOS: a
  stub, every method `Err`, matching `tagent-cli`'s own macOS posture). Windows'
  version inlines a single `GetAsyncKeyState` call instead of porting
  `tagent-cli`'s whole `keycodes` module (that module's real job — hotkey name/VK
  parsing — belongs to Stage 5), and its comments describe the Alt-release-wait/
  `WM_CANCELMODE` steps as groundwork for Stage 5's future hook rather than citing
  `tagent-cli`'s hook module, which doesn't exist here. `set_text` is currently
  unused (`#[allow(dead_code)]`, kept for API parity and future use, e.g. copying
  a translation result back to the clipboard) — this stage only wires up reading.
  A "📋" button in the input row (`copy-requested` callback) calls
  `ClipboardManager::get_text_with_copy()` on a background `std::thread::spawn`
  (both platforms' `copy_selected_text` block for real wall-clock time — up to
  ~250ms on Linux, ~900ms on Windows — so this must not run on Slint's own event
  loop thread), then applies the result via `slint::invoke_from_event_loop`,
  mirroring `on_translate_requested`'s existing thread-hop pattern exactly. On
  success it replaces `input-text`; on failure (e.g. the Wayland guard, or
  `xdotool` missing) it pushes a `TranscriptEntry { phrase: "[Clipboard]", ... }`
  through the existing `push_transcript_entry` error-display convention, rather
  than inventing a new one. **Known limitation, inherent to a button-triggered (as
  opposed to global-hotkey-triggered) design**: clicking "📋" necessarily gives
  `tagent-gui`'s own window input focus first, so the simulated Ctrl+C inside
  `copy_selected_text` targets `tagent-gui` itself, not whatever window/selection
  was active immediately before the click — unlike `tagent-cli`'s global hotkey,
  which fires without stealing focus from the source app. In practice the button
  behaves as "pull whatever the system clipboard already holds into the input
  box" (useful on its own, e.g. after a manual Ctrl+C elsewhere) rather than "grab
  the current selection with no prior Ctrl+C needed" — that stronger capability is
  only meaningful once Stage 5's global hotkey exists, since a hotkey doesn't
  require a focus change to fire. Not a bug to fix in this stage; documented here
  so it isn't mistaken for one later.
- **Scope**: a bare-bones translate-only prototype — no dictionary-entry display, no
  spell-check notices, no TTS button, no global hotkeys, no history
  logging. `app.slint` hardcodes a 6-language list (Auto/English/Russian/Spanish/French/German),
  much smaller than the ~16 languages `config.rs` supports for CLI/interactive mode.

### Known gaps in `tagent-gui`

- **`tagent-gui.json` only has one settable field** (`translate_provider`, via the
  Settings dialog above) — language list, hotkeys, history logging, colors, TTS
  settings, dictionary/spell-check toggles aren't configurable at all yet (either
  hardcoded, like the 6-language list, or simply unsupported, like history/
  hotkeys). `tagent-cli.conf` is not read at all any more (no migration path — see
  the "own configuration" concept in the development plan).
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
   (`assets/icons/taa_256.ico`, inside the `tagent-cli/` package itself) and version resource
   via `winres`.

There is no GUI-specific version sync step: an earlier Tauri-based `tagent-gui`
prototype had one (writing into `tagent-gui/src-tauri/Cargo.toml` etc.), but it was
removed once `tagent-gui` moved to Slint and that Tauri layout stopped existing.
`tagent-gui`'s own version is whatever is in `tagent-gui/Cargo.toml`
(currently `0.13.0`, unlinked from `tagent-cli`'s `0.13.0+003`) and is not synced by
anything. As of the 2026-08-15 independence decision (see "Concept" at the top of the
`tagent-gui` section above), this is deliberate rather than merely unaddressed:
`tagent-gui` versions on its own plain-semver track — no `+BUILD` suffix, since that
convention is specific to `tagent-cli`'s dev-iteration tracking — and logs its history
in its own [`tagent-gui/CHANGELOG.md`](../tagent-gui/CHANGELOG.md), separate from the
root `CHANGELOG.md` that `tagent-cli/build.rs` syncs into (which is `tagent-cli`'s
changelog, not the workspace's). The `tagent` library crate's version (`1.0.0`) is
likewise standalone, plain semver with no `+BUILD` suffix. `1.0.0` was chosen
deliberately: it needs to sort above whatever version the old single-crate `tagent`
application last published to crates.io, so that `cargo install tagent` / `cargo add
tagent` resolve to the library once it is actually published (not done as part of this
restructuring), not the old app.

## Other known gaps worth knowing about

- **`[Colors]` and `[Speech]` config sections** (`SourcePromptColor`, `TargetPromptColor`,
  `DictionaryPromptColor`, `EnableTextToSpeech`, `SpeechHotkey`, `EnableSpeechHotkey`)
  exist in `config.rs` and are used by CLI/interactive/keyboard-hook code, but have no
  equivalent in `tagent-gui`.
