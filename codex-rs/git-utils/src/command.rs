//! Process construction for Git helpers whose output is consumed by Codex.

use std::ffi::OsStr;
use std::process::Command;

/// Creates a non-interactive Git command without allocating a Windows console.
/// Arguments, environment, working directory, and stdio remain caller-controlled.
pub fn git_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    };
    command
}

#[cfg(all(test, windows))]
#[path = "command_windows_tests.rs"]
mod windows_tests;
