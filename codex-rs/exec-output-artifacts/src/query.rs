use crate::ArtifactDescriptor;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use regex::RegexBuilder;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::ArtifactError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactQuery {
    Metadata,
    Head {
        max_bytes: usize,
    },
    Tail {
        max_bytes: usize,
    },
    Bytes {
        start: usize,
        length: usize,
    },
    Lines {
        start: usize,
        end: usize,
        max_bytes: usize,
    },
    Search {
        pattern: String,
        mode: ArtifactSearchMode,
        case_sensitive: bool,
        context_lines: usize,
        max_matches: usize,
        max_bytes: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactSearchMode {
    Literal,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactQueryPresentation {
    scope: String,
    repeated_slice: RepeatedSlicePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepeatedSlicePolicy {
    ReturnReceipt,
    IncludeData,
}

impl ArtifactQueryPresentation {
    pub fn return_receipt(scope: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            repeated_slice: RepeatedSlicePolicy::ReturnReceipt,
        }
    }

    pub fn include_data(scope: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            repeated_slice: RepeatedSlicePolicy::IncludeData,
        }
    }

    pub(crate) fn scope(&self) -> &str {
        &self.scope
    }

    pub(crate) fn repeated_slice(&self) -> RepeatedSlicePolicy {
        self.repeated_slice
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArtifactQueryResult {
    pub descriptor: ArtifactDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ArtifactQueryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slice_sha256: Option<String>,
    pub repeated_slice: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArtifactQueryData {
    Text {
        text: String,
        byte_start: u64,
        byte_end: u64,
        line_start: Option<u64>,
        line_end: Option<u64>,
        truncated: bool,
    },
    Bytes {
        data_base64: String,
        byte_start: u64,
        byte_end: u64,
        truncated: bool,
    },
    Matches {
        matches: Vec<ArtifactSearchMatch>,
        truncated: bool,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArtifactSearchMatch {
    pub line: u64,
    pub byte_start: u64,
    pub byte_end: u64,
    pub text: String,
}

pub(crate) struct AppliedQuery {
    pub data: Option<ArtifactQueryData>,
    pub slice_sha256: Option<String>,
}

impl ArtifactQuery {
    pub(crate) fn requires_complete_tail(&self) -> bool {
        matches!(self, Self::Tail { .. })
    }
}

pub(crate) fn apply_query(
    content: &[u8],
    query: &ArtifactQuery,
    hard_max_bytes: usize,
) -> Result<AppliedQuery, ArtifactError> {
    match query {
        ArtifactQuery::Metadata => Ok(AppliedQuery {
            data: None,
            slice_sha256: None,
        }),
        ArtifactQuery::Head { max_bytes } => {
            let max_bytes = validate_max_bytes(*max_bytes, hard_max_bytes)?;
            let end = utf8_prefix_boundary(content, max_bytes.min(content.len()));
            text_slice(content, 0, end, end < content.len())
        }
        ArtifactQuery::Tail { max_bytes } => {
            let max_bytes = validate_max_bytes(*max_bytes, hard_max_bytes)?;
            let requested_start = content.len().saturating_sub(max_bytes);
            let start = utf8_suffix_boundary(content, requested_start);
            text_slice(content, start, content.len(), start > 0)
        }
        ArtifactQuery::Bytes { start, length } => {
            let length = validate_max_bytes(*length, hard_max_bytes)?;
            if *start > content.len() {
                return Err(ArtifactError::InvalidQuery(
                    "byte range starts after the artifact".to_string(),
                ));
            }
            let end = start.saturating_add(length).min(content.len());
            let selected = &content[*start..end];
            let data = ArtifactQueryData::Bytes {
                data_base64: STANDARD.encode(selected),
                byte_start: u64::try_from(*start).unwrap_or(u64::MAX),
                byte_end: u64::try_from(end).unwrap_or(u64::MAX),
                truncated: end < content.len(),
            };
            applied_data(data)
        }
        ArtifactQuery::Lines {
            start,
            end,
            max_bytes,
        } => {
            let max_bytes = validate_max_bytes(*max_bytes, hard_max_bytes)?;
            if *start == 0 || *end < *start {
                return Err(ArtifactError::InvalidQuery(
                    "line ranges are one-based and require end >= start".to_string(),
                ));
            }
            let Some((first, requested_end, total_lines)) = line_range(content, *start, *end)
            else {
                return Err(ArtifactError::InvalidQuery(
                    "line range starts after the artifact".to_string(),
                ));
            };
            let capped_end =
                first + utf8_prefix_boundary(&content[first..requested_end], max_bytes);
            let truncated = capped_end < requested_end || *end < total_lines;
            text_slice_with_lines(
                content,
                first,
                capped_end,
                Some(*start),
                line_number_at_end_offset(content, capped_end),
                truncated,
            )
        }
        ArtifactQuery::Search {
            pattern,
            mode,
            case_sensitive,
            context_lines,
            max_matches,
            max_bytes,
        } => apply_search(
            content,
            SearchOptions {
                pattern,
                mode: *mode,
                case_sensitive: *case_sensitive,
                context_lines: *context_lines,
                max_matches: *max_matches,
                max_bytes: validate_max_bytes(*max_bytes, hard_max_bytes)?,
            },
        ),
    }
}

fn validate_max_bytes(requested: usize, hard_max: usize) -> Result<usize, ArtifactError> {
    if requested == 0 || requested > hard_max {
        return Err(ArtifactError::InvalidQuery(format!(
            "requested output bytes must be in 1..={hard_max}"
        )));
    }
    Ok(requested)
}

fn text_slice(
    content: &[u8],
    start: usize,
    end: usize,
    truncated: bool,
) -> Result<AppliedQuery, ArtifactError> {
    text_slice_with_lines(
        content,
        start,
        end,
        line_number_at_start_offset(content, start),
        line_number_at_end_offset(content, end),
        truncated,
    )
}

fn text_slice_with_lines(
    content: &[u8],
    start: usize,
    end: usize,
    line_start: Option<usize>,
    line_end: Option<usize>,
    truncated: bool,
) -> Result<AppliedQuery, ArtifactError> {
    let text = std::str::from_utf8(&content[start..end])
        .map_err(|_| ArtifactError::Corrupt)?
        .to_string();
    let data = ArtifactQueryData::Text {
        text,
        byte_start: u64::try_from(start).unwrap_or(u64::MAX),
        byte_end: u64::try_from(end).unwrap_or(u64::MAX),
        line_start: line_start.map(|line| u64::try_from(line).unwrap_or(u64::MAX)),
        line_end: line_end.map(|line| u64::try_from(line).unwrap_or(u64::MAX)),
        truncated,
    };
    applied_data(data)
}

fn applied_data(data: ArtifactQueryData) -> Result<AppliedQuery, ArtifactError> {
    let serialized = serde_json::to_vec(&data)?;
    let digest = format!("{:x}", Sha256::digest(&serialized));
    Ok(AppliedQuery {
        data: Some(data),
        slice_sha256: Some(digest),
    })
}

struct SearchOptions<'a> {
    pattern: &'a str,
    mode: ArtifactSearchMode,
    case_sensitive: bool,
    context_lines: usize,
    max_matches: usize,
    max_bytes: usize,
}

fn apply_search(content: &[u8], options: SearchOptions<'_>) -> Result<AppliedQuery, ArtifactError> {
    if options.pattern.is_empty() || options.pattern.len() > 1024 {
        return Err(ArtifactError::InvalidQuery(
            "search patterns must contain 1..=1024 bytes".to_string(),
        ));
    }
    if options.context_lines > 10 || options.max_matches == 0 || options.max_matches > 100 {
        return Err(ArtifactError::InvalidQuery(
            "search requires context_lines <= 10 and max_matches in 1..=100".to_string(),
        ));
    }
    let expression = match options.mode {
        ArtifactSearchMode::Literal => regex::escape(options.pattern),
        ArtifactSearchMode::Regex => options.pattern.to_string(),
    };
    let matcher = RegexBuilder::new(&expression)
        .case_insensitive(!options.case_sensitive)
        .size_limit(1 << 20)
        .build()
        .map_err(|err| ArtifactError::InvalidQuery(format!("invalid regular expression: {err}")))?;
    let text = std::str::from_utf8(content).map_err(|_| ArtifactError::Corrupt)?;
    let mut ranges = Vec::new();
    let mut match_limit_reached = false;

    for (index, line) in text.split_inclusive('\n').enumerate() {
        if matcher.is_match(line) {
            if ranges.len() == options.max_matches {
                match_limit_reached = true;
                break;
            }
            ranges.push(SearchRange {
                match_line: index + 1,
                context_start_line: index.saturating_sub(options.context_lines) + 1,
                context_end_line: index + options.context_lines + 1,
                byte_start: 0,
                byte_end: 0,
            });
        }
    }

    populate_search_byte_ranges(text, &mut ranges);
    let mut matches = Vec::new();
    let mut presented_bytes = 0usize;
    let mut truncated = match_limit_reached;

    for range in ranges {
        let mut range_truncated = false;
        let mut rendered = text[range.byte_start..range.byte_end].to_string();
        let remaining = options.max_bytes.saturating_sub(presented_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        if rendered.len() > remaining {
            rendered.truncate(utf8_prefix_boundary(rendered.as_bytes(), remaining));
            truncated = true;
            range_truncated = true;
        }
        if rendered.is_empty() {
            truncated = true;
            break;
        }
        presented_bytes = presented_bytes.saturating_add(rendered.len());
        matches.push(ArtifactSearchMatch {
            line: u64::try_from(range.match_line).unwrap_or(u64::MAX),
            byte_start: u64::try_from(range.byte_start).unwrap_or(u64::MAX),
            byte_end: u64::try_from(range.byte_start.saturating_add(rendered.len()))
                .unwrap_or(u64::MAX),
            text: rendered,
        });
        if range_truncated {
            break;
        }
    }

    applied_data(ArtifactQueryData::Matches { matches, truncated })
}

struct SearchRange {
    match_line: usize,
    context_start_line: usize,
    context_end_line: usize,
    byte_start: usize,
    byte_end: usize,
}

fn populate_search_byte_ranges(text: &str, ranges: &mut [SearchRange]) {
    let mut next_start = 0usize;
    let mut next_end = 0usize;
    let mut byte_start = 0usize;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let byte_end = byte_start.saturating_add(line.len());
        while ranges
            .get(next_start)
            .is_some_and(|range| range.context_start_line == line_number)
        {
            ranges[next_start].byte_start = byte_start;
            next_start += 1;
        }
        while ranges
            .get(next_end)
            .is_some_and(|range| range.context_end_line <= line_number)
        {
            ranges[next_end].byte_end = byte_end;
            next_end += 1;
        }
        byte_start = byte_end;
    }
    for range in ranges.iter_mut().skip(next_end) {
        range.byte_end = text.len();
    }
}

fn line_range(content: &[u8], start: usize, end: usize) -> Option<(usize, usize, usize)> {
    if content.is_empty() {
        return None;
    }
    let mut current_line = 1usize;
    let mut first = (start == 1).then_some(0);
    let mut requested_end = None;
    for (index, byte) in content.iter().enumerate() {
        if *byte == b'\n' {
            if current_line == end {
                requested_end = Some(index + 1);
            }
            current_line += 1;
            if current_line == start && index + 1 < content.len() {
                first = Some(index + 1);
            }
        }
    }
    let total_lines = current_line.saturating_sub(usize::from(content.ends_with(b"\n")));
    let first = first.filter(|_| start <= total_lines)?;
    Some((first, requested_end.unwrap_or(content.len()), total_lines))
}

fn line_number_at_start_offset(content: &[u8], offset: usize) -> Option<usize> {
    (!content.is_empty()).then(|| {
        content[..offset.min(content.len())]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            .saturating_add(1)
    })
}

fn line_number_at_end_offset(content: &[u8], offset: usize) -> Option<usize> {
    if content.is_empty() {
        return None;
    }
    let before_last_selected_byte = offset.saturating_sub(1).min(content.len());
    Some(
        content[..before_last_selected_byte]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            .saturating_add(1),
    )
}

fn utf8_prefix_boundary(bytes: &[u8], mut end: usize) -> usize {
    end = end.min(bytes.len());
    while end > 0 && std::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }
    end
}

fn utf8_suffix_boundary(bytes: &[u8], mut start: usize) -> usize {
    start = start.min(bytes.len());
    while start < bytes.len() && std::str::from_utf8(&bytes[start..]).is_err() {
        start += 1;
    }
    start
}
