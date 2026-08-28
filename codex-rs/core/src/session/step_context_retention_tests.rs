//! Selected-step retention across compaction and terminal task transitions.

use super::*;
use crate::compact::InitialContextInjection;
use crate::responses_metadata::CodexResponsesRequestKind;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_extension_api::SelectedPluginSnapshot;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::Notify;
use wiremock::ResponseTemplate;

pub(in crate::session) struct HeldStepTask {
    pub(in crate::session) kind: TaskKind,
    pub(in crate::session) finish: Arc<Notify>,
}

impl SessionTask for HeldStepTask {
    fn kind(&self) -> TaskKind {
        self.kind
    }

    fn span_name(&self) -> &'static str {
        "session_task.step_activation_test"
    }

    async fn run(
        self: Arc<Self>,
        _session: Arc<Session>,
        _turn: Arc<TurnContext>,
        _input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> SessionTaskResult {
        tokio::select! {
            _ = cancellation_token.cancelled() => {},
            _ = self.finish.notified() => {},
        }
        Ok(None)
    }
}

#[test_case(TerminalEventKind::TurnComplete; "completion")]
#[test_case(TerminalEventKind::TurnAborted; "interruption")]
#[tokio::test]
async fn finished_turn_retains_last_known_step_context(terminal: TerminalEventKind) {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    let finish = Arc::new(Notify::new());
    session
        .spawn_task(
            Arc::clone(&turn),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::clone(&finish),
            },
        )
        .await;
    let expected = session
        .capture_step_context(turn, &CancellationToken::new())
        .await
        .expect("capture executing step");
    let state = {
        let active = session.active_turn.lock().await;
        Arc::clone(&active.as_ref().expect("active turn").turn_state)
    };

    match terminal {
        TerminalEventKind::TurnComplete => finish.notify_one(),
        TerminalEventKind::TurnAborted => {
            session.abort_all_tasks(TurnAbortReason::Interrupted).await;
        }
    }
    recv_terminal_event(&events, terminal).await;

    assert!(session.active_turn.lock().await.is_none());
    assert_eq!(
        state
            .lock()
            .await
            .last_known_step_context
            .as_ref()
            .map(Arc::as_ptr),
        Some(Arc::as_ptr(&expected)),
    );
}

#[derive(Clone, Copy)]
enum FirstAttempt {
    Succeeds,
    Retries,
}

