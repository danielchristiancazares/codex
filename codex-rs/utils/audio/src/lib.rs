//! Audio preparation and duration-based token estimates for model inputs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::read::DecoderReader;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::MAX_PROMPT_AUDIO_INPUT_BYTES;
use codex_protocol::models::ResponseItem;
use codex_utils_cache::BlockingLruCache;
use codex_utils_cache::blake3_digest;
use codex_utils_string::approx_token_count;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::num::NonZeroUsize;
use std::sync::LazyLock;
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use tracing::warn;

const AUDIO_PROCESSING_ERROR_PLACEHOLDER: &str =
    "audio content omitted because it could not be processed";
const AUDIO_TOO_LARGE_PLACEHOLDER: &str =
    "audio content omitted because it exceeded the supported size limit; use a smaller audio file";
const UNSUPPORTED_AUDIO_FORMAT_PLACEHOLDER: &str =
    "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg";

const MAX_PROMPT_AUDIO_BASE64_BYTES: usize = MAX_PROMPT_AUDIO_INPUT_BYTES.div_ceil(3) * 4;
const AUDIO_TOKEN_ESTIMATE_CACHE_SIZE: usize = 32;
const AUDIO_TOKENS_PER_SECOND: f64 = 10.0;
const SMALL_AUDIO_TOKEN_ESTIMATE_CACHE_MAX_BYTES: usize = 16 * 1024;
const WAV_HEADER_PREFIX_BYTES: [usize; 4] = [256, 4 * 1024, 64 * 1024, 256 * 1024];

static AUDIO_TOKEN_ESTIMATE_CACHE: LazyLock<BlockingLruCache<[u8; 32], usize>> =
    LazyLock::new(|| {
        BlockingLruCache::new(
            NonZeroUsize::new(AUDIO_TOKEN_ESTIMATE_CACHE_SIZE).unwrap_or(NonZeroUsize::MIN),
        )
    });

#[derive(Debug, thiserror::Error)]
enum AudioPreparationError {
    #[error("invalid audio data URL: {reason}")]
    InvalidDataUrl { reason: &'static str },
    #[error("unsupported audio format")]
    UnsupportedFormat,
    #[error("audio input is too large ({size} bytes; max {MAX_PROMPT_AUDIO_INPUT_BYTES} bytes)")]
    AudioTooLarge { size: usize },
}

impl AudioPreparationError {
    fn placeholder(&self) -> &'static str {
        match self {
            AudioPreparationError::InvalidDataUrl { .. } => AUDIO_PROCESSING_ERROR_PLACEHOLDER,
            AudioPreparationError::UnsupportedFormat => UNSUPPORTED_AUDIO_FORMAT_PLACEHOLDER,
            AudioPreparationError::AudioTooLarge { .. } => AUDIO_TOO_LARGE_PLACEHOLDER,
        }
    }
}

/// Canonicalizes audio inputs and replaces unsupported inputs with text placeholders.
pub fn prepare_response_items(items: &mut [ResponseItem]) {
    for item in items {
        match item {
            ResponseItem::Message { content, .. } => prepare_message_content(content),
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items_mut() {
                    prepare_tool_output_content(content);
                }
            }
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }
}

