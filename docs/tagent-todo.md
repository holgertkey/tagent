# TODO

## Hotkeys and selection copy: X11 checks and Step 4

Added 2026-10-08. Everything from `tagent-cli` 0.17.0+029 to +032 and `tagent-gui` 0.15.0+029
to +032 was built and tested on Windows first. Checked on Linux 2026-10-09 (GNOME 50.1,
Wayland): clippy and the workspace tests pass, and the hotkeys showed no problem. What is
left needs an X11 session (Xorg, not Wayland; the development machine has none): on Wayland
the hotkeys go through the portal and read PRIMARY, so the X11 copy path never runs there.

**1. AltGr on X11:**
- [ ] On Windows, AltGr+letter also fires an `Alt+letter` hotkey (modifiers are normalized,
      so right Alt counts as Alt; e.g. Polish AltGr+A = `ą` fires `Alt+A`). Check whether the
      X11 path does the same (`Key::AltGr` maps to `KEY_RALT`, normalized to Alt), then decide
      whether to fix it in both apps. Needs a layout where right Alt is AltGr (`pl`, `de`);
      with `us`/`ru` right Alt is plain Alt, and firing the hotkey is correct.

**2. Step 4 of the hotkey latency stage** (`docs/tagent-gui-dev-plan.md`, "Planned stage —
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

  ---
