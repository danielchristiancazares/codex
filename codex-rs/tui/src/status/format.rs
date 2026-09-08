use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::collections::BTreeSet;

use crate::width::display_width;

#[derive(Debug, Clone)]
pub(crate) struct FieldFormatter {
    indent: &'static str,
    label_width: usize,
    value_offset: usize,
    value_indent: String,
}

impl FieldFormatter {
    pub(crate) const INDENT: &'static str = " ";

    pub(crate) fn from_labels<S>(labels: impl IntoIterator<Item = S>) -> Self
    where
        S: AsRef<str>,
    {
        let label_width = labels
            .into_iter()
            .map(|label| display_width(label.as_ref()))
            .max()
            .unwrap_or(0);
        let indent_width = display_width(Self::INDENT);
        let value_offset = indent_width + label_width + 1 + 3;

        Self {
            indent: Self::INDENT,
            label_width,
            value_offset,
            value_indent: " ".repeat(value_offset),
        }
    }

    pub(crate) fn with_max_label_width(mut self, max_width: usize) -> Self {
        self.label_width = self.label_width.min(max_width);
        self.value_offset = display_width(self.indent) + self.label_width + 1 + 3;
        self.value_indent = " ".repeat(self.value_offset);
        self
    }

    pub(crate) fn line(
        &self,
        label: &'static str,
        value_spans: Vec<Span<'static>>,
    ) -> Line<'static> {
        Line::from(self.full_spans(label, value_spans))
    }

    pub(crate) fn continuation(&self, mut spans: Vec<Span<'static>>) -> Line<'static> {
        let mut all_spans = Vec::with_capacity(spans.len() + 1);
        all_spans.push(Span::from(self.value_indent.clone()).dim());
        all_spans.append(&mut spans);
        Line::from(all_spans)
    }

    pub(crate) fn value_width(&self, available_inner_width: usize) -> usize {
        available_inner_width.saturating_sub(self.value_offset)
    }

    pub(crate) fn full_spans(
        &self,
        label: &str,
        mut value_spans: Vec<Span<'static>>,
    ) -> Vec<Span<'static>> {
        let mut spans = Vec::with_capacity(value_spans.len() + 1);
        spans.push(self.label_span(label));
        spans.append(&mut value_spans);
        spans
    }

    fn label_span(&self, label: &str) -> Span<'static> {
        let compact_label = if display_width(label) > self.label_width {
            if let Some(prefix) = label.strip_suffix(" credit limit") {
                format!("{prefix} credits")
            } else if let Some(prefix) = label.strip_suffix(" usage limit") {
                format!("{prefix} usage")
            } else if let Some(prefix) = label.strip_suffix(" limit") {
                prefix.to_string()
            } else {
                label.to_string()
            }
        } else {
            label.to_string()
        };
        let label = crate::text_formatting::truncate_text(&compact_label, self.label_width);
        let label = label
            .strip_suffix("...")
            .map_or(label.clone(), |prefix| format!("{prefix}…"));
        let mut buf = String::with_capacity(self.value_offset);
        buf.push_str(self.indent);

        buf.push_str(&label);
        buf.push(':');

        let label_width = display_width(&label);
        let padding = 3 + self.label_width.saturating_sub(label_width);
        for _ in 0..padding {
            buf.push(' ');
        }

        Span::from(buf).dim()
    }
}

pub(crate) fn push_label(labels: &mut Vec<String>, seen: &mut BTreeSet<String>, label: &str) {
    if seen.contains(label) {
        return;
    }

    let owned = label.to_string();
    seen.insert(owned.clone());
    labels.push(owned);
}
