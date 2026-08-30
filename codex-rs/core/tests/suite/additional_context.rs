use anyhow::Result;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::NewThread;
use codex_core::TurnInputRequest;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

use super::compact::non_openai_model_provider;
use super::compact::set_test_compact_prompt;

fn additional_context(
    entries: &[(&str, &str, AdditionalContextKind)],
) -> BTreeMap<String, AdditionalContextEntry> {
    entries
        .iter()
        .map(|(key, value, kind)| {
            (
                (*key).to_string(),
                AdditionalContextEntry {
                    value: (*value).to_string(),
                    kind: *kind,
                },
            )
        })
        .collect()
}

fn context_texts(request: &ResponsesRequest, role: &str, prefix: &str) -> Vec<String> {
    request
        .message_input_texts(role)
        .into_iter()
        .filter(|text| text.starts_with(prefix))
        .collect()
}

async fn submit_context_turn(
    codex: &Arc<CodexThread>,
    prompt: &str,
    context: Option<BTreeMap<String, AdditionalContextEntry>>,
) -> Result<()> {
    let request = TurnInputRequest::user_input(vec![UserInput::Text {
        text: prompt.to_string(),
        text_elements: Vec::new(),
    }]);
    let request = match context {
        Some(context) => request.with_additional_context(context),
        None => request,
    };
    codex.start_or_steer_turn(request).await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_model_visible_but_not_a_user_message_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect the active tab".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: "tab one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Application,
                    },
                ),
            ])),
        )
        .await?;

    let user_item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        user_item.content,
        vec![UserInput::Text {
            text: "inspect the active tab".to_string(),
            text_elements: Vec::new(),
        }]
    );
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    assert!(request.has_content_kinds(&["additional_content.automation_info"]));
    assert!(request.has_content_kinds(&["additional_content.browser_info"]));
    assert!(request.has_content_kinds(&["user.text"]));
    insta::assert_snapshot!(
        "additional_context_simple_input",
        context_snapshot::format_labeled_requests_snapshot(
            "additional context is inserted before the user turn input.",
            &[("Request", &request)],
            &ContextSnapshotOptions::default()
                .strip_capability_instructions()
                .render_mode(ContextSnapshotRenderMode::KindWithTextPrefix { max_chars: 160 }),
        )
    );
    let developer_context_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_context_texts,
        vec!["<automation_info>run one</automation_info>"]
    );
    assert_eq!(
        request.message_input_texts("user"),
        vec![
            "<external_browser_info>tab one</external_browser_info>",
            "inspect the active tab",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_context_like_user_text_remains_a_user_message_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let user_input = UserInput::Text {
        text: "<external_api>".to_string(),
        text_elements: Vec::new(),
    };

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![user_input.clone()]))
        .await?;

    let user_item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    assert_eq!(user_item.content, vec![user_input]);
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    assert_eq!(request.message_input_texts("user"), vec!["<external_api>"]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_trust_controls_message_role() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect context".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: "tab one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Application,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    let developer_context_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_context_texts,
        vec!["<automation_info>run one</automation_info>"]
    );
    assert_eq!(
        request.message_input_texts("user"),
        vec![
            "<external_browser_info>tab one</external_browser_info>",
            "inspect context",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_deduplicated_between_turns_while_retained() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let additional_context = BTreeMap::from([(
        "browser_info".to_string(),
        AdditionalContextEntry {
            value: "same tab".to_string(),
            kind: AdditionalContextKind::Untrusted,
        },
    )]);

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(additional_context.clone()),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(additional_context),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
            "second turn",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn omitted_additional_context_preserves_the_current_projection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let requests = mount_sse_sequence(
        &server,
        vec![
            sse_completed("resp-1"),
            sse_completed("resp-2"),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let context =
        additional_context(&[("browser_info", "same tab", AdditionalContextKind::Untrusted)]);

    for (prompt, context) in [
        ("first turn", Some(context.clone())),
        ("ordinary turn", None),
        ("third turn", Some(context)),
    ] {
        submit_context_turn(&test.codex, prompt, context).await?;
    }

    let requests = requests.requests();
    assert_eq!(requests.len(), 3);
    let third_request = &requests[2];
    let third_request_bytes = third_request.body_json().to_string().len();
    eprintln!("additional_context_final_serialized_request_bytes={third_request_bytes}");
    assert_eq!(
        context_texts(third_request, "user", "<external_browser_info>").len(),
        1,
        "third request serialized bytes: {third_request_bytes}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compaction_rehydrates_the_current_additional_context_projection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let requests = mount_sse_sequence(
        &server,
        vec![
            sse_completed("resp-1"),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("summary-1", "compacted summary"),
                ev_completed("resp-2"),
            ]),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let model_provider = non_openai_model_provider(&server);
    let test = test_codex()
        .with_config(move |config| {
            config.include_environment_context = false;
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
        })
        .build(&server)
        .await?;

    submit_context_turn(
        &test.codex,
        "first turn",
        Some(additional_context(&[(
            "browser_info",
            "same tab",
            AdditionalContextKind::Untrusted,
        )])),
    )
    .await?;

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| matches!(event, EventMsg::Warning(_))).await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    submit_context_turn(&test.codex, "after compact", None).await?;

    let requests = requests.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        context_texts(&requests[2], "user", "<external_browser_info>"),
        vec!["<external_browser_info>same tab</external_browser_info>"]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_and_fork_restore_the_additional_context_projection_snapshot() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let requests = mount_sse_sequence(
        &server,
        vec![
            sse_completed("resp-initial"),
            sse_completed("resp-forked"),
            sse_completed("resp-resumed"),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.include_environment_context = false;
    });
    let initial = builder.build(&server).await?;
    let context =
        additional_context(&[("browser_info", "same tab", AdditionalContextKind::Untrusted)]);
    submit_context_turn(&initial.codex, "initial turn", Some(context.clone())).await?;

    let NewThread { thread: forked, .. } = initial
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            initial.config.clone(),
            initial.codex.rollout_path().expect("rollout path"),
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await?;
    submit_context_turn(&forked, "forked turn", Some(context.clone())).await?;
    let resumed = builder.restart(&server, &initial).await?;
    submit_context_turn(&resumed.codex, "resumed turn", Some(context)).await?;

    let requests = requests.requests();
    assert_eq!(requests.len(), 3);
    for request in &requests[1..] {
        assert_eq!(
            context_texts(request, "user", "<external_browser_info>"),
            vec!["<external_browser_info>same tab</external_browser_info>"]
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_restores_projection_for_both_context_treatments() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let requests = mount_sse_sequence(
        &server,
        vec![
            sse_completed("resp-1"),
            sse_completed("resp-2"),
            sse_completed("resp-3"),
            sse_completed("resp-4"),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let first_context = additional_context(&[
        (
            "automation_info",
            "run one",
            AdditionalContextKind::Application,
        ),
        ("browser_info", "tab one", AdditionalContextKind::Untrusted),
    ]);
    let second_context = additional_context(&[
        (
            "automation_info",
            "run two",
            AdditionalContextKind::Application,
        ),
        ("browser_info", "tab two", AdditionalContextKind::Untrusted),
    ]);

    for (prompt, context) in [
        ("first turn", first_context.clone()),
        ("second turn", second_context),
    ] {
        submit_context_turn(&test.codex, prompt, Some(context)).await?;
    }

    test.codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ThreadRolledBack(_))
    })
    .await;
    submit_context_turn(&test.codex, "retried turn", Some(first_context.clone())).await?;

    let first_three_requests = requests.requests();
    assert_eq!(
        context_texts(&first_three_requests[2], "user", "<external_browser_info>",),
        vec!["<external_browser_info>tab one</external_browser_info>"]
    );
    assert_eq!(
        context_texts(&first_three_requests[2], "developer", "<automation_info>"),
        vec!["<automation_info>run one</automation_info>"]
    );

    test.codex
        .submit(Op::ThreadRollback { num_turns: 2 })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ThreadRolledBack(_))
    })
    .await;
    submit_context_turn(&test.codex, "fresh turn", Some(first_context)).await?;

    let requests = requests.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        context_texts(&requests[3], "user", "<external_browser_info>"),
        vec!["<external_browser_info>tab one</external_browser_info>"]
    );
    assert_eq!(
        context_texts(&requests[3], "developer", "<automation_info>"),
        vec!["<automation_info>run one</automation_info>"]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_removes_one_value_while_adding_another() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let third_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: "tab one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "terminal_info".to_string(),
                    AdditionalContextEntry {
                        value: "pty one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "third turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: "run one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: "tab one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
                (
                    "terminal_info".to_string(),
                    AdditionalContextEntry {
                        value: "pty one".to_string(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
        ]
    );
    assert_eq!(
        third_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
            "<external_browser_info>tab one</external_browser_info>",
            "third turn",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_values_are_truncated_before_model_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES: usize = 5 * 1024;

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let long_browser_value = format!("browser-head-{}browser-tail", "b".repeat(40_000));
    let long_automation_value = format!("automation-head-{}automation-tail", "a".repeat(40_000));
    let untruncated_browser_fragment =
        format!("<external_browser_info>{long_browser_value}</external_browser_info>");
    let untruncated_automation_fragment =
        format!("<automation_info>{long_automation_value}</automation_info>");

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "summarize context".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry {
                        value: long_automation_value.clone(),
                        kind: AdditionalContextKind::Application,
                    },
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry {
                        value: long_browser_value.clone(),
                        kind: AdditionalContextKind::Untrusted,
                    },
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    let developer_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    let [automation_text] = developer_texts.as_slice() else {
        panic!("expected application additional context, got {developer_texts:?}");
    };
    assert!(automation_text.starts_with(&format!(
        "<automation_info>automation-head-{}",
        "a".repeat(1024)
    )));
    assert!(automation_text.contains("tokens truncated"));
    assert!(automation_text.ends_with("automation-tail</automation_info>"));
    assert!(automation_text.len() < untruncated_automation_fragment.len());
    assert!(
        automation_text.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES,
        "application additional context was not capped before model input: {} bytes",
        automation_text.len()
    );

    let user_texts = request.message_input_texts("user");
    let [external_text, user_text] = user_texts.as_slice() else {
        panic!("expected external context plus user input, got {user_texts:?}");
    };
    assert_eq!(user_text, "summarize context");
    assert!(external_text.starts_with(&format!(
        "<external_browser_info>browser-head-{}",
        "b".repeat(1024)
    )));
    assert!(external_text.contains("tokens truncated"));
    assert!(external_text.ends_with("browser-tail</external_browser_info>"));
    assert!(external_text.len() < untruncated_browser_fragment.len());
    assert!(
        external_text.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES,
        "untrusted additional context was not capped before model input: {} bytes",
        external_text.len()
    );

    Ok(())
}
