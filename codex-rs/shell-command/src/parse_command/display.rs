//! Command summaries for display, including batches containing unsupported commands.
//!
//! Shell expansions remain source text; these summaries do not describe resolved arguments
//! and must not be used for execution or approval decisions.

use super::cd_target;
use super::join_paths;
use super::parse_shell_script;
use super::summarize_main_tokens;
use crate::bash::parse_plain_command_from_node;
use crate::bash::try_parse_shell;
use codex_protocol::parse_command::ParsedCommand;
use std::path::PathBuf;

/// Summarizes separate commands while retaining complex shell statements in full.
///
/// This accepts argument globs for display without treating them as literal runtime arguments.
/// Pipelines retain the existing primary-command summaries, and unsupported statements remain
/// unknown commands rather than hiding the other commands in a batch.
pub fn parse_shell_script_for_display(script: &str) -> Vec<ParsedCommand> {
    let unknown = || {
        vec![ParsedCommand::Unknown {
            cmd: script.to_string(),
        }]
    };
    let Some(tree) = try_parse_shell(script) else {
        return unknown();
    };
    if tree.root_node().has_error() {
        return unknown();
    }

    let mut pending = vec![tree.root_node()];
    let mut commands = Vec::new();
    let mut cwd: Option<String> = None;
    while let Some(node) = pending.pop() {
        let text = &script[node.byte_range()];
        match node.kind() {
            "program" | "list" => {
                let mut cursor = node.walk();
                let children_start = pending.len();
                pending.extend(node.named_children(&mut cursor));
                pending[children_start..].reverse();
            }
            "comment" => {}
            "command" => {
                let Some(tokens) = parse_plain_command_from_node(node, script) else {
                    commands.push(ParsedCommand::Unknown {
                        cmd: text.to_string(),
                    });
                    continue;
                };
                if let Some((head, tail)) = tokens.split_first()
                    && head == "cd"
                    && let Some(dir) = cd_target(tail)
                {
                    cwd = Some(match &cwd {
                        Some(base) => join_paths(base, &dir),
                        None => dir,
                    });
                    continue;
                }
                let mut parsed = summarize_main_tokens(&tokens);
                match &mut parsed {
                    ParsedCommand::Read { cmd, path, .. } => {
                        *cmd = text.to_string();
                        if let Some(base) = &cwd {
                            *path = PathBuf::from(join_paths(base, &path.to_string_lossy()));
                        }
                    }
                    ParsedCommand::ListFiles { cmd, .. }
                    | ParsedCommand::Search { cmd, .. }
                    | ParsedCommand::Unknown { cmd } => *cmd = text.to_string(),
                }
                commands.push(parsed);
            }
            "pipeline" => commands.extend(parse_shell_script(text)),
            _ => commands.push(ParsedCommand::Unknown {
                cmd: text.to_string(),
            }),
        }
    }
    if commands.is_empty() {
        unknown()
    } else {
        commands
    }
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
