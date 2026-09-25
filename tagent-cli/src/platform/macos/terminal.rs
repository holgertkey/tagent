use std::io::{self, IsTerminal, Write};

/// Sets the terminal window's title for as long as it lives, and restores the
/// previous one when dropped.
///
/// Uses the xterm OSC 0 sequence, which virtually every terminal emulator honors.
/// The previous title is saved/restored through xterm's title stack (`CSI 22 t` /
/// `CSI 23 t`); a terminal without one just ignores those, leaving the title for
/// the shell to reset. Does nothing when stdout isn't a terminal, so no escape
/// sequences end up in redirected output.
pub struct TerminalTitle {
    enabled: bool,
    current: Option<String>,
}

impl TerminalTitle {
    /// Saves the current title (if stdout is a terminal).
    pub fn new() -> Self {
        let enabled = io::stdout().is_terminal();
        if enabled {
            write_sequence("\x1b[22;0t");
        }
        Self {
            enabled,
            current: None,
        }
    }

    /// Sets the title, skipping the write when it's unchanged.
    pub fn set(&mut self, title: &str) {
        if !self.enabled || self.current.as_deref() == Some(title) {
            return;
        }
        write_sequence(&format!("\x1b]0;{}\x07", sanitize(title)));
        self.current = Some(title.to_string());
    }
}

impl Drop for TerminalTitle {
    fn drop(&mut self) {
        if self.enabled {
            write_sequence("\x1b[23;0t");
        }
    }
}

/// Drops control characters, which would end or corrupt the OSC sequence.
fn sanitize(title: &str) -> String {
    title.chars().filter(|c| !c.is_control()).collect()
}

fn write_sequence(sequence: &str) {
    let mut stdout = io::stdout();
    let _ = stdout.write_all(sequence.as_bytes());
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_control_characters() {
        assert_eq!(sanitize("Tagent\x07 — auto\x1b → ru\n"), "Tagent — auto → ru");
    }
}
