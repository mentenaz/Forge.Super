//! Creation of `std::process::Command` without console window on Windows.
//!
//! Replaces Zed's `gpui_util::new_std_command` in backends and shared tools.

use std::ffi::OsStr;
use std::process::Command;

/// Creates a `std::process::Command` configured with `CREATE_NO_WINDOW` on Windows,
/// so calling it never pops up a black console window.
pub fn new_command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_command_cleanly() {
        let cmd = new_command("git");
        let _ = format!("{cmd:?}");
    }
}
