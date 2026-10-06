//! One running copy per user: a second start asks the running copy to show its window
//! and exits.
//!
//! The running copy listens on a local socket (`interprocess`): a file socket in
//! `$XDG_RUNTIME_DIR` on Linux (the data dir without it, and on macOS), a named pipe with
//! the user name in it on Windows. A start first [`ask`]s that socket to `show`; whoever
//! answers is the running copy. Otherwise it [`claim`]s the name and [`serve`]s it for
//! the rest of the process. Per user on purpose: Linux's abstract socket namespace (what
//! `interprocess` would use for a namespaced name) is shared by every user of the machine
//! and has no permissions, and Windows pipe names are machine-wide.
//!
//! The protocol is one line each way: the client sends `show`, the server answers
//! `ok <pid>`, or `error unknown request` for anything else.

use interprocess::local_socket::{
    prelude::*, ConnectOptions, GenericFilePath, GenericNamespaced, Listener, ListenerOptions,
    Name, Stream,
};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The name part of the socket: the app id (see `desktop_entry::APP_ID`, Linux only).
const SOCKET_BASE: &str = "io.github.holgertkey.TagentGui";
/// How long either side waits for the other's line.
const TIMEOUT: Duration = Duration::from_secs(2);
/// The one request there is.
const SHOW: &str = "show";

/// Where the running copy listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketName {
    /// A socket file (Linux, macOS).
    File(PathBuf),
    /// A namespaced name: a named pipe (Windows).
    Namespaced(String),
}

impl std::fmt::Display for SocketName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketName::File(path) => write!(f, "{}", path.display()),
            SocketName::Namespaced(name) => write!(f, "{name}"),
        }
    }
}

impl SocketName {
    fn to_name(&self) -> io::Result<Name<'_>> {
        match self {
            SocketName::File(path) => path.as_path().to_fs_name::<GenericFilePath>(),
            SocketName::Namespaced(name) => name.as_str().to_ns_name::<GenericNamespaced>(),
        }
    }
}

/// The socket name from its inputs. Pure, for testing; [`socket_name`] passes the real
/// ones. `None` when there is nowhere per-user to put it.
pub fn socket_name_from(
    windows: bool,
    runtime_dir: Option<&Path>,
    data_dir: Option<&Path>,
    user: Option<&str>,
) -> Option<SocketName> {
    if windows {
        let user: String = user
            .filter(|user| !user.is_empty())?
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        return Some(SocketName::Namespaced(format!("{SOCKET_BASE}.{user}")));
    }
    let dir = match runtime_dir.filter(|dir| dir.is_absolute()) {
        Some(dir) => dir.to_path_buf(),
        None => data_dir?.join("tagent-gui"),
    };
    Some(SocketName::File(dir.join(format!("{SOCKET_BASE}.sock"))))
}

/// This user's socket name: `$XDG_RUNTIME_DIR` (Linux), else the data dir; on Windows a
/// pipe named after the user.
pub fn socket_name() -> Option<SocketName> {
    let runtime_dir = if cfg!(target_os = "linux") {
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
    } else {
        None
    };
    socket_name_from(
        cfg!(windows),
        runtime_dir.as_deref(),
        dirs::data_dir().as_deref(),
        std::env::var("USERNAME").ok().as_deref(),
    )
}

/// The answer to one request line.
fn reply_for(line: &str, pid: u32) -> String {
    if line.trim() == SHOW {
        format!("ok {pid}\n")
    } else {
        "error unknown request\n".to_string()
    }
}

/// The running copy's pid from its answer line.
fn parse_reply(line: &str) -> Result<u32, String> {
    let line = line.trim();
    line.strip_prefix("ok ")
        .and_then(|pid| pid.trim().parse().ok())
        .ok_or_else(|| {
            if line.is_empty() {
                "no answer".to_string()
            } else {
                format!("unexpected answer \"{line}\"")
            }
        })
}

/// What asking the socket found.
#[derive(Debug)]
pub enum Asked {
    /// A running copy showed its window; its pid.
    Shown(u32),
    /// Something accepted the connection but didn't answer properly in time.
    NoAnswer(String),
    /// Nobody listens (or the socket can't be reached); this start is the first.
    NotRunning,
}

