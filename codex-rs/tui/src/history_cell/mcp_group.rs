//! Ordered lifecycle owner for concurrently active MCP tool calls.

use super::HistoryCell;
use super::McpInvocation;
use super::McpToolCallCell;
use super::new_active_mcp_tool_call;
use codex_protocol::mcp::CallToolResult;
use ratatui::text::Line;
use std::time::Duration;

#[cfg(test)]
#[path = "mcp_group_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) struct McpToolCallEntry {
    cell: McpToolCallCell,
    image_result: Option<Box<dyn HistoryCell>>,
}

impl McpToolCallEntry {
    fn new(cell: McpToolCallCell) -> Self {
        Self {
            cell,
            image_result: None,
        }
    }

    fn append_display_lines(&self, width: u16, lines: &mut Vec<Line<'static>>) {
        lines.extend(self.cell.display_lines(width));
        if let Some(image_result) = self.image_result.as_ref() {
            lines.extend(image_result.display_lines(width));
        }
    }

    fn append_transcript_lines(&self, width: u16, lines: &mut Vec<Line<'static>>) {
        lines.extend(self.cell.transcript_lines(width));
        if let Some(image_result) = self.image_result.as_ref() {
            lines.extend(image_result.transcript_lines(width));
        }
    }

    fn append_raw_lines(&self, lines: &mut Vec<Line<'static>>) {
        lines.extend(self.cell.raw_lines());
        if let Some(image_result) = self.image_result.as_ref() {
            lines.extend(image_result.raw_lines());
        }
    }
}

#[derive(Debug)]
pub(crate) struct McpToolCallGroupCell {
    entries: Vec<McpToolCallEntry>,
}

impl McpToolCallGroupCell {
    pub(crate) fn new(
        call_id: String,
        invocation: McpInvocation,
        animations_enabled: bool,
    ) -> Self {
        Self {
            entries: vec![McpToolCallEntry::new(new_active_mcp_tool_call(
                call_id,
                invocation,
                animations_enabled,
            ))],
        }
    }

    pub(crate) fn add_started_call(
        &mut self,
        call_id: String,
        invocation: McpInvocation,
        animations_enabled: bool,
    ) -> bool {
        if self.owns_call(&call_id) {
            return false;
        }
        self.entries
            .push(McpToolCallEntry::new(new_active_mcp_tool_call(
                call_id,
                invocation,
                animations_enabled,
            )));
        true
    }

    pub(crate) fn complete_call(
        &mut self,
        call_id: &str,
        duration: Duration,
        result: Result<CallToolResult, String>,
    ) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.cell.call_id() == call_id)
        else {
            return false;
        };
        if !entry.cell.is_active() {
            return false;
        }
        entry.image_result = entry.cell.complete(duration, result);
        true
    }

    pub(crate) fn owns_call(&self, call_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.cell.call_id() == call_id)
    }

    pub(crate) fn has_active_members(&self) -> bool {
        self.entries.iter().any(|entry| entry.cell.is_active())
    }

    pub(crate) fn mark_all_incomplete_failed(&mut self) {
        for entry in &mut self.entries {
            if entry.cell.is_active() {
                entry.cell.mark_failed();
            }
        }
    }
}

impl HistoryCell for McpToolCallGroupCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for entry in &self.entries {
            entry.append_display_lines(width, &mut lines);
        }
        lines
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for entry in &self.entries {
            entry.append_transcript_lines(width, &mut lines);
        }
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for entry in &self.entries {
            entry.append_raw_lines(&mut lines);
        }
        lines
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        self.entries
            .iter()
            .filter(|entry| entry.cell.is_active())
            .filter_map(|entry| entry.cell.transcript_animation_tick())
            .max()
    }
}
