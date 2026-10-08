# TODO

## tagent-gui: native menus ignore the app theme (Slint upstream)

Added 2026-10-08. Come back to this when working on Linux.

**Problem.** On Windows, Slint 1.17 (winit backend) shows `ContextMenuArea` (the provider
`google` button menu) as a native Win32 popup through `muda`. Such a menu ignores
`Palette.color-scheme` and follows the system light/dark setting, so it stayed light under the
app's Dark theme while Windows itself was in light mode.

**Our workaround** (tagent-gui 0.15.0+025): `tagent-gui/src/platform/windows/menu_theme.rs` calls the
undocumented uxtheme ordinals 135 (`SetPreferredAppMode`) and 136 (`FlushMenuThemes`) from
`apply_style`. Still needed with Slint 1.17.1 / muda 0.19.3.

**Upstream status:**
- [slint#9646](https://github.com/slint-ui/slint/issues/9646): ContextMenuArea stays light on Windows 11
  (exactly our case). Closed as a duplicate of #8092. Maintainer: native popup menus "should follow
  the platform style and not the slint style".
- [slint#8092](https://github.com/slint-ui/slint/issues/8092): MenuBar ignores color-scheme. Closed as
  fixed by PR #10034.
- [slint PR #10034](https://github.com/slint-ui/slint/pull/10034): only the menubar, and only follows
  the *system* theme, not `Palette.color-scheme`. Popup menus are untouched.
- [slint#9771](https://github.com/slint-ui/slint/issues/9771) (open, related): menu icons are not
  colorized by theme.

So an app-forced theme that differs from the system theme is still unsupported upstream.

**To do on Linux:**
- [ ] Check how `ContextMenuArea` behaves on Linux: is it drawn by Slint (follows the palette) or
      native? Does the provider menu match the Dark/Light theme of the app when the desktop theme differs?
- [ ] Check the tray menu theme on Linux.
- [ ] Decide whether to report upstream: a comment in #9646 with our uxtheme workaround, or a new
      issue saying #10034 did not cover app theme != system theme for `ContextMenuArea`, with a
      minimal repro.

## Hotkeys and selection copy: Linux checks and Step 4

Added 2026-10-08. Everything from `tagent-cli` 0.17.0+029 to +032 and `tagent-gui` 0.15.0+029
to +032 was built and tested on Windows only; Linux was never compiled for it, and CI runs
only on `main`.

**1. Build and tests (before anything else, and before pushing to `main`):**
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace`
- What could break: `get_selected_text(_mode: CopyMode)` in `platform/linux/clipboard.rs`
  (both apps); `CopyMode` passed through `trigger_translation`/`trigger_speech`/`speak_clipboard`
  in `tagent-cli/src/platform/linux/keyboard.rs` (X11 and portal paths); the new hotkey tests in
  both `config.rs` (`uses_alt`, `uses_win`, `validate_*`), which run with the Linux key codes.

**2. Hotkey rules on Linux (+030, +031):**
- [ ] A refused hotkey (`Win+T`, `Alt+Tab`, `Ctrl+C`, `Alt+Alt`, `A+Q`) in `tagent-cli.toml`
      is turned off at start with the reason, and the other hotkey keeps working.
- [ ] `tagent-gui` Settings shows the error for a refused hotkey, typed and recorded.
- [ ] The defaults `Alt+A`/`Alt+S` still work on X11 and through the Wayland portal.
- [ ] AltGr: on Windows, AltGr+letter also fires an `Alt+letter` hotkey (modifiers are
      normalized, so right Alt counts as Alt; e.g. Polish AltGr+A = `ą` fires `Alt+A`).
      Check whether Linux does the same (`Key::AltGr` maps to `KEY_RALT`, normalized to Alt),
      then decide whether to fix it in both apps.

**3. Step 4 of the hotkey latency stage** (`docs/tagent-gui-dev-plan.md`, "Planned stage —
Hotkey latency (selection copy)", Step 4; X11/XWayland only, both apps):
- [ ] Measure first: temporary `eprintln!("[clip] ...")` timing on each wait in
      `copy_selected_text`/`send_copy_keystroke` (`platform/linux/clipboard.rs`), never
      committed. Today's waits: 100 ms initial, the trigger-key release wait (bounded,
      1.5 s), 50 ms after the modifier releases, 100 ms after Ctrl+C.
- [ ] Drop the initial 100 ms (the trigger-key wait covers it).
- [ ] Drop the 50 ms after the modifier releases (`XSync` flushes; XTest events arrive in order).
- [ ] Replace the final 100 ms with a success signal: XFixes `XFixesSelectSelectionInput` on
      CLIPBOARD (every new ownership is reported, even by the same app; needs the `xfixes`
      feature of the `x11` crate), bounded like Windows' 200 ms. Polling `arboard` for new
      text is not enough: copying the same text again looks like no change.
- [ ] Check whether `rdev`'s listener or the `XGrabKey` thread has a poll interval that
      delays keys, like the Windows hook's 10 ms loop did before +028 (the `tagent-cli` X11
      loop has a 100 ms `tokio::select!` tick for `should_exit`, which shouldn't matter).
- Keep: the trigger-key release wait. While the grabbed hotkey key is down, X11 sends every
  key event, the fake Ctrl+C included, to tagent instead of the app.
- No Alt path: X11 has no menu mode, so `CopyMode` stays ignored and Step 3's Alt checks
  don't apply.
- Checks per change: the right text on the first attempt, no stuck Ctrl/Shift/Alt
  afterwards, with `Alt+A` and with `Ctrl+Shift+T` or `F9`, in GNOME Text Editor,
  Firefox or Chrome, Obsidian and Sublime Text; a Russian layout once.
- Afterwards: changelog entries in both apps, Step 4 marked done in the dev plan (with the
  measurements), and the Linux part of the "Windows specifics" copy section in
  `docs/ARCHITECTURE.md` (or a Linux counterpart).
- Out of scope: Wayland (reads PRIMARY, simulates no keys); terminals, which copy with
  Ctrl+Shift+C, not the Ctrl+C the copy sends.
