use super::*;
use crate::context_manager::ContextManager;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

const OUTPUT_LIMIT: usize = 10_000;

#[test]
fn zero_and_sub_framing_budgets_keep_only_empty_wire_values() {
    for body in [
        FunctionCallOutputBody::Text("content".to_string()),
        FunctionCallOutputBody::ContentItems(vec![FunctionCallOutputContentItem::InputText {
            text: "content".to_string(),
        }]),
    ] {
        let empty = match &body {
            FunctionCallOutputBody::Text(_) => FunctionCallOutputBody::Text(String::new()),
            FunctionCallOutputBody::ContentItems(_) => {
                FunctionCallOutputBody::ContentItems(Vec::new())
            }
        };
        let payload = FunctionCallOutputPayload {
            body,
            success: Some(false),
        };
        for (policy, saved_limit) in [
            (TruncationPolicy::Tokens(0), 10_000),
            (TruncationPolicy::Bytes(0), 10_000),
            (TruncationPolicy::Bytes(1), 10_000),
            (TruncationPolicy::Bytes(256), 0),
        ] {
            assert_eq!(
                truncate_function_output_payload_with_token_limit(&payload, policy, saved_limit),
                FunctionCallOutputPayload {
                    body: empty.clone(),
                    success: Some(false)
                },
            );
        }
    }
}

#[test]
fn complete_tool_items_reserve_envelope_metadata_within_the_hard_ceiling() {
    let output = FunctionCallOutputPayload::from_text("!@#$%^&*()[]{}".repeat(4_000));
    let call_id = format!("call-{}", "!@#$".repeat(128));
    let items = [
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id.clone()),
            name: Some("tool".to_string()),
            namespace: Some("namespace".to_string()),
            output: output.clone(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCallOutput {
            id: None,
            call_id,
            name: Some("tool".to_string()),
            output,
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    for original in items {
        let mut history = ContextManager::new();
        history.record_items(
            std::slice::from_ref(&original),
            TruncationPolicy::Tokens(50_000),
        );
        let item = &history.annotated_items()[0].item;
        let serialized = serde_json::to_string(item).expect("serialize complete tool item");
        assert!(
            FUNCTION_OUTPUT_TOKENIZER
                .as_ref()
                .expect("output tokenizer")
                .count_ordinary(&serialized)
                <= OUTPUT_LIMIT
        );
        let actual = serde_json::to_value(item).expect("actual item");
        let mut expected = serde_json::to_value(original).expect("original item");
        expected["output"] = actual["output"].clone();
        assert_eq!(actual, expected);
    }
}

#[test]
fn saved_tool_limits_respect_model_policy_and_the_hard_ceiling() {
    let payload = FunctionCallOutputPayload::from_text("!@#$%^&*()[]{}".repeat(4_000));
    for (policy, tool_limit, expected_tokens) in [
        (TruncationPolicy::Tokens(128), 64, 64),
        (TruncationPolicy::Tokens(64), 128, 64),
        (TruncationPolicy::Tokens(50_000), 30_000, 10_000),
        (TruncationPolicy::Bytes(200_000), 30_000, 10_000),
        (TruncationPolicy::Bytes(600), 128, 128),
    ] {
        let output =
            truncate_function_output_payload_with_token_limit(&payload, policy, tool_limit);
        assert_ne!(output, payload);
        assert_payload_fits(&output, policy);
        assert_payload_fits(&output, TruncationPolicy::Tokens(expected_tokens));
    }
}

#[test]
fn mixed_currency_budgets_bound_dense_text_and_structured_payloads() {
    let payloads = [
        FunctionCallOutputPayload::from_text("word !@#$ é🙂\n".repeat(100)),
        FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputText {
                text: "word !@#$ é🙂\n".repeat(100),
            },
            FunctionCallOutputContentItem::InputText {
                text: "tail".repeat(100),
            },
        ]),
    ];
    for bytes in [32, 64, 96, 256, 1_000] {
        for tokens in [8, 16, 24, 64] {
            for payload in &payloads {
                let output = truncate_function_output_payload_with_token_limit(
                    payload,
                    TruncationPolicy::Bytes(bytes),
                    tokens,
                );
                assert_payload_fits(&output, TruncationPolicy::Bytes(bytes));
                assert_payload_fits(&output, TruncationPolicy::Tokens(tokens));
            }
        }
    }
}

#[test]
fn saved_limits_survive_projection_and_match_the_model_visible_delta() {
    use codex_history::CodexHarnessMetadata;
    use codex_history::ResponseItemEnvelope;
    use codex_protocol::openai_models::InputModality;

    let raw = ResponseItemEnvelope {
        item: ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some("audio_tool".to_string()),
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(
                (0..100)
                    .map(|_| FunctionCallOutputContentItem::InputAudio {
                        audio_url: "data:audio/wav;base64,".to_string(),
                    })
                    .collect(),
            ),
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata {
            fallback_token_limit_override: Some(64),
            ..Default::default()
        }),
    };
    let mut history = ContextManager::new();
    history.replace_annotated(vec![raw.clone()]);
    let policy = TruncationPolicy::Tokens(256);
    let delta = history.model_visible_token_delta(&[InputModality::Text], policy);
    let projected = history.for_prompt_annotated_with_policy(&[InputModality::Text], policy);
    assert_eq!(projected[0].metadata, raw.metadata);
    let ResponseItem::FunctionCallOutput { output, .. } = &projected[0].item else {
        panic!("expected function output");
    };
    assert_payload_fits(output, TruncationPolicy::Tokens(64));
    assert_eq!(
        delta,
        crate::context_manager::estimate_item_token_count(&projected[0].item)
            - crate::context_manager::estimate_item_token_count(&raw.item),
    );
}

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
