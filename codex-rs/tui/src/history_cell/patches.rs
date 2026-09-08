//! Patch summaries and image-tool transcript helpers.

use super::*;
use crate::diff_render::create_grouped_diff_file_summary;
use codex_utils_path_uri::LegacyAppPathString;

/// Consecutive patches share a file summary and retain their individual full diffs.
#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
    entries: Vec<PatchHistoryEntry>,
    cwd: PathBuf,
}

#[derive(Debug)]
enum PatchHistoryEntry {
    Changes(HashMap<PathBuf, FileChange>),
    Transcript(Box<dyn HistoryCell>),
}

impl PatchHistoryCell {
    pub(crate) fn append_changes(&mut self, changes: HashMap<PathBuf, FileChange>) {
        self.entries.push(PatchHistoryEntry::Changes(changes));
    }

    pub(crate) fn append_transcript(&mut self, cell: Box<dyn HistoryCell>) {
        self.entries.push(PatchHistoryEntry::Transcript(cell));
    }
}

impl HistoryCell for PatchHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let patches = self.entries.iter().filter_map(|entry| match entry {
            PatchHistoryEntry::Changes(changes) => Some(changes),
            PatchHistoryEntry::Transcript(_) => None,
        });
        create_grouped_diff_file_summary(patches, &self.cwd, usize::from(width))
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for entry in &self.entries {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.extend(match entry {
                PatchHistoryEntry::Changes(changes) => {
                    create_diff_summary(changes, &self.cwd, usize::from(width))
                }
                PatchHistoryEntry::Transcript(cell) => cell.transcript_lines(width),
            });
        }
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.transcript_lines(RAW_DIFF_SUMMARY_WIDTH as u16))
    }
}
/// Create a new `PendingPatch` cell that lists the file‑level summary of
/// a proposed patch. The summary lines should already be formatted (e.g.
/// "A path/to/file.rs").
pub(crate) fn new_patch_event(
    changes: HashMap<PathBuf, FileChange>,
    cwd: &Path,
) -> PatchHistoryCell {
    PatchHistoryCell {
        entries: vec![PatchHistoryEntry::Changes(changes)],
        cwd: cwd.to_path_buf(),
    }
}

pub(crate) fn new_patch_apply_failure(stderr: String) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Failure title. Red matches every other failure indicator in the transcript (approvals,
    // image generation, hooks); magenta is reserved for the Codex brand accent, not error tone.
    lines.push(Line::from("✗ Failed to apply patch".red().bold()));

    if !stderr.trim().is_empty() {
        let output = output_lines(
            Some(&CommandOutput::new(/*exit_code*/ 1, stderr)),
            OutputLinesParams {
                line_limit: TOOL_CALL_MAX_LINES,
                only_err: true,
                include_angle_pipe: true,
                include_prefix: true,
            },
        );
        lines.extend(output.lines);
    }

    PlainHistoryCell { lines }
}

pub(crate) fn new_view_image_tool_call(path: LegacyAppPathString, cwd: &Path) -> PlainHistoryCell {
    let display_path = path
        .to_inferred_path_uri()
        .and_then(|path| path.to_abs_path().ok())
        .map(|path| display_path_for(path.as_path(), cwd))
        .unwrap_or_else(|| path.into_string());

    let lines: Vec<Line<'static>> = vec![
        vec!["• ".dim(), "Viewed image".bold()].into(),
        vec!["  └ ".dim(), display_path.dim()].into(),
    ];

    PlainHistoryCell { lines }
}

pub(crate) fn new_image_generation_call(
    call_id: String,
    status: &str,
    revised_prompt: Option<String>,
    saved_path: Option<AbsolutePathBuf>,
) -> PlainHistoryCell {
    let detail = revised_prompt.unwrap_or(call_id);
    let heading = if status == "failed" {
        vec!["✗ ".red().bold(), "Image generation failed".bold()].into()
    } else {
        vec!["• ".dim(), "Generated image:".bold()].into()
    };
    let mut lines: Vec<Line<'static>> = vec![heading, vec!["  └ ".dim(), detail.dim()].into()];
    if let Some(saved_path) = saved_path {
        let saved_path = Url::from_file_path(saved_path.as_path())
            .map(|url| url.to_string())
            .unwrap_or_else(|_| saved_path.display().to_string());
        lines.push(vec!["  └ ".dim(), "Saved to: ".dim(), saved_path.into()].into());
    }

    PlainHistoryCell { lines }
}

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;
