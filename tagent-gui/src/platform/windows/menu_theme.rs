//! Dark or light native popup menus, following `tagent-gui`'s theme.
//!
//! The main window's provider menu (Slint's `ContextMenuArea`) is a native Win32 popup menu
//! (`TrackPopupMenu`, through `muda`), not a Slint-drawn one, so it ignores the Slint palette.
//! `muda` only themes menubars; a popup menu's colors come from the process-wide "preferred
//! app mode" in `uxtheme.dll`, which nothing sets, so the menu stayed light under the `Dark`
//! theme. [`apply`] sets that mode from the configured theme.
//!
//! `SetPreferredAppMode` (ordinal 135) and `FlushMenuThemes` (ordinal 136) are undocumented,
//! exported by ordinal only, and exist from Windows 10 1903 on; the same pair is what winit's
//! and Notepad++'s dark mode code use. When they are missing, [`apply`] does nothing.

use std::sync::atomic::{AtomicI32, Ordering};
use windows::core::{s, PCSTR};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::System::Registry::{RegGetValueA, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

/// `uxtheme`'s `PreferredAppMode` values that [`apply`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AppMode {
    ForceDark = 2,
    ForceLight = 3,
}

/// The mode for a `theme` value of `tagent-gui.json` (`"dark"`, `"light"` or `"auto"`);
/// `system_dark` is Windows' own "app mode" setting, used for `"auto"` (and anything else).
pub fn mode_for_theme(theme: &str, system_dark: bool) -> AppMode {
    match theme {
        "dark" => AppMode::ForceDark,
        "light" => AppMode::ForceLight,
        _ if system_dark => AppMode::ForceDark,
        _ => AppMode::ForceLight,
    }
}

/// Whether Windows' "Choose your app mode" is Dark (`AppsUseLightTheme` = 0).
fn system_prefers_dark() -> bool {
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value`/`size` describe a valid, writable DWORD buffer.
    let result = unsafe {
        RegGetValueA(
            HKEY_CURRENT_USER,
            s!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            s!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    result.is_ok() && value == 0
}

/// The mode last set, so the periodic `Auto` re-check (`apply_style` runs on a timer) only
/// touches `uxtheme` when the mode actually changes. 0 = never set.
static CURRENT_MODE: AtomicI32 = AtomicI32::new(0);

/// Makes native popup menus opened from now on dark or light, per `theme`.
pub fn apply(theme: &str) {
    let mode = mode_for_theme(theme, theme != "dark" && theme != "light" && system_prefers_dark());
    if CURRENT_MODE.swap(mode as i32, Ordering::Relaxed) == mode as i32 {
        return;
    }

    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();

    // SAFETY: uxtheme.dll is a system library that stays loaded for the process lifetime; the
    // ordinals are looked up (a missing one is skipped) and called with their known signatures.
    unsafe {
        let Ok(uxtheme) = LoadLibraryA(s!("uxtheme.dll")) else {
            return;
        };
        let Some(set_mode) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) else {
            return;
        };
        let set_mode: SetPreferredAppMode = std::mem::transmute(set_mode);
        set_mode(mode as i32);
        if let Some(flush) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
            let flush: FlushMenuThemes = std::mem::transmute(flush);
            flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_theme_wins_over_system() {
        assert_eq!(mode_for_theme("dark", false), AppMode::ForceDark);
        assert_eq!(mode_for_theme("light", true), AppMode::ForceLight);
    }

    #[test]
    fn auto_follows_system() {
        assert_eq!(mode_for_theme("auto", true), AppMode::ForceDark);
        assert_eq!(mode_for_theme("auto", false), AppMode::ForceLight);
    }
}
