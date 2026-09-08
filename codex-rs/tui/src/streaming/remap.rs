//! Semantic source-region ownership for active stream queue remapping.
//!
//! Rich Markdown can join physical source lines into one rendered top-level block, so newline
//! boundaries are not sufficient proof that a queued row can be rebuilt independently. The final
//! mutable top-level block is treated as one layout-bound source region. Rows already owned by that
//! region retain their old layout until emission, then bookkeeping advances to the equivalent
//! boundary in the new layout. Raw mode keeps physical newline boundaries because it preserves
//! source line separation.

use super::StreamCore;
use super::render_source;
use crate::history_cell::HistoryRenderMode;
use crate::terminal_hyperlinks::HyperlinkLine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeferredRowsReflow {
    #[default]
    NotRequired,
    Required,
}

#[derive(Default)]
pub(super) struct DeferredRows {
    rows: Vec<HyperlinkLine>,
    reflow: DeferredRowsReflow,
}

impl DeferredRows {
    fn extend(&mut self, rows: Vec<HyperlinkLine>) {
        self.rows.extend(rows);
    }

    pub(super) fn mark_effective_render_mode_changed(&mut self) {
        if !self.rows.is_empty() {
            self.reflow = DeferredRowsReflow::Required;
        }
    }

    pub(super) fn take(&mut self) -> (Vec<HyperlinkLine>, DeferredRowsReflow) {
        let rows = std::mem::take(&mut self.rows);
        let reflow = std::mem::take(&mut self.reflow);
        (rows, reflow)
    }