fn prepare_message_content(items: &mut [ContentItem]) {
    for item in items {
        if let ContentItem::InputAudio { audio_url } = item
            && let Err(error) = prepare_audio(audio_url)
        {
            warn!(%error, "failed to prepare message audio");
            *item = ContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn prepare_tool_output_content(items: &mut [FunctionCallOutputContentItem]) {
    for item in items {
        if let FunctionCallOutputContentItem::InputAudio { audio_url } = item
            && let Err(error) = prepare_audio(audio_url)
        {
            warn!(%error, "failed to prepare tool output audio");
            *item = FunctionCallOutputContentItem::InputText {
                text: error.placeholder().to_string(),
            };
        }
    }
}

fn is_data_url(audio_url: &str) -> bool {
    audio_url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn canonical_audio_mime(mime: &str) -> Option<&'static str> {
    if mime.eq_ignore_ascii_case("audio/wav")
        || mime.eq_ignore_ascii_case("audio/x-wav")
        || mime.eq_ignore_ascii_case("audio/wave")
        || mime.eq_ignore_ascii_case("audio/vnd.wave")
    {
        Some("audio/wav")
    } else if mime.eq_ignore_ascii_case("audio/mpeg") || mime.eq_ignore_ascii_case("audio/mp3") {
        Some("audio/mpeg")
    } else if mime.eq_ignore_ascii_case("audio/mp4")
        || mime.eq_ignore_ascii_case("audio/m4a")
        || mime.eq_ignore_ascii_case("audio/x-m4a")
    {
        Some("audio/mp4")
    } else if mime.eq_ignore_ascii_case("audio/webm") {
        Some("audio/webm")
    } else if mime.eq_ignore_ascii_case("audio/ogg") {
        Some("audio/ogg")
    } else {
        None
    }
}

/// Estimates audio tokens from decoded duration, falling back to the data URL size.
pub fn estimate_audio_token_count(audio_url: &str) -> usize {
    let parsed = parse_base64_audio_data_url(audio_url);
    if audio_url.len() <= SMALL_AUDIO_TOKEN_ESTIMATE_CACHE_MAX_BYTES {
        let key = blake3_digest(audio_url.as_bytes());
        return AUDIO_TOKEN_ESTIMATE_CACHE.get_or_insert_with(key, || {
            if let Some(("audio/wav", payload)) = parsed
                && let Some(duration_seconds) = wav_duration_seconds_from_base64(payload)
            {
                return audio_tokens_for_duration(duration_seconds);
            }
            estimate_audio_token_count_with_duration_probe(audio_url, parsed)
        });
    }

    if let Some(("audio/wav", payload)) = parsed
        && let Some(duration_seconds) = wav_duration_seconds_from_base64(payload)
    {
        return audio_tokens_for_duration(duration_seconds);
    }

    let key = blake3_digest(audio_url.as_bytes());
    AUDIO_TOKEN_ESTIMATE_CACHE.get_or_insert_with(key, || {
        estimate_audio_token_count_with_duration_probe(audio_url, parsed)
    })
}

fn estimate_audio_token_count_with_duration_probe(
    audio_url: &str,
    parsed: Option<(&str, &str)>,
) -> usize {
    let Some((canonical_mime, payload)) = parsed else {
        return approx_token_count(audio_url);
    };
    let Some(duration_seconds) = audio_duration_seconds(canonical_mime, payload) else {
        return approx_token_count(audio_url);
    };
    audio_tokens_for_duration(duration_seconds)
}

fn parse_base64_audio_data_url(audio_url: &str) -> Option<(&'static str, &str)> {
    let (metadata, payload) = audio_url.split_once(',')?;
    let metadata = metadata.get("data:".len()..)?;
    let mut metadata_parts = metadata.split(';');
    let canonical_mime = canonical_audio_mime(metadata_parts.next()?)?;
    if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return None;
    }
    Some((canonical_mime, payload))
}

fn audio_tokens_for_duration(duration_seconds: f64) -> usize {
    let token_count = (duration_seconds * AUDIO_TOKENS_PER_SECOND).ceil();
    if token_count >= usize::MAX as f64 {
        usize::MAX
    } else {
        token_count as usize
    }
}

fn wav_duration_seconds_from_base64(payload: &str) -> Option<f64> {
    let mut decoder = DecoderReader::new(payload.as_bytes(), &BASE64_STANDARD);
    let mut header = Vec::with_capacity(WAV_HEADER_PREFIX_BYTES[0]);
    for target_len in WAV_HEADER_PREFIX_BYTES {
        let remaining = target_len.saturating_sub(header.len());
        decoder
            .by_ref()
            .take(remaining as u64)
            .read_to_end(&mut header)
            .ok()?;
        if let Some(duration_seconds) = pcm_wav_duration_seconds(&header) {
            return Some(duration_seconds);
        }
        if header.len() < target_len {
            return None;
        }
    }
    None
}

fn pcm_wav_duration_seconds(header: &[u8]) -> Option<f64> {
    if header.get(..4)? != b"RIFF" || header.get(8..12)? != b"WAVE" {
        return None;
    }

    let mut byte_rate = None;
    let mut offset = 12usize;
    while offset.checked_add(8)? <= header.len() {
        let chunk_id = header.get(offset..offset + 4)?;
        let chunk_size =
            u32::from_le_bytes(header.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        let data_offset = offset.checked_add(8)?;
        if chunk_id == b"fmt " {
            let format =
                u16::from_le_bytes(header.get(data_offset..data_offset + 2)?.try_into().ok()?);
            if !matches!(format, 1 | 3) {
                return None;
            }
            byte_rate = Some(u32::from_le_bytes(
                header
                    .get(data_offset + 8..data_offset + 12)?
                    .try_into()
                    .ok()?,
            ));
        } else if chunk_id == b"data" {
            let byte_rate = byte_rate?;
            if byte_rate == 0 {
                return None;
            }
            let duration_seconds = chunk_size as f64 / f64::from(byte_rate);
            return duration_seconds.is_finite().then_some(duration_seconds);
        }
        offset = data_offset
            .checked_add(chunk_size)?
            .checked_add(chunk_size % 2)?;
    }
    None
}

fn audio_duration_seconds(canonical_mime: &str, payload: &str) -> Option<f64> {
    let bytes = match BASE64_STANDARD.decode(payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::trace!(%error, "failed to decode audio payload for token estimation");
            return None;
        }
    };
    let media_source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.mime_type(canonical_mime);
    let format = match symphonia::default::get_probe().probe(
        &hint,
        media_source,
        FormatOptions::default(),
        MetadataOptions::default(),
    ) {
        Ok(format) => format,
        Err(error) => {
            tracing::trace!(%error, "failed to read audio duration for token estimation");
            return None;
        }
    };
    let track = format.default_track(TrackType::Audio)?;
    let timing = track.time_base.zip(track.duration).or_else(|| {
        format
            .media_info()
            .time_base
            .zip(format.media_info().duration)
    });
    let (time_base, duration) = timing?;
    let duration_seconds =
        duration.get() as f64 * f64::from(time_base.numer.get()) / f64::from(time_base.denom.get());
    duration_seconds.is_finite().then_some(duration_seconds)
}

fn prepare_audio(audio_url: &mut String) -> Result<(), AudioPreparationError> {
    if !is_data_url(audio_url) {
        return Err(AudioPreparationError::InvalidDataUrl {
            reason: "audio input must be a data URL",
        });
    }

    let comma_index = audio_url
        .find(',')
        .ok_or(AudioPreparationError::InvalidDataUrl {
            reason: "missing payload separator",
        })?;
    let payload_start = comma_index + 1;
    let metadata = &audio_url[..comma_index];
    let payload = &audio_url[payload_start..];
    let metadata = metadata
        .get("data:".len()..)
        .ok_or(AudioPreparationError::InvalidDataUrl {
            reason: "missing data URL prefix",
        })?;
    let mut metadata_parts = metadata.split(';');
    let mime = metadata_parts
        .next()
        .filter(|mime| !mime.is_empty())
        .ok_or(AudioPreparationError::InvalidDataUrl {
            reason: "missing media type",
        })?;
    let canonical_mime =
        canonical_audio_mime(mime).ok_or(AudioPreparationError::UnsupportedFormat)?;
    if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return Err(AudioPreparationError::InvalidDataUrl {
            reason: "audio payload is not base64 encoded",
        });
    }
    if payload.len() > MAX_PROMPT_AUDIO_BASE64_BYTES {
        return Err(AudioPreparationError::AudioTooLarge {
            size: payload.len(),
        });
    }

    let mut decoder = DecoderReader::new(payload.as_bytes(), &BASE64_STANDARD);
    let decoded_len = io::copy(&mut decoder, &mut io::sink()).map_err(|_| {
        AudioPreparationError::InvalidDataUrl {
            reason: "invalid base64 payload",
        }
    })?;
    if decoded_len > MAX_PROMPT_AUDIO_INPUT_BYTES as u64 {
        return Err(AudioPreparationError::AudioTooLarge {
            size: usize::try_from(decoded_len).unwrap_or(usize::MAX),
        });
    }

    let canonical_prefix = format!("data:{canonical_mime};base64,");
    if audio_url.get(..payload_start) != Some(canonical_prefix.as_str()) {
        audio_url.replace_range(..payload_start, &canonical_prefix);
    }
    Ok(())
}

#[cfg(test)]
#[path = "audio_preparation_tests.rs"]
mod tests;
