//! Linux desktop integration: a `.desktop` file and an app icon in the user's data dir.
//!
//! The GlobalShortcuts portal, which delivers the hotkeys on a Wayland session (see
//! `platform::linux::portal`), accepts only an app whose reverse-DNS id ([`APP_ID`]) is
//! backed by an installed `.desktop` file. The entry also puts "Tagent CLI" in the
//! application menu (it opens in a terminal, `Terminal=true`) and in GNOME Settings >
//! Apps, where the bound keys are changed. [`install`] writes:
//!
//! - `~/.local/share/applications/io.github.holgertkey.TagentCli.desktop`
//! - `~/.local/share/icons/hicolor/512x512/apps/io.github.holgertkey.TagentCli.png`
//!
//! (`$XDG_DATA_HOME` instead of `~/.local/share` when set.) Installing is explicit,
//! through [`INSTALL_FLAG`], never automatic on startup: a `cargo run` from `target/debug`
//! would otherwise point the launcher at a debug build. `Exec=` is the running executable's
//! own path, so installing again after moving the binary fixes the entry.
//!
//! The release archive (see `release.yml`) ships the same entry as a static file,
//! `assets/linux/io.github.holgertkey.TagentCli.desktop`, with `Exec=tagent-cli` (found on
//! `PATH`); a test keeps it identical to what [`install`] writes.

use std::io;
use std::path::{Path, PathBuf};

/// The app id: the `.desktop` file name and icon name, and what the portal knows the app
/// by. Reverse-DNS because `xdg-desktop-portal-gnome` rejects a bind request from an app
/// whose id isn't, or has no installed `.desktop` file of that name. Duplicated from
/// `tagent-gui`'s `desktop_entry` (platform code isn't shared), with its own id.
pub const APP_ID: &str = "io.github.holgertkey.TagentCli";
/// Command-line flag that installs the `.desktop` file and icon, then exits.
pub const INSTALL_FLAG: &str = "--install-desktop";
/// Command-line flag that removes what [`INSTALL_FLAG`] installed, then exits.
pub const UNINSTALL_FLAG: &str = "--uninstall-desktop";

/// The icon, embedded so the installed binary needs no files next to it.
const ICON_PNG: &[u8] = include_bytes!("../assets/icons/tagent-cli.png");
/// Size of [`ICON_PNG`]; names its `hicolor` directory.
const ICON_SIZE: &str = "512x512";

/// What a desktop-integration flag on the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// [`INSTALL_FLAG`].
    Install,
    /// [`UNINSTALL_FLAG`].
    Uninstall,
}

/// The desktop-integration command in `args` (without the program name), if any. Only
/// the first argument counts, as for `--update-config`: text to translate that contains
/// the flag is still text.
pub fn command_from_args(args: &[String]) -> Option<Command> {
    match args.first()?.as_str() {
        INSTALL_FLAG => Some(Command::Install),
        UNINSTALL_FLAG => Some(Command::Uninstall),
        _ => None,
    }
}

/// Runs `command` against the user's data dir, prints what happened, and returns the
/// process exit code.
pub fn run(command: Command) -> i32 {
    let Some(data_dir) = dirs::data_dir() else {
        eprintln!(
            "Error: cannot determine the user data directory ($XDG_DATA_HOME or ~/.local/share)."
        );
        return 1;
    };
    let result = match command {
        Command::Install => std::env::current_exe().and_then(|exe| {
            let paths = install(&data_dir, &exe)?;
            println!("Installed {}", paths.desktop_file.display());
            println!("Installed {}", paths.icon.display());
            println!("Launcher entry points to {}", exe.display());
            Ok(())
        }),
        Command::Uninstall => uninstall(&data_dir).map(|paths| {
            if paths.is_empty() {
                println!("Nothing to remove: the desktop entry is not installed.");
            }
            for path in paths {
                println!("Removed {}", path.display());
            }
        }),
    };
    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Error: {err}");
            1
        }
    }
}

/// Where [`install`] puts its files, under a data dir.
#[derive(Debug, PartialEq, Eq)]
pub struct Paths {
    /// The `.desktop` file.
    pub desktop_file: PathBuf,
    /// The icon PNG.
    pub icon: PathBuf,
}

impl Paths {
    /// The paths under `data_dir` (normally `~/.local/share`).
    pub fn under(data_dir: &Path) -> Self {
        Self {
            desktop_file: data_dir
                .join("applications")
                .join(format!("{APP_ID}.desktop")),
            icon: data_dir
                .join("icons")
                .join("hicolor")
                .join(ICON_SIZE)
                .join("apps")
                .join(format!("{APP_ID}.png")),
        }
    }
}

/// Writes the `.desktop` file (launching `exe`) and the icon under `data_dir`, creating
/// directories as needed. A file that already has the right contents is left untouched.
pub fn install(data_dir: &Path, exe: &Path) -> io::Result<Paths> {
    let paths = Paths::under(data_dir);
    write_if_changed(&paths.icon, ICON_PNG)?;
    write_if_changed(&paths.desktop_file, desktop_file_contents(exe).as_bytes())?;
    Ok(paths)
}

/// Removes the files [`install`] writes under `data_dir`; returns the ones that existed.
pub fn uninstall(data_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let paths = Paths::under(data_dir);
    let mut removed = Vec::new();
    for path in [paths.desktop_file, paths.icon] {
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(removed)
}

fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<()> {
    if std::fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, contents)
}