/// Asks the copy listening on `socket` to show its window.
pub fn ask(socket: &SocketName) -> Asked {
    let stream = match socket.to_name().and_then(|name| {
        ConnectOptions::new()
            .name(name)
            .wait_mode(interprocess::ConnectWaitMode::Timeout(TIMEOUT))
            .connect_sync()
    }) {
        Ok(stream) => stream,
        Err(_) => return Asked::NotRunning,
    };
    match exchange(stream) {
        Ok(line) => match parse_reply(&line) {
            Ok(pid) => Asked::Shown(pid),
            Err(err) => Asked::NoAnswer(err),
        },
        Err(err) => Asked::NoAnswer(err.to_string()),
    }
}

/// Sends `show` and reads the answer line.
fn exchange(stream: Stream) -> io::Result<String> {
    stream.set_recv_timeout(Some(TIMEOUT))?;
    stream.set_send_timeout(Some(TIMEOUT))?;
    let mut reader = BufReader::new(stream);
    reader.get_mut().write_all(format!("{SHOW}\n").as_bytes())?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

/// What claiming the socket found.
pub enum Claim {
    /// This is the first copy: serve the listener ([`serve`]) for the whole process.
    First(Listener),
    /// Another copy runs (it was asked to show its window, and did): exit.
    Running(u32),
    /// Another copy holds the name but doesn't answer: exit with an error.
    Unresponsive(String),
    /// The socket can't be used at all; run anyway, without the guard.
    Unavailable(String),
}

/// Claims `socket` for this process. A socket file left by a crashed copy (nobody
/// answers on it) is removed and claimed, once. Never deletes a live copy's socket
/// (`try_overwrite` stays off).
pub fn claim(socket: &SocketName) -> Claim {
    if let SocketName::File(path) = socket {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    let mut reclaimed = false;
    loop {
        let created = socket
            .to_name()
            .and_then(|name| ListenerOptions::new().name(name).create_sync());
        let err = match created {
            Ok(listener) => return Claim::First(listener),
            Err(err) => err,
        };
        // Taken (Unix: AddrInUse; Windows: the first pipe instance exists, PermissionDenied):
        // see who holds it.
        if !matches!(
            err.kind(),
            io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied
        ) {
            return Claim::Unavailable(format!("cannot listen on {socket}: {err}"));
        }
        match ask(socket) {
            Asked::Shown(pid) => return Claim::Running(pid),
            Asked::NoAnswer(why) => return Claim::Unresponsive(why),
            Asked::NotRunning => {}
        }
        match socket {
            SocketName::File(path) if !reclaimed => {
                // A corpse socket from a crash: nobody listens. Remove it and retry once.
                reclaimed = true;
                if let Err(remove_err) = std::fs::remove_file(path) {
                    return Claim::Unavailable(format!(
                        "cannot remove the stale socket {socket}: {remove_err}"
                    ));
                }
            }
            _ => return Claim::Unavailable(format!("cannot listen on {socket}: {err}")),
        }
    }
}

/// Serves `listener` on a background thread for the rest of the process: answers every
/// request, and calls `on_show` for each `show` (before answering, so the asking start
/// knows the request was handed on). Interprocess removes the socket file when the
/// listener goes away with the process.
pub fn serve(listener: Listener, on_show: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let pid = std::process::id();
        for stream in listener.incoming().flatten() {
            let _ = handle(stream, pid, &on_show);
        }
    });
}

