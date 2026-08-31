use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::NewThread;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_login::CodexAuth;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use wiremock::MockServer;

use super::compact::non_openai_model_provider;
use super::compact::set_test_compact_prompt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManualCompaction {
    Local,
    RemoteV1,
    RemoteV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reconstruction {
    Resume,
    Fork,
}

struct CompactionMocks {
    sampling: ResponseMock,
    compact: Option<ResponseMock>,
}

fn additional_context() -> BTreeMap<String, AdditionalContextEntry> {
    BTreeMap::from([
        (
            "automation_info".to_string(),
            AdditionalContextEntry {
                value: "same run".to_string(),
                kind: AdditionalContextKind::Application,
            },
        ),
        (
            "browser_info".to_string(),
            AdditionalContextEntry {
                value: "same tab".to_string(),
                kind: AdditionalContextKind::Untrusted,
            },
        ),
    ])
}

async fn mount_compaction_mocks(
    server: &MockServer,
    compaction: ManualCompaction,
) -> CompactionMocks {
    match compaction {
        ManualCompaction::Local => CompactionMocks {
            sampling: mount_sse_sequence(
                server,
                vec![
                    sse_completed("initial-response"),
                    sse(vec![
                        ev_response_created("local-compact-response"),
                        ev_assistant_message("local-summary", "compacted summary"),
                        ev_completed("local-compact-response"),
                    ]),
                    sse_completed("reconstructed-response"),
                ],
            )
            .await,
            compact: None,
        },
        ManualCompaction::RemoteV1 => CompactionMocks {
            sampling: mount_sse_sequence(
                server,
                vec![
                    sse_completed("initial-response"),
                    sse_completed("reconstructed-response"),
                ],
            )
            .await,
            compact: Some(
                responses::mount_compact_user_history_with_summary_once(
                    server,
                    "remote v1 summary",
                )
                .await,
            ),
        },
        ManualCompaction::RemoteV2 => CompactionMocks {
            sampling: mount_sse_sequence(
                server,
                vec![
                    sse_completed("initial-response"),
                    sse(vec![
                        ev_response_created("remote-v2-compact-response"),
                        json!({
                            "type": "response.output_item.done",
                            "item": {
                                "type": "compaction",
                                "encrypted_content": "remote v2 summary",
                            }
                        }),
                        ev_completed("remote-v2-compact-response"),
                    ]),
                    sse_completed("reconstructed-response"),
                ],
            )
            .await,
            compact: None,
        },
    }
}

async fn submit_context_turn(
    codex: &Arc<CodexThread>,
    prompt: &str,
    context: BTreeMap<String, AdditionalContextEntry>,
) -> Result<()> {
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(context),
        )
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

fn context_texts(request: &ResponsesRequest, role: &str, prefix: &str) -> Vec<String> {
    request
        .message_input_texts(role)
        .into_iter()
        .filter(|text| text.starts_with(prefix))
        .collect()
}

async fn reconstruct_thread(
    reconstruction: Reconstruction,
    builder: &mut core_test_support::test_codex::TestCodexBuilder,
    server: &MockServer,
    initial: &TestCodex,
) -> Result<Arc<CodexThread>> {
    match reconstruction {
        Reconstruction::Resume => Ok(builder.restart(server, initial).await?.codex),
        Reconstruction::Fork => {
            let NewThread { thread, .. } = initial
                .thread_manager
                .fork_thread(
                    ForkSnapshot::Interrupted,
                    initial.config.clone(),
                    initial.codex.rollout_path().expect("rollout path"),
                    /*thread_source*/ None,
                    /*parent_trace*/ None,
                )
                .await?;
            Ok(thread)
        }
    }
}

fn assert_compaction_checkpoint_has_additional_context_baseline(test: &TestCodex) -> Result<()> {
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let lines = std::fs::read_to_string(rollout_path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let compacted_index = lines
        .iter()
        .rposition(|line| matches!(&line.item, RolloutItem::Compacted(_)))
        .expect("compaction checkpoint");
    let world_state = lines[compacted_index + 1..]
        .iter()
        .find_map(|line| match &line.item {
            RolloutItem::WorldState(world_state) if world_state.full => Some(world_state),
            RolloutItem::WorldState(_) => None,
            RolloutItem::Compacted(_) => None,
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::InterAgentCommunication(_)
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::TurnContext(_)
            | RolloutItem::RealtimeItem(_)
            | RolloutItem::SecurityRiskScore(_)
            | RolloutItem::EventMsg(_) => None,
        })
        .expect("full world-state baseline after compaction");
    assert_eq!(
        world_state.state.keys().cloned().collect::<Vec<_>>(),
        vec!["additional_context"]
    );
    Ok(())
}

#[test_case(ManualCompaction::Local, Reconstruction::Resume; "local resume")]
#[test_case(ManualCompaction::Local, Reconstruction::Fork; "local fork")]
#[test_case(ManualCompaction::RemoteV1, Reconstruction::Resume; "remote v1 resume")]
#[test_case(ManualCompaction::RemoteV1, Reconstruction::Fork; "remote v1 fork")]
#[test_case(ManualCompaction::RemoteV2, Reconstruction::Resume; "remote v2 resume")]
#[test_case(ManualCompaction::RemoteV2, Reconstruction::Fork; "remote v2 fork")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compaction_preserves_additional_context_projection(
    compaction: ManualCompaction,
    reconstruction: Reconstruction,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mocks = mount_compaction_mocks(&server, compaction).await;
    let local_provider =
        (compaction == ManualCompaction::Local).then(|| non_openai_model_provider(&server));
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.include_environment_context = false;
            match compaction {
                ManualCompaction::Local => {
                    config.model_provider = local_provider.expect("local provider");
                    set_test_compact_prompt(config);
                }
                ManualCompaction::RemoteV1 => {
                    config
                        .features
                        .disable(Feature::RemoteCompactionV2)
                        .expect("remote compaction v2 should be configurable");
                }
                ManualCompaction::RemoteV2 => {
                    config
                        .features
                        .enable(Feature::RemoteCompactionV2)
                        .expect("remote compaction v2 should be configurable");
                }
            }
        });
    let initial = builder.build_with_auto_env(&server).await?;
    let context = additional_context();
    submit_context_turn(&initial.codex, "initial turn", context.clone()).await?;

    initial.codex.submit(Op::Compact).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_compaction_checkpoint_has_additional_context_baseline(&initial)?;

    let reconstructed = reconstruct_thread(reconstruction, &mut builder, &server, &initial).await?;
    submit_context_turn(&reconstructed, "reconstructed turn", context).await?;

    let requests = mocks.sampling.requests();
    let expected_request_count = match compaction {
        ManualCompaction::RemoteV1 => 2,
        ManualCompaction::Local | ManualCompaction::RemoteV2 => 3,
    };
    assert_eq!(requests.len(), expected_request_count);
    let reconstructed_request = requests.last().expect("reconstructed request");
    assert_eq!(
        context_texts(reconstructed_request, "developer", "<automation_info>",),
        vec!["<automation_info>same run</automation_info>"]
    );
    assert_eq!(
        context_texts(reconstructed_request, "user", "<external_browser_info>",),
        vec!["<external_browser_info>same tab</external_browser_info>"]
    );

    match compaction {
        ManualCompaction::Local => {
            assert!(requests[1].inputs_of_type("compaction_trigger").is_empty());
        }
        ManualCompaction::RemoteV1 => {
            assert_eq!(
                mocks
                    .compact
                    .expect("remote v1 compact mock")
                    .requests()
                    .len(),
                1
            );
        }
        ManualCompaction::RemoteV2 => {
            assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
        }
    }

    Ok(())
}
