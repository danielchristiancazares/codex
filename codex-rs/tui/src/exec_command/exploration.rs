//! Recovers display summaries for command batches collapsed by protocol parsing.
//! This classification only controls transcript grouping, never execution or approval.

use codex_protocol::parse_command::ParsedCommand;
use codex_shell_command::bash::parse_shell_script_into_commands;
use codex_shell_command::parse_command::parse_shell_script_for_display;

pub(crate) fn expand_command_summaries(parsed: Vec<ParsedCommand>) -> Vec<ParsedCommand> {
    let [ParsedCommand::Unknown { cmd }] = parsed.as_slice() else {
        return parsed;
    };
    parse_shell_script_for_display(cmd)
}

pub(crate) fn is_exploration_command(parsed: &ParsedCommand) -> bool {
    let cmd = match parsed {
        ParsedCommand::Read { .. }
        | ParsedCommand::ListFiles { .. }
        | ParsedCommand::Search { .. } => return true,
        ParsedCommand::Unknown { cmd } => cmd,
    };
    let Some(commands) = parse_shell_script_into_commands(cmd) else {
        return false;
    };
    let [words] = commands.as_slice() else {
        return false;
    };
    let Some((program, arguments)) = words.split_first() else {
        return false;
    };
    if !matches!(program.rsplit(['/', '\\']).next(), Some("git" | "git.exe")) {
        return false;
    }
    let mut arguments = arguments.iter().map(String::as_str);
    let subcommand = loop {
        match arguments.next() {
            Some("--no-pager" | "-P") => {}
            Some("-C") => {
                if arguments.next().is_none() {
                    return false;
                }
            }
            argument => break argument,
        }
    };
    matches!(subcommand, Some("diff" | "status" | "log" | "show"))
        && !arguments.any(|argument| argument == "--output" || argument.starts_with("--output="))
}

#[cfg(test)]
#[path = "exploration_tests.rs"]
mod tests;
