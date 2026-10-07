use super::keycodes::is_key_pressed;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{LPARAM, WPARAM};
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
    pub fn copy_selected_text(&self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Taken before anything is sent: `WM_COPY` alone may already write the clipboard.
        let sequence_before = clipboard_win::seq_num();
        unsafe {
            // Capture the foreground window as the very first thing, before any sleep or
            // simulated input -- by the time those run, focus may already have moved.
            let foreground = GetForegroundWindow();

            // Wait a bit before touching anything, to let the triggering click/keystroke
            // settle.
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Wait for a physically-held Alt to actually be released, instead of injecting
            // a synthetic Alt-up below. When this call is triggered by the "📋" button
            // (a click, not a keystroke), a held Alt shouldn't normally happen; when
            // triggered by the Alt+Q-style global hotkey (see `keyboard.rs`), that hook's
            // own swallow-and-replay mechanism already prevents the real Alt keydown from
            // reaching the foreground app in the first place, so this loop is a defensive
            // fallback either way, not the primary protection. Physical release is
            // necessary but not sufficient -- the foreground window's own message queue
            // may not have finished processing the matching keyup yet, which is what the
            // WM_CANCELMODE step below is for.
            let alt_release_deadline =
                std::time::Instant::now() + std::time::Duration::from_millis(600);
            while is_key_pressed(VK_MENU.0 as i32)
                && std::time::Instant::now() < alt_release_deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            // Settle delay so the foreground window's message queue has a chance to catch
            // up on the Alt keyup before we touch it again.
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Explicitly cancel any menu-tracking/modal loop a real Alt keydown may have
            // put the foreground window into. WM_CANCELMODE is the documented API for
            // exactly this ("cancel modal (system) modes, such as ... tracking the
            // menu"). SendMessageTimeoutW instead of bare SendMessageW so a busy/hung
            // target window can't block this thread.
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
            // -- this ensures Ctrl+C is recognized correctly if triggered while a
            // Shift-based or Win-based key combo is still held down.
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

            // Some apps (observed with Firefox, in tagent-cli) don't act on the simulated
            // Ctrl+C below even though SendInput reports it delivered. As a second
            // mechanism -- in addition to, not instead of, the SendInput below, since
            // it's harmless where unsupported -- send WM_COPY directly to the actually-
            // focused control, not the top-level foreground window, which for a
            // multi-control app usually isn't the thing that owns the text selection.
            // GetFocus() only works within your own thread, so the focused control has
            // to be read via GetGUIThreadInfo on the foreground window's thread instead.
            // (Doesn't help every app -- some apps' editing surface isn't backed by any
            // HWND a message can target at all, or otherwise doesn't act on either
            // mechanism.)
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

            // Simulate Ctrl+C using SendInput
            let ctrl_c_inputs: Vec<INPUT> = vec![
                // Ctrl down
                Self::create_key_input(VK_CONTROL.0, false),
                // C down
                Self::create_key_input(b'C' as u16, false),
                // C up
                Self::create_key_input(b'C' as u16, true),
                // Ctrl up
                Self::create_key_input(VK_CONTROL.0, true),
            ];

            SendInput(&ctrl_c_inputs, std::mem::size_of::<INPUT>() as i32);
        }

        Ok(wait_for_clipboard_change(
            clipboard_win::seq_num,
            sequence_before,
            CLIPBOARD_CHANGE_TIMEOUT,
        ))
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
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        const MAX_ATTEMPTS: u32 = 3;

        for _ in 0..MAX_ATTEMPTS {
            if self.copy_selected_text()? {
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
    pub fn get_selected_text(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.get_text_with_copy()
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
