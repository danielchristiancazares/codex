//! Session-owned recovery of text captured before model-facing truncation.

use std::collections::VecDeque;
use std::fmt;

const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SESSION_BYTES: usize = 32 * 1024 * 1024;
const MAX_CAPTURES: usize = 32;
const MAX_QUERY_BYTES: usize = 3_200;
const MAX_RANGE_BYTES: usize = 2_800;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureId([u8; 16]);

impl CaptureId {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for CaptureId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct NonEmptyString(String);

impl std::ops::Deref for NonEmptyString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for NonEmptyString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug)]
pub struct NonEmptyTextRequired;

impl fmt::Display for NonEmptyTextRequired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("text must contain at least one byte")
    }
}

impl std::error::Error for NonEmptyTextRequired {}

impl TryFrom<String> for NonEmptyString {
    type Error = NonEmptyTextRequired;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(NonEmptyTextRequired);
        }
        Ok(Self(value))
    }
}

#[derive(Debug)]
pub struct SearchText(NonEmptyString);

#[derive(Debug)]
pub struct SearchTextRejected;

impl fmt::Display for SearchTextRejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("search text must contain 1 to 512 bytes")
    }
}

impl std::error::Error for SearchTextRejected {}

impl TryFrom<String> for SearchText {
    type Error = SearchTextRejected;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > 512 {
            return Err(SearchTextRejected);
        }
        Ok(Self(NonEmptyString(value)))
    }
}

#[derive(Debug)]
pub struct CaptureRange {
    start: usize,
    end: usize,
}

#[derive(Debug)]
pub struct RangeRejected;

impl fmt::Display for RangeRejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("byte ranges must be ordered and span at most 2800 bytes")
    }
}

impl std::error::Error for RangeRejected {}

impl CaptureRange {
    pub fn new(start: usize, end: usize) -> Result<Self, RangeRejected> {
        if end < start || end - start > MAX_RANGE_BYTES {
            return Err(RangeRejected);
        }
        Ok(Self { start, end })
    }
}

#[derive(Debug)]
pub enum CaptureQuery {
    Range(CaptureRange),
    Search(SearchText),
}

#[derive(Debug)]
pub struct CaptureReceipt {
    id: CaptureId,
    retained_bytes: usize,
    omitted_bytes: usize,
}

impl CaptureReceipt {
    pub fn id(&self) -> &CaptureId {
        &self.id
    }

    pub fn notice(&self) -> NonEmptyString {
        NonEmptyString(format!(
            "Captured output {}. Retained {} bytes; {} bytes omitted. Query with read_output. Session-local, bounded retention.\n",
            self.id, self.retained_bytes, self.omitted_bytes,
        ))
    }
}

struct CapturedText {
    receipt: CaptureReceipt,
    text: Box<str>,
}

#[derive(Default)]
pub struct CapturedOutputStore {
    captures: VecDeque<CapturedText>,
    retained_bytes: usize,
}

#[derive(Debug)]
pub struct CaptureUnavailable;

impl fmt::Display for CaptureUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("capture expired or belongs to another session; use a retained output_id")
    }
}

impl std::error::Error for CaptureUnavailable {}

impl CapturedOutputStore {
    pub fn insert(
        &mut self,
        id: CaptureId,
        mut text: String,
        source_omitted_bytes: usize,
    ) -> CaptureReceipt {
        let original_bytes = text.len();
        text.truncate(text.floor_char_boundary(MAX_CAPTURE_BYTES.min(text.len())));
        while self.captures.len() >= MAX_CAPTURES
            || self.retained_bytes.saturating_add(text.len()) > MAX_SESSION_BYTES
        {
            match self.captures.pop_front() {
                Some(expired) => self.retained_bytes -= expired.text.len(),
                None => break,
            }
        }
        let retained_bytes = text.len();
        let omitted_bytes = source_omitted_bytes.saturating_add(original_bytes - retained_bytes);
        self.retained_bytes += retained_bytes;
        self.captures.push_back(CapturedText {
            receipt: CaptureReceipt {
                id: id.clone(),
                retained_bytes,
                omitted_bytes,
            },
            text: text.into_boxed_str(),
        });
        CaptureReceipt {
            id,
            retained_bytes,
            omitted_bytes,
        }
    }

    pub fn read(
        &self,
        id: &CaptureId,
        query: CaptureQuery,
    ) -> Result<NonEmptyString, CaptureUnavailable> {
        self.captures
            .iter()
            .find(|capture| &capture.receipt.id == id)
            .ok_or(CaptureUnavailable)
            .map(|capture| capture.read(query))
    }
}

impl CapturedText {
    fn read(&self, query: CaptureQuery) -> NonEmptyString {
        let mut result = self.receipt.notice().0;
        match query {
            CaptureQuery::Range(range) => {
                let start = self
                    .text
                    .ceil_char_boundary(range.start.min(self.text.len()));
                let end = self
                    .text
                    .floor_char_boundary(range.end.min(self.text.len()))
                    .max(start);
                result.push_str(&format!("Bytes {start}..{end}\n"));
                result.push_str(&self.text[start..end]);
            }
            CaptureQuery::Search(needle) => {
                result.push_str("Up to 20 bounded literal matches follow.\n");
                for (offset, matched) in self.text.match_indices(&*needle.0).take(20) {
                    let start = self.text.floor_char_boundary(offset.saturating_sub(100));
                    let end = self
                        .text
                        .ceil_char_boundary((offset + matched.len() + 100).min(self.text.len()));
                    let excerpt = format!("Bytes {start}..{end}\n{}\n", &self.text[start..end]);
                    if result.len() + excerpt.len() > MAX_QUERY_BYTES {
                        break;
                    }
                    result.push_str(&excerpt);
                }
            }
        }
        NonEmptyString(result)
    }
}

#[cfg(test)]
#[path = "captured_output_tests.rs"]
mod tests;
