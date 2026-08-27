use shlex::split as shlex_split;

use super::try_parse_powershell_commands;

#[derive(Clone, Copy)]
enum Quote {
    Single,
    Double,
}

/// Parses a PowerShell script into command-scoped word vectors for dangerous-command heuristics.
///
/// The structured parser handles its supported literal subset. The fallback retains statement
/// boundaries while preserving quoted separators, comments, and continued lines, allowing the
/// existing best-effort word heuristics to cover more complex syntax without correlating words
/// from separate commands.
pub(super) fn parse_powershell_script_commands(script: &str) -> Option<Vec<Vec<String>>> {
    if let Some(commands) = try_parse_powershell_commands(script) {
        return Some(commands);
    }

    let mut commands = Vec::new();
    let mut current = String::new();
    let mut chars = script.chars().peekable();
    let mut quote = None;
    let mut grouping_depth = 0usize;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if matches!(ch, '\r' | '\n') {
                in_line_comment = false;
                push_command(&mut commands, &mut current)?;
            }
            continue;
        }

        if in_block_comment {
            if ch == '#' && chars.next_if_eq(&'>').is_some() {
                in_block_comment = false;
                current.push(' ');
            }
            continue;
        }

        match quote {
            Some(Quote::Single) => {
                current.push(ch);
                if ch == '\'' {
                    if chars.next_if_eq(&'\'').is_some() {
                        current.push('\'');
                    } else {
                        quote = None;
                    }
                }
            }
            Some(Quote::Double) => {
                current.push(ch);
                if ch == '`' {
                    if let Some(escaped) = chars.next() {
                        current.push(escaped);
                    }
                } else if ch == '"' {
                    quote = None;
                }
            }
            None => match ch {
                '\'' => {
                    quote = Some(Quote::Single);
                    current.push(ch);
                }
                '"' => {
                    quote = Some(Quote::Double);
                    current.push(ch);
                }
                '`' => {
                    let escaped = chars.next()?;
                    if escaped == '\r' {
                        chars.next_if_eq(&'\n');
                        current.push(' ');
                    } else if escaped == '\n' {
                        current.push(' ');
                    } else {
                        current.push(ch);
                        current.push(escaped);
                    }
                }
                '<' if chars.next_if_eq(&'#').is_some() => {
                    in_block_comment = true;
                    current.push(' ');
                }
                '#' if current.chars().next_back().is_none_or(char::is_whitespace) => {
                    in_line_comment = true;
                }
                '(' | '[' => {
                    grouping_depth += 1;
                    current.push(ch);
                }
                ')' | ']' => {
                    grouping_depth = grouping_depth.saturating_sub(/*rhs*/ 1);
                    current.push(ch);
                }
                '\r' | '\n' if grouping_depth > 0 => current.push(' '),
                ';' | '|' | '&' | '\r' | '\n' => {
                    push_command(&mut commands, &mut current)?;
                }
                _ => current.push(ch),
            },
        }
    }

    if quote.is_some() || in_block_comment {
        return None;
    }
    push_command(&mut commands, &mut current)?;
    Some(commands)
}

fn push_command(commands: &mut Vec<Vec<String>>, current: &mut String) -> Option<()> {
    let segment = current.trim();
    if !segment.is_empty() {
        let words = shlex_split(segment)?;
        if !words.is_empty() {
            commands.push(words);
        }
    }
    current.clear();
    Some(())
}
