//! Streaming markdown render metadata collected during the writer's single parse pass.
//!
//! Top-level block offsets always refer to the exact source passed to this renderer; callers that
//! normalize source before rendering must not apply those offsets to the original source.

use super::DecodedTextMerge;
use super::Event;
use super::HyperlinkLine;
use super::Options;
use super::Parser;
use super::Tag;
use super::Writer;
use super::never_hide_link_destination;
use std::path::Path;

/// Rendered lines and the block metadata needed to keep only the final block mutable.
pub(crate) struct StreamingMarkdownRender {
    /// Styled output produced by the same parser pass that collected the metadata below.
    pub(crate) lines: Vec<HyperlinkLine>,
    /// Byte offset of the final top-level block when at least one earlier block exists.
    pub(crate) last_top_level_block_start: Option<usize>,
    /// Number of rendered lines in the completed prefix before the final top-level block.
    pub(crate) stable_prefix_rendered_len: Option<usize>,
    /// Whether a reference definition can retroactively change another block's rendering.
    pub(crate) has_reference_link_definition: bool,
    /// Whether the first block is raw HTML, which joins a retained prefix without a separator.
    pub(crate) first_top_level_block_is_html: bool,
}

/// Render `input` while tracking the final mutable top-level block.
///
/// Every reported byte offset indexes the exact `input` passed here. Callers that transform source
/// before rendering must map the offset back to their original source before retaining a prefix.
pub(crate) fn render_streaming_markdown_lines_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    cwd: Option<&Path>,
) -> StreamingMarkdownRender {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(input, options);
    let has_reference_link_definition = parser.reference_definitions().iter().next().is_some();
    let parser = DecodedTextMerge::new(parser.into_offset_iter());
    let mut writer = Writer::new(input, parser, width, cwd, &never_hide_link_destination);
    let mut tracker = TopLevelBlockTracker::default();
    writer.run_with_event_observer(|writer, event, range| {
        if tracker.starts_top_level_block(event) {
            // Match the line count produced by rendering the completed prefix on its own. The
            // next block's separator is intentionally left out of this boundary.
            writer.flush_current_line();
            tracker.record_block_start(event, range.start, writer.text.len());
        }
        tracker.advance_depth(event);
    });
    let has_stable_prefix = tracker.block_count > 1;
    StreamingMarkdownRender {
        lines: writer.text,
        last_top_level_block_start: has_stable_prefix.then_some(tracker.last_source_start),
        stable_prefix_rendered_len: has_stable_prefix.then_some(tracker.stable_prefix_rendered_len),
        has_reference_link_definition,
        first_top_level_block_is_html: tracker.first_is_html,
    }
}

/// Records top-level source and rendered-line boundaries during the writer's parser pass.
#[derive(Default)]
struct TopLevelBlockTracker {
    depth: usize,
    block_count: usize,
    last_source_start: usize,
    stable_prefix_rendered_len: usize,
    first_is_html: bool,
}

impl TopLevelBlockTracker {
    fn starts_top_level_block(&self, event: &Event<'_>) -> bool {
        self.depth == 0 && matches!(event, Event::Start(_) | Event::Rule | Event::Html(_))
    }

    fn record_block_start(
        &mut self,
        event: &Event<'_>,
        source_start: usize,
        rendered_start: usize,
    ) {
        self.block_count += 1;
        self.last_source_start = source_start;
        self.stable_prefix_rendered_len = rendered_start;
        if self.block_count == 1 {
            self.first_is_html = matches!(event, Event::Start(Tag::HtmlBlock) | Event::Html(_));
        }
    }

    fn advance_depth(&mut self, event: &Event<'_>) {
        match event {
            Event::Start(_) => self.depth += 1,
            Event::End(_) => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
    }
}
