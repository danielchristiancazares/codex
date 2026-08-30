use super::*;
use crate::context_manager::ContextManager;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

const OUTPUT_LIMIT: usize = 10_000;

fn assert_payload_fits(payload: &FunctionCallOutputPayload, policy: TruncationPolicy) {
    let budget = policy_budget(policy);
    assert!(function_output_payload_cost(payload, policy) <= budget);
    let serialized = serde_json::to_string(payload).expect("serialize payload");
    match policy {
        TruncationPolicy::Bytes(_) => assert!(serialized.len() <= budget),
        TruncationPolicy::Tokens(_) => {
            let tokens = FUNCTION_OUTPUT_TOKENIZER
                .as_ref()
                .map_or(serialized.len(), |tokenizer| {
                    tokenizer.count_ordinary(&serialized)
                });
            assert!(tokens <= budget);
        }
    }
}

#[test]
fn token_dense_text_fits_the_nominal_output_limit() {
    let payload = FunctionCallOutputPayload::from_text("!@#$%^&*()[]{}".repeat(4_000));

    let truncated =
        truncate_function_output_payload(&payload, TruncationPolicy::Tokens(OUTPUT_LIMIT));

    assert_ne!(truncated, payload);
    assert_payload_fits(&truncated, TruncationPolicy::Tokens(OUTPUT_LIMIT));
}

#[test]
fn high_cardinality_text_items_include_structural_cost() {
    let payload = FunctionCallOutputPayload::from_content_items(
        (0..12_000)
            .map(|_| FunctionCallOutputContentItem::InputText {
                text: "x".to_string(),
            })
            .collect(),
    );

    let truncated =
        truncate_function_output_payload(&payload, TruncationPolicy::Tokens(OUTPUT_LIMIT));

    assert_payload_fits(&truncated, TruncationPolicy::Tokens(OUTPUT_LIMIT));
    let FunctionCallOutputBody::ContentItems(items) = truncated.body else {
        panic!("expected content items");
    };
    assert!(items.len() < 12_000);
}

#[test]
fn empty_encrypted_items_have_nonzero_structural_cost() {
    let payload = FunctionCallOutputPayload::from_content_items(
        (0..12_000)
            .map(|_| FunctionCallOutputContentItem::EncryptedContent {
                encrypted_content: String::new(),
            })
            .collect(),
    );

    let truncated =
        truncate_function_output_payload(&payload, TruncationPolicy::Tokens(OUTPUT_LIMIT));

    assert_payload_fits(&truncated, TruncationPolicy::Tokens(OUTPUT_LIMIT));
    let FunctionCallOutputBody::ContentItems(items) = truncated.body else {
        panic!("expected content items");
    };
    assert!(items.len() < 12_000);
}

#[test]
fn zero_duration_audio_items_have_nonzero_structural_cost() {
    let payload = FunctionCallOutputPayload::from_content_items(
        (0..12_000)
            .map(|_| FunctionCallOutputContentItem::InputAudio {
                audio_url: "data:audio/wav;base64,".to_string(),
            })
            .collect(),
    );

    let truncated =
        truncate_function_output_payload(&payload, TruncationPolicy::Tokens(OUTPUT_LIMIT));

    assert_payload_fits(&truncated, TruncationPolicy::Tokens(OUTPUT_LIMIT));
    let FunctionCallOutputBody::ContentItems(items) = truncated.body else {
        panic!("expected content items");
    };
    assert!(items.len() < 12_000);
}

#[test]
fn omission_marker_participates_in_the_same_budget() {
    let image = FunctionCallOutputContentItem::InputImage {
        image_url: "data:image/png;base64,AA==".to_string(),
        detail: None,
    };
    let payload = FunctionCallOutputPayload::from_content_items(vec![
        image.clone(),
        image.clone(),
        image.clone(),
        image.clone(),
        image,
    ]);

    let truncated =
        truncate_function_output_payload(&payload, TruncationPolicy::Tokens(OUTPUT_LIMIT));

    assert_payload_fits(&truncated, TruncationPolicy::Tokens(OUTPUT_LIMIT));
    let FunctionCallOutputBody::ContentItems(items) = truncated.body else {
        panic!("expected content items");
    };
    assert_eq!(items.len(), 5);
    assert!(matches!(
        items.last(),
        Some(FunctionCallOutputContentItem::InputText { text })
            if text == "[omitted 1 image items to fit output budget]"
    ));
}

#[test]
fn byte_policy_charges_the_complete_serialized_media_item() {
    let payload = FunctionCallOutputPayload::from_content_items(vec![
        FunctionCallOutputContentItem::InputAudio {
            audio_url: format!("data:audio/wav;base64,{}", "A".repeat(400)),
        },
    ]);

    let policy = TruncationPolicy::Bytes(200);
    let truncated = truncate_function_output_payload(&payload, policy);

    assert_payload_fits(&truncated, policy);
    assert_eq!(
        truncated,
        FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputText {
                text: "[omitted 1 audio items to fit output budget]".to_string(),
            },
        ])
    );
}

#[test]
fn unsupported_audio_projection_is_finalized_again() {
    let output = FunctionCallOutputPayload::from_content_items(
        (0..12_000)
            .map(|_| FunctionCallOutputContentItem::InputAudio {
                audio_url: "data:audio/wav;base64,".to_string(),
            })
            .collect(),
    );
    let item = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: None,
        name: Some("audio_tool".to_string()),
        namespace: None,
        output,
        internal_chat_message_metadata_passthrough: None,
    };
    let mut history = ContextManager::new();
    history.record_items(
        std::slice::from_ref(&item),
        TruncationPolicy::Tokens(OUTPUT_LIMIT),
    );

    let projected = history.for_prompt_with_policy(
        &[codex_protocol::openai_models::InputModality::Text],
        TruncationPolicy::Tokens(OUTPUT_LIMIT),
    );

    let [ResponseItem::FunctionCallOutput { output, .. }] = projected.as_slice() else {
        panic!("expected one function output");
    };
    assert_payload_fits(output, TruncationPolicy::Tokens(OUTPUT_LIMIT));
    assert_eq!(
        output
            .content_items()
            .expect("content items")
            .iter()
            .filter(|item| matches!(item, FunctionCallOutputContentItem::InputAudio { .. }))
            .count(),
        0
    );
}
