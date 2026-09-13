//! Exploration rendering in command order, coalescing consecutive read-only calls.

use super::ExecCall;
use super::ExecCell;
use super::activity_marker;
use crate::render::line_utils::prefix_lines;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use codex_protocol::parse_command::ParsedCommand;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::style::Stylize;

impl ExecCell {
    pub(super) fn exploring_display_lines(
        &self,
        calls: &[ExecCall],
        width: u16,
    ) -> Vec<Line<'static>> {
        let active_start_time = calls
            .iter()
            .find(|call| call.duration.is_none())
            .and_then(|call| call.start_time);
        let is_active = calls.iter().any(|call| call.duration.is_none());
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
        let mut out_indented = Vec::new();
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
            let call_lines: Vec<(&str, Vec<Span<'static>>)> = if reads_only {
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
                    "Read",
                    Itertools::intersperse(names.map(Into::into), ", ".dim()).collect(),
                )]
            } else {
                call.parsed
                    .iter()
                    .map(|parsed| match parsed {
                        ParsedCommand::Read { name, .. } => ("Read", vec![name.clone().into()]),
                        ParsedCommand::ListFiles { cmd, path } => (
                            "List",
                            vec![path.clone().unwrap_or_else(|| cmd.clone()).into()],
                        ),
                        ParsedCommand::Search { cmd, query, path } => {
                            let spans = match (query, path) {
                                (Some(q), Some(p)) => {
                                    vec![q.clone().into(), " in ".dim(), p.clone().into()]
                                }
                                (Some(q), None) => vec![q.clone().into()],
                                (None, Some(_) | None) => vec![cmd.clone().into()],
                            };
                            ("Search", spans)
                        }
                        ParsedCommand::Unknown { cmd } => ("Run", vec![cmd.clone().into()]),
                    })
                    .collect()
            };

            for (title, spans) in call_lines {
                let line = Line::from(spans);
                let initial_indent = Line::from(vec![title.cyan(), " ".into()]);
                let subsequent_indent = " ".repeat(initial_indent.width()).into();
                let wrapped = adaptive_wrap_line(
                    &line,
                    RtOptions::new(usize::from(width))
                        .initial_indent(initial_indent)
                        .subsequent_indent(subsequent_indent),
                );
                push_owned_lines(&wrapped, &mut out_indented);
            }
        }

        out.extend(prefix_lines(out_indented, "  └ ".dim(), "    ".into()));
        out
    }
}

#[cfg(test)]
#[path = "exploration_tests.rs"]
mod tests;
