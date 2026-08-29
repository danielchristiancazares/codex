use diffy::Hunk;
use std::borrow::Cow;
use std::path::Path;

pub(super) const RAW_FALLBACK_WARNING: &str =
    "Unable to parse unified diff; showing raw patch text";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UpdateDiffMode {
    Unified,
    RawFallback,
}

#[derive(Debug)]
pub(super) struct PreparedUpdateDiff<'a> {
    source: Cow<'a, str>,
    mode: UpdateDiffMode,
}

impl<'a> PreparedUpdateDiff<'a> {
    pub(super) fn new(diff: &'a str, move_path: Option<&Path>) -> Self {
        if let Some(move_path) = move_path {
            let trailer = format!("\n\nMoved to: {}", move_path.display());
            if let Some(stripped) = diff.strip_suffix(&trailer)
                && parses_with_hunks(stripped)
            {
                return Self {
                    source: Cow::Owned(stripped.to_string()),
                    mode: UpdateDiffMode::Unified,
                };
            }
        }

        let mode = match diffy::Patch::from_str(diff) {
            Ok(patch) if !diff.is_empty() && patch.hunks().is_empty() => {
                UpdateDiffMode::RawFallback
            }
            Ok(_) => UpdateDiffMode::Unified,
            Err(_) => UpdateDiffMode::RawFallback,
        };
        Self {
            source: Cow::Borrowed(diff),
            mode,
        }
    }

    pub(super) fn source(&self) -> &str {
        self.source.as_ref()
    }

    pub(super) fn mode(&self) -> UpdateDiffMode {
        self.mode
    }

    pub(super) fn line_counts(&self) -> (usize, usize) {
        match self.mode {
            UpdateDiffMode::Unified => {
                let Ok(patch) = diffy::Patch::from_str(self.source()) else {
                    return raw_line_counts(self.source());
                };
                patch
                    .hunks()
                    .iter()
                    .flat_map(Hunk::lines)
                    .fold((0, 0), |(added, removed), line| match line {
                        diffy::Line::Insert(_) => (added + 1, removed),
                        diffy::Line::Delete(_) => (added, removed + 1),
                        diffy::Line::Context(_) => (added, removed),
                    })
            }
            UpdateDiffMode::RawFallback => raw_line_counts(self.source()),
        }
    }
}

fn raw_line_counts(source: &str) -> (usize, usize) {
    source.lines().fold((0, 0), |(added, removed), line| {
        if line.starts_with('+') && !line.starts_with("+++") {
            (added + 1, removed)
        } else if line.starts_with('-') && !line.starts_with("---") {
            (added, removed + 1)
        } else {
            (added, removed)
        }
    })
}

fn parses_with_hunks(diff: &str) -> bool {
    diffy::Patch::from_str(diff).is_ok_and(|patch| !patch.hunks().is_empty())
}

#[cfg(test)]
#[path = "update_parse_tests.rs"]
mod tests;