    pub(super) fn clear(&mut self) {
        self.rows.clear();
        self.reflow = DeferredRowsReflow::NotRequired;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LayoutBoundSourceRegion {
    source_start: usize,
    source_end: usize,
}

pub(super) struct RenderRemap {
    had_pending_queue: bool,
    had_live_tail: bool,
    layout_bound_region: Option<LayoutBoundSourceRegion>,
    layout_bound_queue_len: usize,
    emitted_source_boundary: usize,
}

impl RenderRemap {
    pub(super) fn capture(core: &StreamCore) -> Self {
        let had_pending_queue = core.state.queued_len() > 0;
        let had_live_tail = core.has_tail();
        let (layout_bound_region, layout_bound_queue_len) = if had_pending_queue {
            capture_layout_bound_region(core)
        } else {
            (None, 0)
        };
        let emitted_source_boundary = emitted_source_boundary(core);
        Self {
            had_pending_queue,
            had_live_tail,
            layout_bound_region,
            layout_bound_queue_len,
            emitted_source_boundary,
        }
    }

    pub(super) fn apply(self, core: &mut StreamCore) {
        if let Some(region) = self.layout_bound_region {
            preserve_layout_bound_region(core, region, self.layout_bound_queue_len);
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
            core.layout_bound_source_region = None;
        }
    }
    core.emitted_stable_len += ordinary_lines;
}

pub(super) fn defer_queue_for_interruption(core: &mut StreamCore) {
    core.deferred_rows
        .extend(core.state.drain_n(/*max_lines*/ usize::MAX));
    core.emitted_stable_len = core.enqueued_stable_len;
    clear_layout_bound_queue(core);
}

pub(super) fn clear_layout_bound_queue(core: &mut StreamCore) {
    core.layout_bound_queue_remaining = 0;
    core.layout_bound_target_emitted_len = None;
    core.layout_bound_source_region = None;
}

fn capture_layout_bound_region(core: &StreamCore) -> (Option<LayoutBoundSourceRegion>, usize) {
    if core.layout_bound_queue_remaining > 0 {
        return (
            core.layout_bound_source_region,
            core.layout_bound_queue_remaining
                .min(core.state.queued_len()),
        );
    }
    if core.emitted_stable_len == 0 {
        return (None, 0);
    }

    match core.render_mode {
        HistoryRenderMode::Raw => {
            let region = partial_raw_source_line_after_rendered_len(core);
            let queue_len = region
                .map(|region| raw_source_region_queue_len(core, region))
                .unwrap_or_default();
            (region, queue_len)
        }
        HistoryRenderMode::Rich => {
            let source_len = core.state.collector.committed_source().len();
            let stable_source_len = core.render.stable_source_prefix_len();
            let stable_rendered_len = core.render.stable_rendered_prefix_len();
            let queue_end = core.enqueued_stable_len;
            let region = if queue_end > stable_rendered_len {
                if core.emitted_stable_len >= stable_rendered_len {
                    LayoutBoundSourceRegion {
                        source_start: stable_source_len,
                        source_end: source_len,
                    }
                } else {
                    LayoutBoundSourceRegion {
                        source_start: 0,
                        source_end: source_len,
                    }
                }
            } else if stable_source_len > 0 {
                LayoutBoundSourceRegion {
                    source_start: 0,
                    source_end: stable_source_len,
                }
            } else {
                LayoutBoundSourceRegion {
                    source_start: 0,
                    source_end: source_len,
                }
            };
            (Some(region), core.state.queued_len())
        }
    }
}

fn preserve_layout_bound_region(
    core: &mut StreamCore,
    region: LayoutBoundSourceRegion,
    layout_bound_queue_len: usize,
) {
    let source = core.state.collector.committed_source().to_string();
    let region_start_len = rendered_len_for_source_boundary(core, &source, region.source_start);
    let region_end_len = rendered_len_for_source_boundary(core, &source, region.source_end);
    let tail_budget = core.active_tail_budget_lines();
    let target_stable_len = core.render.lines.len().saturating_sub(tail_budget);
    let previous_queue = core.state.drain_n(/*max_lines*/ usize::MAX);
    let mut queued = previous_queue
        .into_iter()
        .take(layout_bound_queue_len)
        .collect::<Vec<_>>();
    let retained_layout_len = queued.len();

    clear_layout_bound_queue(core);
    core.emitted_stable_len = region_start_len;
    if region_end_len < target_stable_len {
        queued.extend(core.render.lines[region_end_len..target_stable_len].to_vec());
    }
    if !queued.is_empty() {
        core.state.enqueue(queued);
    }
    core.enqueued_stable_len = region_end_len.max(target_stable_len);
    if retained_layout_len > 0 {
        core.layout_bound_queue_remaining = retained_layout_len;
        core.layout_bound_target_emitted_len = Some(region_end_len);
        core.layout_bound_source_region = Some(region);
    }
}

fn emitted_source_boundary(core: &StreamCore) -> usize {
    let source = core.state.collector.committed_source();
    if core.emitted_stable_len == 0 || source.is_empty() {
        return 0;
    }
    if core.emitted_stable_len >= core.render.lines.len() {
        return source.len();
    }
    match core.render_mode {
        HistoryRenderMode::Raw => {
            raw_source_boundary_after_rendered_len(source, core.emitted_stable_len)
        }
        HistoryRenderMode::Rich => {
            let stable_rendered_len = core.render.stable_rendered_prefix_len();
            if stable_rendered_len > 0 && core.emitted_stable_len >= stable_rendered_len {
                core.render.stable_source_prefix_len()
            } else {
                0
            }
        }
    }
}

fn raw_source_region_queue_len(core: &StreamCore, region: LayoutBoundSourceRegion) -> usize {
    render_prefix_len(
        core,
        core.state.collector.committed_source(),
        region.source_end,
    )
    .saturating_sub(core.emitted_stable_len)
    .min(core.state.queued_len())
}

fn partial_raw_source_line_after_rendered_len(
    core: &StreamCore,
) -> Option<LayoutBoundSourceRegion> {
    let rendered_len = core.emitted_stable_len;
    if rendered_len == 0 || rendered_len >= core.render.lines.len() {
        return None;
    }

    let source = core.state.collector.committed_source();
    let mut source_start = 0;
    let mut source_start_rendered_len = 0;
    for source_end in raw_source_line_boundaries(source) {
        let source_end_rendered_len = render_prefix_len(core, source, source_end);
        if rendered_len < source_end_rendered_len {
            return (rendered_len > source_start_rendered_len).then_some(LayoutBoundSourceRegion {
                source_start,
                source_end,
            });
        }
        source_start = source_end;
        source_start_rendered_len = source_end_rendered_len;
    }
    None
}

fn raw_source_boundary_after_rendered_len(source: &str, rendered_len: usize) -> usize {
    raw_source_line_boundaries(source)
        .get(rendered_len.saturating_sub(1))
        .copied()
        .unwrap_or(source.len())
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
    if core.render_mode == HistoryRenderMode::Rich
        && source_boundary == core.render.stable_source_prefix_len()
    {
        return core.render.stable_rendered_prefix_len();
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

fn raw_source_line_boundaries(source: &str) -> Vec<usize> {
    let mut boundaries = source
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    if boundaries.last().copied() != Some(source.len()) {
        boundaries.push(source.len());
    }
    boundaries
}
