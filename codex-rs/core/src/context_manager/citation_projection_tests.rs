use crate::context_manager::history::ContextManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::default_input_modalities;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn cf_049_prompt_and_compaction_projection_strip_citations_while_raw_history_retains_them() {
    let raw = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "before<oai-mem-citation><memory_citation>source</memory_citation></oai-mem-citation>after"
                .to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: Some(
            InternalChatMessageMetadataPassthrough {
                content_item_kinds: Some(vec![ContentItemKind("unknown".to_string())]),
                ..Default::default()
            },
        ),
    };
    let expected = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "beforeafter".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            content_item_kinds: Some(vec![ContentItemKind("unknown".to_string())]),
            ..Default::default()
        }),
    };
    let mut history = ContextManager::new();
    history.record_items(std::slice::from_ref(&raw), TruncationPolicy::Tokens(10_000));

    assert_eq!(history.raw_items().cloned().collect::<Vec<_>>(), vec![raw]);
    assert_eq!(
        history.for_prompt_with_policy(
            &default_input_modalities(),
            TruncationPolicy::Tokens(10_000),
        ),
        vec![expected]
    );
}