/// The `.desktop` file for launching `exe`.
fn desktop_file_contents(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Tagent CLI\n\
         GenericName=Translator\n\
         Comment={comment}\n\
         Exec={exec}\n\
         Icon={APP_ID}\n\
         Terminal=true\n\
         Categories=Utility;\n\
         Keywords=translate;translation;dictionary;terminal;\n",
        comment = env!("CARGO_PKG_DESCRIPTION"),
        exec = exec_value(&exe.to_string_lossy()),
    )
}

/// `path` as the program of an `Exec=` key: quoted when it holds a reserved character,
/// with `"`, `` ` ``, `$` and `\` escaped inside the quotes, then `\` doubled again for
/// the string-value level; `%` (field codes) is always doubled.
fn exec_value(path: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    let path = path.replace('%', "%%");
    if !path.contains(RESERVED) {
        return path;
    }
    let mut quoted = String::from("\"");
    for ch in path.chars() {
        match ch {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(ch);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn command_flags_are_recognized() {
        assert_eq!(
            command_from_args(&args(&["--install-desktop"])),
            Some(Command::Install)
        );
        assert_eq!(
            command_from_args(&args(&["--uninstall-desktop"])),
            Some(Command::Uninstall)
        );
        assert_eq!(command_from_args(&args(&["--config"])), None);
        assert_eq!(command_from_args(&args(&[])), None);
        // Text to translate that happens to contain the flag is still text.
        assert_eq!(
            command_from_args(&args(&["say", "--install-desktop"])),
            None
        );
    }

    #[test]
    fn desktop_file_runs_in_a_terminal_with_the_icon() {
        let contents = desktop_file_contents(Path::new("/opt/tagent/tagent-cli"));
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines[0], "[Desktop Entry]");
        assert!(lines.contains(&"Exec=/opt/tagent/tagent-cli"));
        assert!(lines.contains(&"Icon=io.github.holgertkey.TagentCli"));
        assert!(lines.contains(&"Type=Application"));
        assert!(lines.contains(&"Terminal=true"));
        assert!(lines.contains(&"Name=Tagent CLI"));
    }

    #[test]
    fn packaged_desktop_file_matches_the_generated_one() {
        assert_eq!(
            include_str!("../assets/linux/io.github.holgertkey.TagentCli.desktop"),
            desktop_file_contents(Path::new("tagent-cli")),
            "assets/linux/io.github.holgertkey.TagentCli.desktop is out of date: regenerate it from \
             desktop_file_contents(\"tagent-cli\")"
        );
    }

    #[test]
    fn exec_value_leaves_plain_paths_alone() {
        assert_eq!(
            exec_value("/home/u/.cargo/bin/tagent-cli"),
            "/home/u/.cargo/bin/tagent-cli"
        );
    }

    #[test]
    fn exec_value_quotes_and_escapes_reserved_characters() {
        assert_eq!(exec_value("/my apps/tagent-cli"), "\"/my apps/tagent-cli\"");
        // `$` -> `\$` inside quotes -> `\\$` after string-value escaping.
        assert_eq!(exec_value("/a$b/t"), "\"/a\\\\$b/t\"");
        // `\` -> `\\` inside quotes -> `\\\\` after string-value escaping.
        assert_eq!(exec_value("/a\\b/t"), "\"/a\\\\\\\\b/t\"");
        assert_eq!(exec_value("/100%/t"), "/100%%/t");
    }

    #[test]
    fn install_writes_both_files_under_the_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let paths = install(dir.path(), Path::new("/usr/bin/tagent-cli")).unwrap();
        assert_eq!(
            paths.desktop_file,
            dir.path()
                .join("applications/io.github.holgertkey.TagentCli.desktop")
        );
        assert_eq!(
            paths.icon,
            dir.path()
                .join("icons/hicolor/512x512/apps/io.github.holgertkey.TagentCli.png")
        );
        assert_eq!(std::fs::read(&paths.icon).unwrap(), ICON_PNG);
        let desktop = std::fs::read_to_string(&paths.desktop_file).unwrap();
        assert!(desktop.contains("Exec=/usr/bin/tagent-cli\n"));
    }

    #[test]
    fn install_again_updates_exec_and_keeps_unchanged_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = install(dir.path(), Path::new("/old/tagent-cli")).unwrap();
        let icon_mtime = std::fs::metadata(&paths.icon).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        install(dir.path(), Path::new("/new/tagent-cli")).unwrap();
        let desktop = std::fs::read_to_string(&paths.desktop_file).unwrap();
        assert!(desktop.contains("Exec=/new/tagent-cli\n"));
        assert!(!desktop.contains("/old/"));
        assert_eq!(
            std::fs::metadata(&paths.icon).unwrap().modified().unwrap(),
            icon_mtime
        );
    }

    #[test]
    fn uninstall_removes_installed_files_and_tolerates_missing_ones() {
        let dir = tempfile::tempdir().unwrap();
        let paths = install(dir.path(), Path::new("/usr/bin/tagent-cli")).unwrap();
        let removed = uninstall(dir.path()).unwrap();
        assert_eq!(
            removed,
            vec![paths.desktop_file.clone(), paths.icon.clone()]
        );
        assert!(!paths.desktop_file.exists());
        assert!(!paths.icon.exists());
        assert!(uninstall(dir.path()).unwrap().is_empty());
    }
}
