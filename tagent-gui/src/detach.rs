//! Detaching from the launching terminal (Linux/macOS).
//!
//! Started from a terminal, `tagent-gui` would otherwise hold it for as long as the app
//! runs (which, with the tray, is "until Quit"). [`detach_from_terminal`] re-launches the
//! executable as a new session leader with no controlling terminal, sends its
//! stdout/stderr to a log file, and lets the original process exit so the shell prompt
//! comes back at once. Windows needs none of this: `main.rs` builds a GUI-subsystem
//! executable, which shells don't wait for in the first place.
//!
//! The re-launched copy gets [`FOREGROUND_FLAG`] appended to its arguments, which is how it
//! knows not to detach again. Users pass the same flag (or `-f`) to keep the app attached
//! to the terminal, e.g. to watch its diagnostics while debugging.

use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Command-line flag that keeps the app attached to the terminal it was started from.
pub const FOREGROUND_FLAG: &str = "--foreground";
/// Short form of [`FOREGROUND_FLAG`].
const FOREGROUND_FLAG_SHORT: &str = "-f";

/// A log bigger than this is started over instead of appended to, so it can't grow
/// without bound across launches.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Whether `args` (without the program name) ask to stay in the foreground.
fn wants_foreground(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == FOREGROUND_FLAG || arg == FOREGROUND_FLAG_SHORT)
}

/// Where the detached process writes its stdout/stderr.
///
/// - **Linux**: `~/.local/share/tagent-gui/tagent-gui.log`
/// - **macOS**: `~/Library/Application Support/tagent-gui/tagent-gui.log`
pub fn log_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tagent-gui")
        .join("tagent-gui.log")
}

/// Opens the log at `path` for appending, creating its directory if needed; a log already
/// larger than `max_bytes` is truncated first.
fn open_log(path: &Path, max_bytes: u64) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let too_big = std::fs::metadata(path).is_ok_and(|meta| meta.len() > max_bytes);
    let mut options = OpenOptions::new();
    options.create(true);
    if too_big {
        options.write(true).truncate(true);
    } else {
        options.append(true);
    }
    options.open(path)
}

/// Re-launches `tagent-gui` detached from the terminal and exits this process, when it was
/// started from a terminal without [`FOREGROUND_FLAG`].
///
/// Returns normally (and the caller just carries on in this process) when there is nothing
/// to detach from — no standard stream is a terminal, e.g. a desktop launcher, autostart
/// or a systemd unit — when the flag is given, or when the re-launch fails (a warning is
/// printed and the app stays in the foreground rather than not starting at all).
///
/// Must run first thing in `main`, before any thread is spawned.
pub fn detach_from_terminal() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if wants_foreground(&args) {
        return;
    }
    let on_terminal = std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::io::stderr().is_terminal();
    if !on_terminal {
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("Warning: cannot locate the tagent-gui executable ({err}); staying in the foreground.");
            return;
        }
    };

    let log = log_path();
    let (stdout, stderr) = match open_log(&log, MAX_LOG_BYTES) {
        Ok(mut file) => {
            let _ = writeln!(
                file,
                "--- tagent-gui {} starting ---",
                env!("CARGO_PKG_VERSION")
            );
            match file.try_clone() {
                Ok(clone) => (Stdio::from(file), Stdio::from(clone)),
                Err(_) => (Stdio::null(), Stdio::null()),
            }
        }
        Err(err) => {
            eprintln!(
                "Warning: cannot open {} ({err}); diagnostics will be discarded.",
                log.display()
            );
            (Stdio::null(), Stdio::null())
        }
    };

    let mut command = Command::new(&exe);
    command
        .args(&args)
        .arg(FOREGROUND_FLAG)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    // SAFETY: runs in the forked child before `exec`; `setsid` is async-signal-safe. A new
    // session has no controlling terminal, so closing the terminal (SIGHUP) or Ctrl+C in it
    // no longer reaches the app.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    match command.spawn() {
        Ok(child) => {
            println!(
                "tagent-gui is running in the background (pid {}); log: {}",
                child.id(),
                log.display()
            );
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("Warning: failed to start tagent-gui in the background ({err}); staying in the foreground.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn foreground_flag_long_and_short_forms_are_recognized() {
        assert!(wants_foreground(&args(&["--foreground"])));
        assert!(wants_foreground(&args(&["-f"])));
        assert!(wants_foreground(&args(&["--other", "--foreground"])));
    }

    #[test]
    fn no_foreground_flag_means_detach() {
        assert!(!wants_foreground(&args(&[])));
        assert!(!wants_foreground(&args(&["--foregroundx", "-F"])));
    }

    #[test]
    fn log_path_lives_in_its_own_tagent_gui_directory() {
        let path = log_path();
        assert_eq!(path.file_name().unwrap(), "tagent-gui.log");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "tagent-gui");
    }

    #[test]
    fn open_log_creates_missing_directory_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("tagent-gui.log");
        writeln!(open_log(&path, MAX_LOG_BYTES).unwrap(), "first").unwrap();
        writeln!(open_log(&path, MAX_LOG_BYTES).unwrap(), "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    }

    #[test]
    fn open_log_starts_over_when_log_is_too_big() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tagent-gui.log");
        std::fs::write(&path, "0123456789").unwrap();
        writeln!(open_log(&path, 5).unwrap(), "fresh").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh\n");
    }
}
