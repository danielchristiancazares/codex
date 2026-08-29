//! Source-aware queue remapping for active streams.
//!
//! Terminal scrollback cannot reflow rows that have already been emitted. If a render setting
//! changes partway through one wrapped source line, this module keeps that line's queued suffix in
//! its old layout and rebuilds only later source against the new width or mode.

use super::StreamCore;
use super::render_source;

#[derive(Clone, Copy)]
pub(super) struct PartialSourceLine {
    source_start: usize,
    source_end: usize,
}

pub(super) struct RenderRemap {
    had_pending_queue: bool,
    had_live_tail: bool,
    partial_line: Option<PartialSourceLine>,
    partial_queue_len: usize,
    emitted_source_boundary: usize,
}

impl RenderRemap {
    pub(super) fn capture(core: &StreamCore) -> Self {
        let had_pending_queue = core.state.queued_len() > 0;
        let had_live_tail = core.has_tail();
        let partial_line = if had_pending_queue {
            core.layout_bound_source_line
                .or_else(|| partial_source_line_after_rendered_len(core))
        } else {
            None
        };
        let partial_queue_len = partial_line
            .map(|partial_line| partial_source_line_queue_len(core, partial_line))
            .unwrap_or_default();
        let emitted_source_boundary =
            source_boundary_after_rendered_len(core, core.emitted_stable_len);
        Self {
            had_pending_queue,
            had_live_tail,
            partial_line,
            partial_queue_len,
            emitted_source_boundary,
        }
    }

    pub(super) fn apply(self, core: &mut StreamCore) {
        if let Some(partial_line) = self.partial_line {
            preserve_partial_source_line_queue(core, partial_line, self.partial_queue_len);
            return;
        }

        let source = core.state.collector.committed_source().to_string();
        core.emitted_stable_len =
            rendered_len_for_source_boundary(core, &source, self.emitted_source_boundary);
        let target_stable_len = core.compute_target_stable_len();
        if self.had_pending_queue
            && core.emitted_stable_len >= target_stable_len
            && target_stable_len > 0
        {
            core.emitted_stable_len = target_stable_len;
        }
        clear_layout_bound_queue(core);
        core.state.clear_queue();
        if core.emitted_stable_len > 0 && !self.had_pending_queue && !self.had_live_tail {
            core.enqueued_stable_len = core.render.lines.len();
            return;
        }
        core.rebuild_stable_queue_from_render();
    }
}

pub(super) fn account_emitted_step(core: &mut StreamCore, step_len: usize) {
    let mut ordinary_lines = step_len;
    if core.layout_bound_queue_remaining > 0 {
        let layout_bound_lines = ordinary_lines.min(core.layout_bound_queue_remaining);
        ordinary_lines -= layout_bound_lines;
        core.layout_bound_queue_remaining -= layout_bound_lines;
        if core.layout_bound_queue_remaining == 0
            && let Some(target_len) = core.layout_bound_target_emitted_len.take()
        {
            core.emitted_stable_len = target_len;
            core.layout_bound_source_line = None;
        }
    }
    core.emitted_stable_len += ordinary_lines;
}

pub(super) fn defer_queue_for_interruption(core: &mut StreamCore) {
    core.deferred_queue
        .extend(core.state.drain_n(/*max_lines*/ usize::MAX));
    core.emitted_stable_len = core.enqueued_stable_len;
    clear_layout_bound_queue(core);
}

pub(super) fn clear_layout_bound_queue(core: &mut StreamCore) {
    core.layout_bound_queue_remaining = 0;
    core.layout_bound_target_emitted_len = None;
    core.layout_bound_source_line = None;
}

fn preserve_partial_source_line_queue(
    core: &mut StreamCore,
    partial_line: PartialSourceLine,
    partial_queue_len: usize,
) {
    let source = core.state.collector.committed_source().to_string();
    let source_line_start_len =
        rendered_len_for_source_boundary(core, &source, partial_line.source_start);
    let source_line_end_len =
        rendered_len_for_source_boundary(core, &source, partial_line.source_end);
    let tail_budget = core.active_tail_budget_lines();
    let target_stable_len = core.render.lines.len().saturating_sub(tail_budget);
    let previous_queue = core.state.drain_n(/*max_lines*/ usize::MAX);
    let mut queued = previous_queue
        .into_iter()
        .take(partial_queue_len)
        .collect::<Vec<_>>();
    let layout_bound_len = queued.len();

    clear_layout_bound_queue(core);
    core.emitted_stable_len = source_line_start_len;
    if source_line_end_len < target_stable_len {
        queued.extend(core.render.lines[source_line_end_len..target_stable_len].to_vec());
    }
    if !queued.is_empty() {
        core.state.enqueue(queued);
    }
    core.enqueued_stable_len = source_line_end_len.max(target_stable_len);
    if layout_bound_len > 0 {
        core.layout_bound_queue_remaining = layout_bound_len;
        core.layout_bound_target_emitted_len = Some(source_line_end_len);
        core.layout_bound_source_line = Some(partial_line);
    }
}

fn partial_source_line_queue_len(core: &StreamCore, partial_line: PartialSourceLine) -> usize {
    if core.layout_bound_queue_remaining > 0 {
        return core
            .layout_bound_queue_remaining
            .min(core.state.queued_len());
    }

    let source = core.state.collector.committed_source();
    render_prefix_len(core, source, partial_line.source_end)
        .saturating_sub(core.emitted_stable_len)
        .min(core.state.queued_len())
}

fn partial_source_line_after_rendered_len(core: &StreamCore) -> Option<PartialSourceLine> {
    let rendered_len = core.emitted_stable_len;
    if rendered_len == 0 || rendered_len >= core.render.lines.len() {
        return None;
    }

    let source = core.state.collector.committed_source();
    let mut source_start = 0;
    let mut source_start_rendered_len = 0;
    for source_end in source_line_boundaries(source) {
        let source_end_rendered_len = render_prefix_len(core, source, source_end);
        if rendered_len < source_end_rendered_len {
            return (rendered_len > source_start_rendered_len).then_some(PartialSourceLine {
                source_start,
                source_end,
            });
        }
        source_start = source_end;
        source_start_rendered_len = source_end_rendered_len;
    }
    None
}

fn source_boundary_after_rendered_len(core: &StreamCore, rendered_len: usize) -> usize {
    let source = core.state.collector.committed_source();
    if rendered_len == 0 || source.is_empty() {
        return 0;
    }
    if rendered_len >= core.render.lines.len() {
        return source.len();
    }

    let boundaries = source_line_boundaries(source);
    let index = boundaries
        .partition_point(|boundary| render_prefix_len(core, source, *boundary) < rendered_len);
    boundaries.get(index).copied().unwrap_or(source.len())
}

fn rendered_len_for_source_boundary(
    core: &StreamCore,
    source: &str,
    source_boundary: usize,
) -> usize {
    if source_boundary == 0 {
        return 0;
    }
    if source_boundary >= source.len() {
        return core.render.lines.len();
    }
    render_prefix_len(core, source, source_boundary)
}

fn render_prefix_len(core: &StreamCore, source: &str, source_boundary: usize) -> usize {
    render_source(
        &source[..source_boundary.min(source.len())],
        core.width,
        core.cwd.as_path(),
        core.render_mode,
        core.inline_visualization_context.as_ref(),
    )
    .len()
}

fn source_line_boundaries(source: &str) -> Vec<usize> {
    let mut boundaries = source
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    if boundaries.last().copied() != Some(source.len()) {
        boundaries.push(source.len());
    }
    boundaries
}
