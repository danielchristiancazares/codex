//! Compact command rosters retain each command in a shell batch and its shared output.

use super::CompactCallState;
use super::ExecCell;
use super::activity_marker;
use super::compact_branch;
use super::compact_call_state;
use super::exploration::ExplorationAction;
use super::exploration::command_action;
use super::format_unified_exec_interaction;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use codex_ansi_escape::ansi_escape_line;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::style::Stylize;

impl ExecCell {
    pub(super) fn compact_group_display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let uses_compact_lifecycle = self.calls.len() > 1
            || self.calls.iter().all(|call| {
                matches!(
                    call.source,
                    ExecCommandSource::Agent | ExecCommandSource::UnifiedExecStartup
                )
            });
        if uses_compact_lifecycle {
            let mut commands = Vec::new();
            for call in &self.calls {
                if !call.is_unified_exec_interaction() && call.parsed.len() > 1 {
                    commands.extend(call.parsed.iter().map(|command| (call, Some(command))));
                } else {
                    commands.push((call, None));
                }
            }
            let command_count = commands.len();
            let active_count = commands
                .iter()
                .filter(|(call, _)| compact_call_state(call) == CompactCallState::Active)
                .count();
            let failed_count = self
                .calls
                .iter()
                .filter(|call| compact_call_state(call) == CompactCallState::Failed)
                .count();
            let noun = if command_count == 1 {
                "command"
            } else {
                "commands"
            };
            let activity = if active_count == 0 {
                format!("Ran {command_count} {noun}")
            } else if active_count == command_count {
                format!("Running {command_count} {noun}")
            } else {
                format!("Running {active_count} of {command_count} {noun}")
            };
            let marker = if let Some(active_call) = self
                .calls
                .iter()
                .find(|call| compact_call_state(call) == CompactCallState::Active)
            {
                activity_marker(active_call.start_time, self.animations_enabled())
            } else if failed_count > 0 {
                "•".red().bold()
            } else {
                "•".dim()
            };
            let mut header = Line::from(vec![marker, " ".into(), activity.bold()]);
            if failed_count > 0 {
                header.push_span(" · ".dim());
                let failure = if self.calls.iter().any(|call| {
                    call.parsed.len() > 1 && compact_call_state(call) == CompactCallState::Failed
                }) {
                    "failed".to_string()
                } else {
                    format!("{failed_count} failed")
                };
                header.push_span(failure.red());
            }
            let mut lines = vec![truncate_line_with_ellipsis_if_overflow(
                header,
                usize::from(width.max(1)),
            )];

            for (index, (call, summary)) in commands.into_iter().enumerate() {
                let state = compact_call_state(call);
                let prefix = if index + 1 == command_count {
                    "  └ "
                } else {
                    "  ├ "
                };
                let command_line = if let Some(summary) = summary {
                    // The shell reports one outcome for the batch, not per-command results.
                    let mut line = Line::from(prefix.dim());
                    let (action, spans) = command_action(summary);
                    if action != ExplorationAction::Run {
                        line.push_span(action.label(state).cyan());
                        line.push_span(" ");
                    }
                    line.extend(spans);
                    line
                } else {
                    let command = if call.is_unified_exec_interaction() {
                        format_unified_exec_interaction(
                            &call.command,
                            call.interaction_input.as_deref(),
                        )
                    } else {
                        strip_bash_lc_and_escape(&call.command).lines().join(" ")
                    };
                    Line::from(vec![compact_branch(prefix, state), command.into()])
                };
                lines.push(truncate_line_with_ellipsis_if_overflow(
                    command_line,
                    usize::from(width.max(1)),
                ));
            }

            let previews = self
                .calls
                .iter()
                .filter_map(|call| {
                    if call.is_unified_exec_interaction() {
                        return None;
                    }
                    let output = call.output.as_ref()?;
                    let mut meaningful_rows = output.lines().rev().filter_map(|raw| {
                        let row = raw.as_ref().trim();
                        let is_divider = row
                            .chars()
                            .all(|ch| matches!(ch, ' ' | '-' | '=' | '_' | '─' | '━'));
                        (!row.is_empty() && !is_divider).then(|| row.to_string())
                    });
                    let preview = meaningful_rows.next()?;
                    let has_context = meaningful_rows.next().is_some();
                    Some((compact_call_state(call), preview, has_context))
                })
                .collect_vec();
            let preview = previews
                .iter()
                .rev()
                .find(|(state, _, _)| *state == CompactCallState::Failed)
                .or_else(|| {
                    previews
                        .iter()
                        .rev()
                        .find(|(_, _, has_context)| *has_context)
                });
            if let Some((state, preview, _)) = preview {
                let mut preview_line = Line::from("    ");
                if *state == CompactCallState::Failed {
                    preview_line.push_span("Error: ".red());
                }
                preview_line.extend(ansi_escape_line(preview));
                preview_line.spans.iter_mut().for_each(|span| {
                    // The retained final line is the command's outcome, so keep it readable.
                    span.style = match state {
                        CompactCallState::Active => span.style.add_modifier(Modifier::DIM),
                        CompactCallState::Succeeded | CompactCallState::Failed => {
                            span.style.remove_modifier(Modifier::DIM)
                        }
                    };
                });
                lines.push(truncate_line_with_ellipsis_if_overflow(
                    preview_line,
                    usize::from(width.max(1)),
                ));
            }
            return lines;
        }

        let completed_commands = self
            .calls
            .iter()
            .take_while(|call| {
                matches!(
                    call.source,
                    ExecCommandSource::Agent | ExecCommandSource::UnifiedExecStartup
                ) && call.duration.is_some()
                    && call
                        .output
                        .as_ref()
                        .is_some_and(|output| output.exit_code == 0)
            })
            .count();
        let completed_commands = if self.is_active()
            && self.calls[..completed_commands]
                .iter()
                .all(Self::is_exploring_call)
        {
            0
        } else {
            completed_commands
        };
        let mut lines = Vec::new();
        if completed_commands > 0 {
            let noun = if completed_commands == 1 {
                "command"
            } else {
                "commands"
            };
            lines.push(Line::from(vec![
                "•".dim(),
                " ".into(),
                format!("Ran {completed_commands} {noun}").bold(),
            ]));
        }
        let remaining_calls = &self.calls[completed_commands..];
        if !self.is_active() {
            for call in remaining_calls {
                lines.extend(self.command_display_lines(call, width));
            }
            return lines;
        }

        let mut calls = remaining_calls;
        let mut first_group = true;
        while let [first, remaining @ ..] = calls {
            let group_len = if Self::is_exploring_call(first) {
                1 + remaining
                    .iter()
                    .take_while(|call| Self::is_exploring_call(call))
                    .count()
            } else {
                1
            };
            let (group, remaining) = calls.split_at(group_len);
            calls = remaining;
            if first_group {
                first_group = false;
            } else {
                lines.push("".into());
            }
            if group.iter().all(Self::is_exploring_call) {
                lines.extend(self.exploring_display_lines(group, width));
            } else {
                lines.extend(self.command_display_lines(&group[0], width));
            }
        }
        lines
    }
}
