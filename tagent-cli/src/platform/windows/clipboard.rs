use super::keycodes;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use std::error::Error;
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

    /// Automatically copy selected text (simulate Ctrl+C)
    pub fn copy_selected_text(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        unsafe {
            // Capture the foreground window as the very first thing, before any sleep or
            // simulated input -- by the time those run, focus may already have moved (e.g.
            // this app's own terminal popping up per ShowTerminalOnTranslate).
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

            // Wait for clipboard to update
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Ok(())
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
    /// already there before the hotkey was pressed. Detects that by snapshotting the
    /// clipboard beforehand and retrying `copy_selected_text` a bounded number of times until
    /// its content actually changes. Re-copying the exact same text the clipboard already
    /// held (e.g. pressing the hotkey twice on an unchanged selection) looks identical to a
    /// failed copy here and pays the same retries, but that's a rare, harmless case -- a few
    /// hundred ms of extra latency, not a wrong result.
    pub fn get_text_with_copy(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        const MAX_ATTEMPTS: u32 = 3;

        let before = self.get_text().unwrap_or_default();

        let mut last_result = String::new();
        for attempt in 1..=MAX_ATTEMPTS {
            self.copy_selected_text()?;
            last_result = self.get_text()?;

            if last_result != before || attempt == MAX_ATTEMPTS {
                break;
            }
        }

        Ok(last_result)
    }
}
