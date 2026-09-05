//! Error prose uses a readable measure while raw transcript output retains the original text.

use super::*;

#[derive(Debug)]
pub(crate) struct ErrorHistoryCell {
    pub(super) message: String,
}

impl HistoryCell for ErrorHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let line = Line::from(self.message.clone().red());
        crate::wrapping::word_wrap_line(
            &line,
            RtOptions::new(usize::from(width.min(88)))
                .initial_indent(Line::from("■ ".red()))
                .subsequent_indent(Line::from("  ")),
        )
        .iter()
        .map(line_to_static)
        .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(format!("■ {}", self.message))]
    }
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
