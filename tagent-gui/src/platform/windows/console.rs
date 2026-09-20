//! Console output for a GUI-subsystem executable.
//!
//! `main.rs` builds `tagent-gui.exe` with `windows_subsystem = "windows"` so that starting it
//! from Explorer or a shortcut doesn't open a terminal window. The price is that the process
//! starts with no console and no standard handles, so `eprintln!` output (config warnings,
//! "Global hotkeys disabled", speech errors, ...) would go nowhere even when the exe is
//! launched from a terminal on purpose. [`attach_parent`] gets that output back.

use std::fs::OpenOptions;
use std::os::windows::io::IntoRawHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_HANDLE,
    STD_OUTPUT_HANDLE,
};

/// Attach to the console of the process that started us, if it has one, so `println!`/
/// `eprintln!` show up in the terminal the app was launched from.
///
/// A no-op when there is nothing to attach to (started from Explorer, a shortcut, autostart
/// or the tray) -- the standard handles then stay invalid, which is harmless because `std`
/// treats a write to an invalid handle as a silent success rather than a panic. It also
/// leaves a stream alone when the caller already redirected it (`tagent-gui.exe 2> log.txt`).
///
/// Note that cmd and PowerShell don't wait for a GUI-subsystem program, so the prompt comes
/// back immediately and the output interleaves with it; see `tagent-gui/README.md`.
pub fn attach_parent() {
    // SAFETY: plain Win32 calls with no pointer arguments.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() {
        return;
    }
    redirect_to_console(STD_OUTPUT_HANDLE);
    redirect_to_console(STD_ERROR_HANDLE);
}

/// Point `which` at the console screen buffer unless it already refers to something (a file
/// or pipe inherited from the parent).
fn redirect_to_console(which: STD_HANDLE) {
    // SAFETY: `GetStdHandle` only reads the process's standard handle slot.
    let existing = unsafe { GetStdHandle(which) };
    if matches!(existing, Ok(handle) if !handle.is_invalid() && handle.0 != 0) {
        return;
    }
    // "CONOUT$" is the active console screen buffer of the console we just attached to.
    let Ok(console) = OpenOptions::new().write(true).open("CONOUT$") else {
        return;
    };
    // The handle has to outlive this function: it becomes the process's standard handle,
    // so ownership is handed over to the OS slot instead of being closed on drop.
    let raw = HANDLE(console.into_raw_handle() as isize);
    // SAFETY: `raw` is a valid, open console handle.
    let _ = unsafe { SetStdHandle(which, raw) };
}