#[tokio::test]
async fn speculative_capture_preserves_selected_plugin_and_tool_metadata_until_selected() {
    let (session, mut turn, events) = make_session_and_context_with_rx().await;
    let unshared_turn = Arc::get_mut(&mut turn).expect("unshared turn");
    let config = Arc::make_mut(&mut unshared_turn.config);
    config.tool_registry.turn_metadata_includes_tool_info = true;
    for feature in [Feature::ShellTool, Feature::UnifiedExec, Feature::CodeMode] {
        config
            .features
            .enable(feature)
            .expect("enable tool planning feature");
    }
    update_turn_settings_for_test(unshared_turn, |settings| {
        let model = Arc::make_mut(&mut settings.model_info);
        model.use_responses_lite = true;
        model.tool_mode = Some(ToolMode::Direct);
        model.shell_type = ConfigShellToolType::Disabled;
        model.apply_patch_tool_type = None;
    });
    session
        .spawn_task(
            Arc::clone(&turn),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(Notify::new()),
            },
        )
        .await;

    let mut primary = session
        .capture_speculative_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture primary step");
    // Isolate publication from the MCP projection that normally supplies this snapshot.
    Arc::get_mut(&mut primary)
        .expect("unshared primary candidate")
        .selected_plugins
        .disabled_plugin_roots = vec!["primary-disabled".to_string()];
    session.set_last_known_step_context(&primary).await;
    let published_metadata = || {
        (
            turn.extension_data
                .get::<SelectedPluginSnapshot>()
                .expect("published plugin snapshot")
                .disabled_plugin_roots
                .clone(),
            turn.turn_metadata_state
                .to_responses_metadata(
                    "installation".to_string(),
                    "window".to_string(),
                    CodexResponsesRequestKind::Turn,
                )
                .tool_namespaces_info,
        )
    };
    let expected_primary = (
        vec!["primary-disabled".to_string()],
        primary.tool_router.tool_namespaces_info().cloned(),
    );
    assert_eq!(published_metadata(), expected_primary);

    let mut fallback_turn = turn
        .with_model("gpt-5.2".to_string(), &session.services.models_manager)
        .await;
    update_turn_settings_for_test(&mut fallback_turn, |settings| {
        let model = Arc::make_mut(&mut settings.model_info);
        model.use_responses_lite = true;
        model.tool_mode = Some(ToolMode::CodeModeOnly);
        model.shell_type = ConfigShellToolType::UnifiedExec;
        model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
    });
    let fallback = session
        .capture_speculative_step_context(Arc::new(fallback_turn), &CancellationToken::new())
        .await
        .expect("capture candidate");
    let expected_fallback = (
        fallback.selected_plugins.disabled_plugin_roots.clone(),
        fallback.tool_router.tool_namespaces_info().cloned(),
    );
    assert_ne!(expected_fallback.0, expected_primary.0);
    assert_ne!(expected_fallback.1, expected_primary.1);
    assert_eq!(published_metadata(), expected_primary);

    session.set_last_known_step_context(&fallback).await;
    assert_eq!(published_metadata(), expected_fallback);

    let mut unreported_turn = turn
        .with_model("gpt-5.2".to_string(), &session.services.models_manager)
        .await;
    update_turn_settings_for_test(&mut unreported_turn, |settings| {
        Arc::make_mut(&mut settings.model_info).use_responses_lite = false;
    });
    let unreported = session
        .capture_speculative_step_context(Arc::new(unreported_turn), &CancellationToken::new())
        .await
        .expect("capture step without reported inventory");
    assert_eq!(published_metadata(), expected_fallback);
    session.set_last_known_step_context(&unreported).await;
    assert_eq!(
        published_metadata(),
        (
            unreported.selected_plugins.disabled_plugin_roots.clone(),
            None
        ),
    );
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    recv_terminal_event(&events, TerminalEventKind::TurnAborted).await;
}

async fn make_remote_compaction_session(
    server_uri: &str,
) -> (
    Arc<Session>,
    Arc<TurnContext>,
    async_channel::Receiver<Event>,
) {
    // The unit-test crate has its own client static, separate from core_test_support's copy.
    crate::client::enable_responses_sse_for_tests();
    let mut provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    provider.base_url = Some(format!("{server_uri}/v1"));
    provider.supports_websockets = false;
    make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        Vec::new(),
        move |config| {
            config.model = Some("gpt-5.2".to_string());
            config.model_provider = provider;
            let _ = config.features.enable(Feature::RemoteCompactionV2);
            let _ = config.features.disable(Feature::TokenBudget);
            let _ = config.features.disable(Feature::EnableRequestCompression);
        },
    )
    .await
}

#[test_case(FirstAttempt::Succeeds; "primary succeeds")]
#[test_case(FirstAttempt::Retries; "fallback executes")]
#[tokio::test]
async fn legacy_compaction_retains_only_the_selected_step(first_attempt: FirstAttempt) {
    let server = responses::start_mock_server().await;
    let (session, turn, events) = make_remote_compaction_session(&server.uri()).await;
    session
        .record_conversation_items(&turn, &[user_message("before compaction")])
        .await;
    session
        .spawn_task(
            Arc::clone(&turn),
            Vec::new(),
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: true,
            },
        )
        .await;
    let primary_turn = Arc::new(
        turn.with_model("gpt-5.4".to_string(), &session.services.models_manager)
            .await,
    );
    let primary = session
        .capture_step_context(primary_turn, &CancellationToken::new())
        .await
        .expect("capture primary step");
    let fallback = session
        .capture_speculative_step_context(turn, &CancellationToken::new())
        .await
        .expect("capture speculative fallback");
    let state = {
        let active = session.active_turn.lock().await;
        Arc::clone(&active.as_ref().expect("active turn").turn_state)
    };
    assert_eq!(
        state
            .lock()
            .await
            .last_known_step_context
            .as_ref()
            .map(Arc::as_ptr),
        Some(Arc::as_ptr(&primary)),
    );

    let success = ResponseTemplate::new(/*status*/ 200).set_body_json(json!({
        "output": [{ "type": "compaction", "encrypted_content": "summary" }]
    }));
    let replies = match first_attempt {
        FirstAttempt::Succeeds => vec![success],
        FirstAttempt::Retries => vec![
            ResponseTemplate::new(/*status*/ 400)
                .set_body_json(json!({ "detail": "previous model unavailable" })),
            success,
        ],
    };
    let requests = responses::mount_compact_response_sequence(&server, replies).await;
    crate::compact_remote::run_inline_remote_auto_compact_task(
        Arc::clone(&session),
        Arc::clone(&primary),
        Some(Arc::clone(&fallback)),
        Arc::new(OnceLock::new()),
        InitialContextInjection::DoNotInject,
        CompactionReason::ModelDownshift,
        CompactionPhase::PreTurn,
    )
    .await
    .expect("compaction succeeds");

    let (expected, models) = match first_attempt {
        FirstAttempt::Succeeds => (&primary, vec![json!("gpt-5.4")]),
        FirstAttempt::Retries => (&fallback, vec![json!("gpt-5.4"), json!("gpt-5.2")]),
    };
    assert_eq!(
        state
            .lock()
            .await
            .last_known_step_context
            .as_ref()
            .map(Arc::as_ptr),
        Some(Arc::as_ptr(expected)),
    );
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        models,
    );
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    recv_terminal_event(&events, TerminalEventKind::TurnAborted).await;
}

