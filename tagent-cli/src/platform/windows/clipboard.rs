use super::keycodes;
use crate::config::CopyMode;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetMessageExtraInfo, GetWindowThreadProcessId,
    SendMessageTimeoutW, GUITHREADINFO, SMTO_ABORTIFHUNG, WM_CANCELMODE, WM_COPY,
};

/// Windows clipboard access, backed by `clipboard-win` (get/set) and `SendInput`
/// (simulating Ctrl+C to copy the current text selection).
#[derive(Clone)]
pub struct ClipboardManager;

impl ClipboardManager {
    /// Create a new clipboard manager.
    pub fn new() -> Self {
        Self
    }

    /// Get text from clipboard
    pub fn get_text(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        match get_clipboard(formats::Unicode) {
            Ok(text) => Ok(text),
            Err(e) => Err(format!("Clipboard read error: {}", e).into()),
        }
    }

    /// Set text to clipboard
    pub fn set_text(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        match set_clipboard(formats::Unicode, text) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Clipboard write error: {}", e).into()),
        }
    }

    /// Automatically copy selected text (simulate Ctrl+C), then wait up to
    /// [`CLIPBOARD_CHANGE_TIMEOUT`] for the app to write the clipboard. Returns whether it
    /// did.
    ///
    /// `mode` comes from the hotkey that fired. [`CopyMode::Plain`] (no Alt in the
    /// hotkey) copies at once: Ctrl, Shift and Win put no window into menu mode, so the
    /// only thing to take care of is a Shift or Win still held, which would turn Ctrl+C
    /// into another shortcut (Ctrl+Shift+C opens Chrome's inspector). Those keys are
    /// released in the same `SendInput` call as the Ctrl+C, whose events Windows delivers
    /// in order with no other input in between, so nothing has to wait for them.
    /// [`CopyMode::Alt`] keeps the waits the Alt hotkeys need (see [`Self::copy_alt`]).
    pub fn copy_selected_text(&self, mode: CopyMode) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Taken before anything is sent: `WM_COPY` alone may already write the clipboard.
        let sequence_before = clipboard_win::seq_num();
        match mode {
            CopyMode::Alt => unsafe { Self::copy_alt() },
            CopyMode::Plain => unsafe { Self::copy_plain() },
        }

        Ok(wait_for_clipboard_change(
            clipboard_win::seq_num,
            sequence_before,
            CLIPBOARD_CHANGE_TIMEOUT,
        ))
    }

    /// The copy for hotkeys without Alt: `WM_COPY`, then a single `SendInput` with the
    /// releases of the held Shift/Win keys followed by Ctrl+C. No sleeps.
    unsafe fn copy_plain() {
        Self::send_wm_copy(GetForegroundWindow());

        let mut inputs: Vec<INPUT> = keys_to_release(|vk| keycodes::is_key_pressed(vk as i32))
            .into_iter()
            .map(|vk| Self::create_key_input(vk, true))
            .collect();
        inputs.extend(Self::ctrl_c_inputs());
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    /// The copy for hotkeys with Alt. Every wait in it is there for Alt's menu mode
    /// (`tagent-cli` 0.16.0+004 to +007).
    unsafe fn copy_alt() {
        // Capture the foreground window as the very first thing, before any sleep or
        // simulated input -- by the time those run, focus may already have moved (e.g.
        // this app's own terminal popping up per show_terminal_on_translate).
        let foreground = GetForegroundWindow();

        // Wait a bit before touching anything, to let the initial hotkey keystroke
        // settle.
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Wait for a physically-held Alt to actually be released, instead of injecting
        // a synthetic Alt-up below. Alt-based hotkeys (e.g. Alt+Q) are fully owned by
        // the low-level hook's swallow-and-replay mechanism (see
        // platform/windows/keyboard.rs's module docs), which blocks Alt's keydown from
        // ever reaching the foreground window in the first place -- so this loop
        // should normally see Alt already released by the time it runs. Kept as a
        // defensive fallback (e.g. non-combo hotkey types don't swallow Alt at all) and
        // bounded so a stuck key can't hang this call forever. Physical release is
        // necessary but not sufficient -- the foreground window's own message queue may
        // not have finished processing the matching keyup yet, which is what the
        // WM_CANCELMODE step below is for.
        let alt_release_deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(600);
        while keycodes::is_key_pressed(VK_MENU.0 as i32)
            && std::time::Instant::now() < alt_release_deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Settle delay so the foreground window's message queue has a chance to catch
        // up on the Alt keyup before we touch it again.
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Explicitly cancel any menu-tracking/modal loop the real Alt keydown may have
        // put the foreground window into (see CHANGELOG). WM_CANCELMODE is the
        // documented API for exactly this ("cancel modal (system) modes, such as ...
        // tracking the menu"). SendMessageTimeoutW instead of bare SendMessageW so a
        // busy/hung target window can't block this thread, which is also the thread
        // pumping this process's own hotkey message loop.
        if foreground.0 != 0 {
            let mut result: usize = 0;
            SendMessageTimeoutW(
                foreground,
                WM_CANCELMODE,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                150,
                Some(&mut result),
            );
        }

        // Release Shift/Win if still held (unlike Alt, these don't put the foreground
        // window into a menu-mode gesture on their own, so a synthetic up is safe here)
        // — this ensures Ctrl+C is recognized correctly when triggered by hotkeys like
        // Shift-based or Win-based combos.
        let inputs: Vec<INPUT> = vec![
            // Release Shift (both left and right)
            Self::create_key_input(VK_SHIFT.0, true),
            Self::create_key_input(VK_LSHIFT.0, true),
            Self::create_key_input(VK_RSHIFT.0, true),
            // Release Win (both left and right)
            Self::create_key_input(VK_LWIN.0, true),
            Self::create_key_input(VK_RWIN.0, true),
        ];

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);

        // Delay to ensure modifiers are processed
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Some apps (observed with Firefox) don't act on the simulated Ctrl+C below even
        // though SendInput reports it delivered. As a second mechanism -- in addition to,
        // not instead of, the SendInput below, since it's harmless where unsupported --
        // send WM_COPY directly to the actually-focused control, not the top-level
        // foreground window, which for a multi-control app usually isn't the thing that
        // owns the text selection. GetFocus() only works within your own thread, so the
        // focused control has to be read via GetGUIThreadInfo on the foreground window's
        // thread instead. (Doesn't help every app -- see the Notepad and Chrome
        // limitations in CHANGELOG: some apps' editing surface isn't backed by any HWND
        // a message can target at all, or otherwise doesn't act on either mechanism.)
        Self::send_wm_copy(foreground);

        SendInput(&Self::ctrl_c_inputs(), std::mem::size_of::<INPUT>() as i32);
    }

    /// Sends `WM_COPY` to the control that has the keyboard focus in `foreground`'s
    /// thread (see the comment at the call in [`Self::copy_alt`]).
    unsafe fn send_wm_copy(foreground: HWND) {
        let target_thread_id = GetWindowThreadProcessId(foreground, None);
        let mut gui_thread_info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let focus_target = if GetGUIThreadInfo(target_thread_id, &mut gui_thread_info).is_ok()
            && gui_thread_info.hwndFocus.0 != 0
        {
            gui_thread_info.hwndFocus
        } else {
            foreground
        };
        let mut wm_copy_result: usize = 0;
        SendMessageTimeoutW(
            focus_target,
            WM_COPY,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            150,
            Some(&mut wm_copy_result),
        );
    }

    /// Ctrl down, C down, C up, Ctrl up.
    unsafe fn ctrl_c_inputs() -> [INPUT; 4] {
        [
            Self::create_key_input(VK_CONTROL.0, false),
            Self::create_key_input(b'C' as u16, false),
            Self::create_key_input(b'C' as u16, true),
            Self::create_key_input(VK_CONTROL.0, true),
        ]
    }

    /// Helper function to create keyboard input structure for SendInput
    ///
    /// Sets `KEYEVENTF_SCANCODE` (scan code from `MapVirtualKeyW`, `wVk` left populated but
    /// ignored by the OS in this mode per the `KEYBDINPUT` docs) instead of a bare `wVk`
    /// event -- this routes the injected event through the same scan-code-to-virtual-key
    /// translation path real hardware keystrokes take, rather than a pre-resolved virtual
    /// key, which per a Microsoft Q&A-endorsed pattern is what some apps require to act on
    /// simulated input at all. `KEYEVENTF_EXTENDEDKEY` is added for keys in the AT-101
    /// extended set (here: Right Ctrl/Alt and the Windows keys) per the same requirement.
    unsafe fn create_key_input(vk_code: u16, is_keyup: bool) -> INPUT {
        let scan_code = MapVirtualKeyW(vk_code as u32, MAPVK_VK_TO_VSC) as u16;

        let is_extended = matches!(
            vk_code,
            v if v == VK_RCONTROL.0 || v == VK_RMENU.0 || v == VK_LWIN.0 || v == VK_RWIN.0
        );

        let mut flags = KEYEVENTF_SCANCODE;
        if is_keyup {
            flags |= KEYEVENTF_KEYUP;
        }
        if is_extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        let ki = KEYBDINPUT {
            wVk: VIRTUAL_KEY(vk_code),
            wScan: scan_code,
            dwFlags: flags,
            dwExtraInfo: GetMessageExtraInfo().0 as usize,
            ..Default::default()
        };

        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki },
        }
    }

    /// Get text from clipboard with automatic copying
    ///
    /// Some apps don't reliably pick up the simulated Ctrl+C/WM_COPY on the first attempt --
    /// the clipboard is left untouched, so this would otherwise silently return whatever was
    /// already there. `copy_selected_text` reports whether the clipboard changed (its
    /// sequence number, which any write bumps), and the copy is retried a bounded number of
    /// times until it does. Copying the same text the clipboard already held still counts
    /// as a change, so it costs no retries. If nothing was ever copied, the clipboard's
    /// current text is returned, as before.
    pub fn get_text_with_copy(
        &self,
        mode: CopyMode,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        const MAX_ATTEMPTS: u32 = 3;

        for _ in 0..MAX_ATTEMPTS {
            if self.copy_selected_text(mode)? {
                return self.read_text_after_change();
            }
        }

        self.get_text()
    }

    /// Reads the clipboard's text right after another app wrote it. That app may still
    /// hold the clipboard open for a moment (the sequence number changes when it empties
    /// the clipboard, before it sets the text and closes it), so a failed read is retried
    /// briefly.
    fn read_text_after_change(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        let deadline = Instant::now() + Duration::from_millis(100);
        loop {
            match self.get_text() {
                Ok(text) => return Ok(text),
                Err(err) if Instant::now() >= deadline => return Err(err),
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    /// The text currently selected in whatever app has it, for the global hotkeys: the
    /// same as [`Self::get_text_with_copy`] here (Linux reads PRIMARY on Wayland).
    pub fn get_selected_text(
        &self,
        mode: CopyMode,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.get_text_with_copy(mode)
    }
}

/// How long [`ClipboardManager::copy_selected_text`] waits for the app to put the copied
/// text on the clipboard before counting the attempt as failed. The apps measured
/// (Sublime Text, Obsidian, Chrome) had done it by the time the simulated Ctrl+C's
/// `SendInput` returned; this leaves room for slow ones. It used to be a fixed 100 ms
/// wait, after which the clipboard was read whether the app had written it or not.
const CLIPBOARD_CHANGE_TIMEOUT: Duration = Duration::from_millis(200);

/// Polls `sequence_number` (the clipboard's, in practice: `clipboard_win::seq_num`) until
/// it differs from `before` or `timeout` passes; returns whether it changed.
fn wait_for_clipboard_change(
    mut sequence_number: impl FnMut() -> Option<NonZeroU32>,
    before: Option<NonZeroU32>,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if sequence_number() != before {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The modifiers [`CopyMode::Plain`] releases before its Ctrl+C: those of left/right Shift
/// and Win that `is_pressed` reports held. Ctrl is left alone, since the Ctrl+C presses
/// and releases it anyway; a held Alt never reaches this path.
fn keys_to_release(is_pressed: impl Fn(u16) -> bool) -> Vec<u16> {
    [VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .map(|key| key.0)
        .filter(|&vk| is_pressed(vk))
        .collect()
}

#[cfg(test)]
mod keys_to_release_tests {
    use super::*;

    #[test]
    fn keys_to_release_is_empty_when_nothing_is_held() {
        assert!(keys_to_release(|_| false).is_empty());
    }

    #[test]
    fn keys_to_release_lists_only_the_held_shift_and_win_keys() {
        let held = [VK_RSHIFT.0, VK_LWIN.0];
        assert_eq!(
            keys_to_release(|vk| held.contains(&vk)),
            vec![VK_RSHIFT.0, VK_LWIN.0]
        );
    }

    #[test]
    fn keys_to_release_leaves_ctrl_and_alt_alone() {
        let held = [VK_CONTROL.0, VK_LCONTROL.0, VK_MENU.0, VK_LMENU.0];
        assert!(keys_to_release(|vk| held.contains(&vk)).is_empty());
    }
}

#[cfg(test)]
mod clipboard_change_tests {
    use super::*;
    use std::cell::Cell;

    fn seq(n: u32) -> Option<NonZeroU32> {
        NonZeroU32::new(n)
    }

    #[test]
    fn wait_for_clipboard_change_returns_once_the_sequence_number_moves() {
        let polls = Cell::new(0);
        let changed = wait_for_clipboard_change(
            || {
                polls.set(polls.get() + 1);
                if polls.get() < 3 {
                    seq(7)
                } else {
                    seq(8)
                }
            },
            seq(7),
            Duration::from_secs(5),
        );
        assert!(changed);
        assert_eq!(polls.get(), 3);
    }

    #[test]
    fn wait_for_clipboard_change_gives_up_after_the_timeout() {
        let started = Instant::now();
        let changed = wait_for_clipboard_change(|| seq(7), seq(7), Duration::from_millis(30));
        assert!(!changed);
        assert!(started.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn wait_for_clipboard_change_counts_a_first_write_as_a_change() {
        // No sequence number before (never written), one now.
        assert!(wait_for_clipboard_change(|| seq(1), None, Duration::ZERO));
    }
}