/// Answers one connection.
fn handle(stream: Stream, pid: u32, on_show: &impl Fn()) -> io::Result<()> {
    stream.set_recv_timeout(Some(TIMEOUT))?;
    stream.set_send_timeout(Some(TIMEOUT))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let reply = reply_for(&line, pid);
    if reply.starts_with("ok") {
        on_show();
    }
    reader.get_mut().write_all(reply.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Unix only: on Windows `/run/user/1000` isn't absolute (no drive letter), and the
    // Unix branch never runs there anyway.
    #[cfg(unix)]
    #[test]
    fn unix_uses_the_runtime_dir_then_the_data_dir() {
        assert_eq!(
            socket_name_from(false, Some(Path::new("/run/user/1000")), None, None),
            Some(SocketName::File(PathBuf::from(
                "/run/user/1000/io.github.holgertkey.TagentGui.sock"
            )))
        );
        assert_eq!(
            socket_name_from(false, None, Some(Path::new("/home/u/.local/share")), None),
            Some(SocketName::File(PathBuf::from(
                "/home/u/.local/share/tagent-gui/io.github.holgertkey.TagentGui.sock"
            )))
        );
        // A relative runtime dir is ignored, like an unset one.
        assert_eq!(
            socket_name_from(false, Some(Path::new("run")), Some(Path::new("/d")), None),
            Some(SocketName::File(PathBuf::from(
                "/d/tagent-gui/io.github.holgertkey.TagentGui.sock"
            )))
        );
        assert_eq!(socket_name_from(false, None, None, Some("u")), None);
    }

    #[test]
    fn windows_names_the_pipe_after_the_user() {
        assert_eq!(
            socket_name_from(true, None, None, Some("Holger K")),
            Some(SocketName::Namespaced(
                "io.github.holgertkey.TagentGui.Holger_K".to_string()
            ))
        );
        assert_eq!(socket_name_from(true, None, None, None), None);
        assert_eq!(socket_name_from(true, None, None, Some("")), None);
    }

    #[test]
    fn protocol_answers_show_and_rejects_the_rest() {
        assert_eq!(reply_for("show\n", 42), "ok 42\n");
        assert_eq!(reply_for("  show \r\n", 42), "ok 42\n");
        assert_eq!(reply_for("quit\n", 42), "error unknown request\n");
        assert_eq!(parse_reply("ok 42\n"), Ok(42));
        assert_eq!(
            parse_reply("error unknown request\n"),
            Err("unexpected answer \"error unknown request\"".to_string())
        );
        assert_eq!(parse_reply(""), Err("no answer".to_string()));
        assert!(parse_reply("ok x").is_err());
    }

    fn temp_socket(dir: &tempfile::TempDir) -> SocketName {
        SocketName::File(dir.path().join("t.sock"))
    }

    #[test]
    #[cfg(unix)]
    fn a_second_claim_finds_the_first_and_makes_it_show() {
        let dir = tempfile::tempdir().unwrap();
        let socket = temp_socket(&dir);
        let Claim::First(listener) = claim(&socket) else {
            panic!("the first claim must win");
        };
        let shown = Arc::new(AtomicUsize::new(0));
        let shown_in_server = shown.clone();
        serve(listener, move || {
            shown_in_server.fetch_add(1, Ordering::SeqCst);
        });
        match claim(&socket) {
            Claim::Running(pid) => assert_eq!(pid, std::process::id()),
            _ => panic!("the second claim must find the first"),
        }
        assert_eq!(shown.load(Ordering::SeqCst), 1);
        assert!(matches!(ask(&socket), Asked::Shown(_)));
        assert_eq!(shown.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[cfg(unix)]
    fn a_stale_socket_file_is_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = temp_socket(&dir);
        let SocketName::File(path) = &socket else {
            unreachable!()
        };
        // A socket file nobody listens on, as a crash leaves it.
        drop(std::os::unix::net::UnixListener::bind(path).unwrap());
        assert!(path.exists());
        assert!(matches!(ask(&socket), Asked::NotRunning));
        assert!(matches!(claim(&socket), Claim::First(_)));
    }

    #[test]
    #[cfg(unix)]
    fn a_listener_that_never_answers_is_unresponsive() {
        let dir = tempfile::tempdir().unwrap();
        let socket = temp_socket(&dir);
        let SocketName::File(path) = &socket else {
            unreachable!()
        };
        // Accepts (the kernel does, through the backlog) but never answers.
        let _silent = std::os::unix::net::UnixListener::bind(path).unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(claim(&socket), Claim::Unresponsive(_)));
        assert!(started.elapsed() < TIMEOUT * 3);
    }

    #[test]
    #[cfg(unix)]
    fn nothing_listening_means_not_running() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(ask(&temp_socket(&dir)), Asked::NotRunning));
    }
}
