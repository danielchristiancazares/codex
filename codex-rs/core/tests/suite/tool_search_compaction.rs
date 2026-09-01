use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use core_test_support::apps_test_server::configure_search_capable_model;
use core_test_support::responses;
use core_test_support::responses::namespace_child_tool;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

use super::compact::non_openai_model_provider;
use super::compact::set_test_compact_prompt;

const SEARCH_CALL_ID: &str = "pending-tool-search";
const TOOL_NAMESPACE: &str = "codex_app";
const TOOL_NAME: &str = "deferred_calendar_create";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompactionKind {
    Local,
    RemoteV1,
    RemoteV2,
}

#[test_case(CompactionKind::Local; "local")]
#[test_case(CompactionKind::RemoteV1; "remote_v1")]
#[test_case(CompactionKind::RemoteV2; "remote_v2")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_compaction_omits_search_schemas_and_restores_the_live_result(
    compaction_kind: CompactionKind,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let first_response = responses::sse(vec![
        responses::ev_tool_search_call(
            SEARCH_CALL_ID,
            &json!({
                "query": TOOL_NAME,
                "limit": 1,
            }),
        ),
        responses::ev_completed_with_tokens("response-1", /*total_tokens*/ 500),
    ]);
    let final_response = responses::sse(vec![
        responses::ev_assistant_message("message-final", "done"),
        responses::ev_completed_with_tokens("response-final", /*total_tokens*/ 10),
    ]);
    let response_mock = responses::mount_sse_sequence(
        &server,
        match compaction_kind {
            CompactionKind::Local => vec![
                first_response,
                responses::sse(vec![
                    responses::ev_assistant_message("message-compact", "compact summary"),
                    responses::ev_completed_with_tokens(
                        "response-compact",
                        /*total_tokens*/ 10,
                    ),
                ]),
                final_response,
            ],
            CompactionKind::RemoteV1 => vec![first_response, final_response],
            CompactionKind::RemoteV2 => vec![
                first_response,
                responses::sse(vec![
                    json!({
                        "type": "response.output_item.done",
                        "item": {
                            "type": "compaction",
                            "encrypted_content": "remote v2 compact summary",
                        }
                    }),
                    responses::ev_completed_with_tokens(
                        "response-compact",
                        /*total_tokens*/ 10,
                    ),
                ]),
                final_response,
            ],
        },
    )
    .await;
    let compact_mock = if compaction_kind == CompactionKind::RemoteV1 {
        Some(
            responses::mount_compact_user_history_with_summary_once(
                &server,
                "remote v1 compact summary",
            )
            .await,
        )
    } else {
        None
    };

    let local_provider =
        (compaction_kind == CompactionKind::Local).then(|| non_openai_model_provider(&server));
    let mut builder = test_codex().with_config(move |config| {
        configure_search_capable_model(config);
        set_test_compact_prompt(config);
        config.model_context_window = Some(100_000);
        config.model_auto_compact_token_limit = Some(200);
        if let Some(provider) = local_provider {
            config.model_provider = provider;
        }
        if compaction_kind == CompactionKind::RemoteV2 {
            config
                .features
                .enable(Feature::RemoteCompactionV2)
                .expect("remote compaction v2 should be configurable");
        } else {
            config
                .features
                .disable(Feature::RemoteCompactionV2)
                .expect("remote compaction v2 should be configurable");
        }
    });
    if compaction_kind != CompactionKind::Local {
        builder = builder.with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    }
    let mut test = builder.build_with_auto_env(&server).await?;
    let new_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![deferred_tool()],
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.submit_text_turn("Find the deferred calendar tool")
        .await?;

    let response_requests = response_mock.requests();
    assert_eq!(
        response_requests.len(),
        match compaction_kind {
            CompactionKind::RemoteV1 => 2,
            CompactionKind::Local | CompactionKind::RemoteV2 => 3,
        }
    );
    let (compact_request, follow_up_request) = match compaction_kind {
        CompactionKind::Local => (response_requests[1].clone(), response_requests[2].clone()),
        CompactionKind::RemoteV1 => (
            compact_mock
                .as_ref()
                .expect("remote v1 should mount the compact endpoint")
                .single_request(),
            response_requests[1].clone(),
        ),
        CompactionKind::RemoteV2 => (response_requests[1].clone(), response_requests[2].clone()),
    };
    assert_eq!(
        compact_request.tool_search_output(SEARCH_CALL_ID)["tools"],
        json!([]),
        "compaction should retain the output envelope without resending its schema body"
    );

    let follow_up_output = follow_up_request.tool_search_output(SEARCH_CALL_ID);
    assert!(
        namespace_child_tool(&follow_up_output, TOOL_NAMESPACE, TOOL_NAME).is_some(),
        "the first coding-model request after compaction should receive the live search result: {follow_up_output}"
    );
    assert_eq!(
        (
            follow_up_request
                .inputs_of_type("tool_search_call")
                .iter()
                .filter(|item| {
                    item.get("call_id").and_then(serde_json::Value::as_str) == Some(SEARCH_CALL_ID)
                })
                .count(),
            follow_up_request
                .inputs_of_type("tool_search_output")
                .iter()
                .filter(|item| {
                    item.get("call_id").and_then(serde_json::Value::as_str) == Some(SEARCH_CALL_ID)
                })
                .count(),
        ),
        (1, 1),
    );

    Ok(())
}

fn deferred_tool() -> DynamicToolSpec {
    DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: TOOL_NAMESPACE.to_string(),
        description: "Calendar tools.".to_string(),
        tools: vec![DynamicToolNamespaceTool::Function(
            DynamicToolFunctionSpec {
                name: TOOL_NAME.to_string(),
                description: "Create a calendar event.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                    },
                    "required": ["title"],
                    "additionalProperties": false,
                }),
                defer_loading: true,
            },
        )],
    })
}
