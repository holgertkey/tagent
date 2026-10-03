# Install

## Download

Each [GitHub release](https://github.com/holgertkey/tagent/releases/latest) has
ready-made programs for 64-bit Windows and Linux:

| File | Contains |
|------|----------|
| `tagent-cli-<version>-windows-x86_64.zip` | `tagent-cli.exe` |
| `tagent-cli-<version>-linux-x86_64.tar.gz` | `tagent-cli` |
| `tagent-gui-<version>-windows-x86_64.zip` | `tagent-gui.exe` |
| `tagent-gui-<version>-linux-x86_64.tar.gz` | `tagent-gui`, its menu entry (`tagent-gui.desktop`) and icon |
| `tagent-gui_<version>-1_amd64.deb` | `tagent-gui` as a package for Debian, Ubuntu and their relatives |

The two applications have their own version numbers.

### Windows

Unpack the `.zip` anywhere and run the `.exe`. Nothing else is needed.

### Linux

Unpack the archive and run the program, or put it in a folder on your `PATH`
(`~/.local/bin`, for example):

```bash
tar -xzf tagent-cli-<version>-linux-x86_64.tar.gz
./tagent-cli
```

The programs need the X11 and ALSA libraries, which desktop systems have.

For `tagent-gui`, the `.deb` is the easy way: it installs the program, a menu entry and
the icon.

```bash
sudo apt install ./tagent-gui_<version>-1_amd64.deb
```

Without the `.deb`, `tagent-gui --install-desktop` adds the menu entry and the icon for
your user (in `~/.local/share`), pointing at the program you ran; on GNOME, this also
makes the dock show the right icon. `tagent-gui --uninstall-desktop` removes them.

There are no ready-made programs for macOS: install with Cargo (below).

## With Cargo

With [Rust](https://rustup.rs) installed, both applications install from crates.io:

```bash
cargo install tagent-cli
cargo install tagent-gui
```

On Linux, building needs the development packages for X11, XTest, ALSA and fontconfig.
On Debian and Ubuntu:

```bash
sudo apt-get install libx11-dev libxtst-dev libasound2-dev libfontconfig1-dev
```

## From source

```bash
git clone https://github.com/holgertkey/tagent
cd tagent
cargo build --release
```

builds both: `target/release/tagent-cli` and `target/release/tagent-gui` (`.exe` on
Windows). `cargo build --release -p tagent-cli` (or `-p tagent-gui`) builds one. The
same Linux packages as above are needed.

## Uninstall

Delete the program. Its settings stay in your configuration folder until you delete
them too; see [File locations](../reference/file-locations.md). For the `.deb`:
`sudo apt remove tagent-gui`.
