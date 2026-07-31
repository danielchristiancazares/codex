use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

use super::*;

fn pcm_wav_payload(sample_count: u32, prefix_chunk_bytes: usize) -> String {
    let padding = sample_count % 2;
    let prefix_padding = prefix_chunk_bytes % 2;
    let riff_size = 36usize
        .saturating_add(sample_count as usize)
        .saturating_add(padding as usize)
        .saturating_add(if prefix_chunk_bytes == 0 {
            0
        } else {
            8 + prefix_chunk_bytes + prefix_padding
        });
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(riff_size as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    if prefix_chunk_bytes != 0 {
        bytes.extend_from_slice(b"JUNK");
        bytes.extend_from_slice(&(prefix_chunk_bytes as u32).to_le_bytes());
        bytes.resize(bytes.len() + prefix_chunk_bytes + prefix_padding, 0);
    }
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&sample_count.to_le_bytes());
    bytes.resize(
        bytes.len() + sample_count as usize + padding as usize,
        /*value*/ 0,
    );
    BASE64_STANDARD.encode(bytes)
}

#[test]
fn reads_pcm_wav_duration_from_progressive_base64_prefix() {
    let payload = pcm_wav_payload(
        /*sample_count*/ 16_000, /*prefix_chunk_bytes*/ 8_192,
    );

    assert_eq!(wav_duration_seconds_from_base64(&payload), Some(2.0));
}

#[test]
fn estimates_large_pcm_wav_without_hashing_or_full_decode() {
    let payload = pcm_wav_payload(
        /*sample_count*/ 5 * 1024 * 1024,
        /*prefix_chunk_bytes*/ 0,
    );
    let audio_url = format!("data:audio/wav;base64,{payload}");

    assert_eq!(estimate_audio_token_count(&audio_url), 6_554);
}

#[test]
fn preparation_canonicalizes_data_urls_and_rejects_remote_urls() {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputAudio {
                audio_url: "data:audio/x-wav;base64,YXVkaW8=".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "data:audio/ogg;base64,YXVkaW8=".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "https://example.com/audio.mp3".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                ContentItem::InputAudio {
                    audio_url: "data:audio/ogg;base64,YXVkaW8=".to_string(),
                },
                ContentItem::InputText {
                    text: "audio content omitted because it could not be processed".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_replaces_invalid_message_audio_with_placeholders() {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputAudio {
                audio_url: "data:audio/wav;base64,%%%".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "data:audio/flac;base64,YXVkaW8=".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "audio content omitted because it could not be processed".to_string(),
                },
                ContentItem::InputText {
                    text: "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_replaces_only_failed_tool_audio_and_preserves_metadata() {
    let mut items = vec![ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav,not-base64".to_string(),
                },
            ]),
            success: Some(true),
        },
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "before".to_string(),
                    },
                    FunctionCallOutputContentItem::InputAudio {
                        audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "audio content omitted because it could not be processed".to_string(),
                    },
                ]),
                success: Some(true),
            },
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_errors_map_to_expected_placeholders() {
    let cases = [
        (
            AudioPreparationError::InvalidDataUrl {
                reason: "details remain in logs",
            },
            "audio content omitted because it could not be processed",
        ),
        (
            AudioPreparationError::UnsupportedFormat,
            "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg",
        ),
        (
            AudioPreparationError::AudioTooLarge { size: usize::MAX },
            "audio content omitted because it exceeded the supported size limit; use a smaller audio file",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.placeholder(), expected);
    }
}