#[tokio::test]
async fn interrupting_compaction_fallback_retains_last_known_step_context() {
    let (release_primary, primary_gate) = tokio::sync::oneshot::channel();
    let (release_fallback, fallback_gate) = tokio::sync::oneshot::channel();
    let (server, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(primary_gate),
            body: responses::sse_failed(
                "primary",
                "context_length_exceeded",
                "compact with the current model",
            ),
        }],
        vec![StreamingSseChunk {
            gate: Some(fallback_gate),
            body: responses::sse_completed("fallback"),
        }],
    ])
    .await;
    let (session, mut turn, events) = make_remote_compaction_session(server.uri()).await;
    update_turn_settings_for_test(
        Arc::get_mut(&mut turn).expect("unshared turn"),
        |settings| {
            Arc::make_mut(&mut settings.model_info).comp_hash = Some("new".to_string());
        },
    );
    session
        .set_previous_turn_settings(Some(PreviousTurnSettings {
            model: "gpt-5.4".to_string(),
            comp_hash: Some("old".to_string()),
            realtime_active: Some(turn.realtime_active),
        }))
        .await;
    session
        .record_conversation_items(&turn, &[user_message("before compaction")])
        .await;
    session
        .spawn_task(turn, Vec::new(), crate::tasks::RegularTask::new())
        .await;
    let state = {
        let active = session.active_turn.lock().await;
        Arc::clone(&active.as_ref().expect("active turn").turn_state)
    };

    // The real turn loop has prepared both contexts before sending its first compact request.
    timeout(
        Duration::from_secs(/*secs*/ 10),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await
    .unwrap_or_else(|error| {
        let queued_events = std::iter::from_fn(|| events.try_recv().ok())
            .map(|event| event.msg)
            .collect::<Vec<_>>();
        panic!("primary compaction request: {error}; queued events: {queued_events:?}");
    });
    let primary = state
        .lock()
        .await
        .last_known_step_context
        .clone()
        .expect("primary step");
    assert_eq!(primary.settings.model_info.slug, "gpt-5.4");

    release_primary.send(()).expect("release primary failure");
    timeout(
        Duration::from_secs(/*secs*/ 10),
        server.wait_for_request_count(/*count*/ 2),
    )
    .await
    .expect("fallback compaction request");
    let fallback = state
        .lock()
        .await
        .last_known_step_context
        .clone()
        .expect("fallback step");
    assert_eq!(fallback.settings.model_info.slug, "gpt-5.2");

    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    recv_terminal_event(&events, TerminalEventKind::TurnAborted).await;
    assert_eq!(
        state
            .lock()
            .await
            .last_known_step_context
            .as_ref()
            .map(Arc::as_ptr),
        Some(Arc::as_ptr(&fallback)),
    );
    drop(release_fallback);
    server.shutdown().await;
}
