//! Semantic exploration rendering, grouped by action in first-seen order.
//!
//! Each action retains its details in encounter order and stays active until all of its calls
//! finish. Consecutive read-only calls retain their existing filename deduplication.

use super::CompactCallState;
use super::ExecCall;
use super::ExecCell;
use super::activity_marker;
use super::compact_branch;
use super::compact_call_state;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use codex_protocol::parse_command::ParsedCommand;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::style::Stylize;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ExplorationAction {
    Read,
    Search,
    ListFiles,
    Run,
}

impl ExplorationAction {
    pub(super) fn label(self, state: CompactCallState) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Search if state == CompactCallState::Active => "Search",
            Self::Search => "Searched",
            Self::ListFiles if state == CompactCallState::Active => "List",
            Self::ListFiles => "Listed",
            Self::Run if state == CompactCallState::Active => "Run",
            Self::Run => "Ran",
        }
    }
}

pub(super) fn command_action(parsed: &ParsedCommand) -> (ExplorationAction, Vec<Span<'static>>) {
    match parsed {
        ParsedCommand::Read { name, .. } => (ExplorationAction::Read, vec![name.clone().into()]),
        ParsedCommand::ListFiles { cmd, path } => (
            ExplorationAction::ListFiles,
            vec![path.clone().unwrap_or_else(|| cmd.clone()).into()],
        ),
        ParsedCommand::Search { cmd, query, path } => {
            let spans = match (query, path) {
                (Some(q), Some(p)) => vec![q.clone().into(), " in ".dim(), p.clone().into()],
                (Some(q), None) => vec![q.clone().into()],
                (None, Some(_) | None) => vec![cmd.clone().into()],
            };
            (ExplorationAction::Search, spans)
        }
        ParsedCommand::Unknown { cmd } => {
            (ExplorationAction::Run, vec![cmd.lines().join(" ").into()])
        }
    }
}

struct ExplorationBlock {
    action: ExplorationAction,
    spans: Vec<Span<'static>>,
    state: CompactCallState,
}

impl ExecCell {
    pub(super) fn exploring_display_lines(
        &self,
        calls: &[ExecCall],
        width: u16,
    ) -> Vec<Line<'static>> {
        let active_start_time = calls
            .iter()
            .find(|call| compact_call_state(call) == CompactCallState::Active)
            .and_then(|call| call.start_time);
        let is_active = calls
            .iter()
            .any(|call| compact_call_state(call) == CompactCallState::Active);
        let mut out = vec![Line::from(vec![
            if is_active {
                activity_marker(active_start_time, self.animations_enabled())
            } else {
                "•".dim()
            },
            " ".into(),
            if is_active {
                "Exploring".bold()
            } else {
                "Explored".bold()
            },
        ])];

        let mut calls = calls;
        let mut action_blocks: Vec<ExplorationBlock> = Vec::new();
        while let Some((call, remaining)) = calls.split_first() {
            let reads_only = call
                .parsed
                .iter()
                .all(|parsed| matches!(parsed, ParsedCommand::Read { .. }));
            let group_len = if reads_only {
                1 + remaining
                    .iter()
                    .take_while(|next| {
                        next.parsed
                            .iter()
                            .all(|parsed| matches!(parsed, ParsedCommand::Read { .. }))
                    })
                    .count()
            } else {
                1
            };
            let (group, remaining) = calls.split_at(group_len);
            calls = remaining;
            let group_state = if group
                .iter()
                .any(|call| compact_call_state(call) == CompactCallState::Failed)
            {
                CompactCallState::Failed
            } else if group
                .iter()
                .any(|call| compact_call_state(call) == CompactCallState::Active)
            {
                CompactCallState::Active
            } else {
                CompactCallState::Succeeded
            };

            let call_lines: Vec<(ExplorationAction, Vec<Span<'static>>)> = if reads_only {
                let names = group
                    .iter()
                    .flat_map(|call| &call.parsed)
                    .map(|parsed| match parsed {
                        ParsedCommand::Read { name, .. } => name.clone(),
                        ParsedCommand::ListFiles { .. }
                        | ParsedCommand::Search { .. }
                        | ParsedCommand::Unknown { .. } => {
                            unreachable!("read-only groups contain only reads")
                        }
                    })
                    .unique();
                vec![(
                    ExplorationAction::Read,
                    Itertools::intersperse(names.map(Into::into), ", ".dim()).collect(),
                )]
            } else {
                call.parsed.iter().map(command_action).collect()
            };

            for (action, spans) in call_lines {
                if let Some(block) = action_blocks
                    .iter_mut()
                    .find(|block| block.action == action)
                {
                    block.spans.push(", ".dim());
                    block.spans.extend(spans);
                    let states = [block.state, group_state];
                    block.state = if states.contains(&CompactCallState::Failed) {
                        CompactCallState::Failed
                    } else if states.contains(&CompactCallState::Active) {
                        CompactCallState::Active
                    } else {
                        CompactCallState::Succeeded
                    };
                } else {
                    action_blocks.push(ExplorationBlock {
                        action,
                        spans,
                        state: group_state,
                    });
                }
            }
        }

        let block_count = action_blocks.len();
        for (index, block) in action_blocks.into_iter().enumerate() {
            let title = block.action.label(block.state);
            let line = Line::from(block.spans);
            let initial_indent = Line::from(vec![title.cyan(), " ".into()]);
            let subsequent_indent = " ".repeat(initial_indent.width()).into();
            let wrapped = adaptive_wrap_line(
                &line,
                RtOptions::new(usize::from(width.saturating_sub(4).max(1)))
                    .initial_indent(initial_indent)
                    .subsequent_indent(subsequent_indent),
            );
            let mut lines = Vec::new();
            push_owned_lines(&wrapped, &mut lines);

            let is_last = index + 1 == block_count;
            let initial_prefix = if is_last { "  └ " } else { "  ├ " };
            let subsequent_prefix = if is_last { "    " } else { "  │ " };
            out.extend(
                prefix_lines(
                    lines,
                    compact_branch(initial_prefix, block.state),
                    subsequent_prefix.dim(),
                )
                .into_iter()
                .map(|line| {
                    truncate_line_with_ellipsis_if_overflow(line, usize::from(width.max(1)))
                }),
            );
        }
        out
    }
}

#[cfg(test)]
#[path = "exploration_tests.rs"]
mod tests;
