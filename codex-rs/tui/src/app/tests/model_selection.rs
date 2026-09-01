use super::*;
use crate::app_event::ModelSelectionScope;
use assert_matches::assert_matches;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn context_window_selection_updates_memory_and_preserves_configured_provider() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.model_provider_id = "copilot".to_string();
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"model_provider = "microsoft"

[model_providers.microsoft]
name = "Microsoft Azure OpenAI"
base_url = "https://example.openai.azure.com/openai/v1"
wire_api = "responses"
"#,
    )?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CommitModelSelection {
            model: "gpt-5.6-sol".to_string(),
            effort: Some(ReasoningEffortConfig::High),
            context_window: Some(922_000),
            scope: ModelSelectionScope::GlobalAndPlan,
        },
    )
    .await?;

    assert_eq!(app.config.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(
        app.config.model_reasoning_effort,
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(app.config.model_context_window, Some(922_000));
    assert_eq!(
        app.config.plan_mode_reasoning_effort,
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(
        app.chat_widget.config_ref().model_context_window,
        Some(922_000)
    );

    let persist_plan_event = app_event_rx
        .try_recv()
        .expect("complete Plan selection should enqueue Plan persistence");
    assert_matches!(
        &persist_plan_event,
        AppEvent::PersistPlanModeReasoningEffort(Some(ReasoningEffortConfig::High))
    );
    app.handle_event(&mut tui, &mut app_server, persist_plan_event)
        .await?;

    let persist_event = app_event_rx
        .try_recv()
        .expect("complete selection should enqueue persistence");
    assert_matches!(
        &persist_event,
        AppEvent::PersistModelSelection {
            model,
            effort: Some(ReasoningEffortConfig::High),
            context_window: Some(922_000),
        } if model == "gpt-5.6-sol"
    );
    app.handle_event(&mut tui, &mut app_server, persist_event)
        .await?;

    let persisted_config = toml::from_str::<TomlValue>(&std::fs::read_to_string(config_path)?)?;
    let expected_config = toml::from_str::<TomlValue>(
        r#"model = "gpt-5.6-sol"
model_provider = "microsoft"
model_reasoning_effort = "high"
model_context_window = 922000
plan_mode_reasoning_effort = "high"

[model_providers.microsoft]
name = "Microsoft Azure OpenAI"
base_url = "https://example.openai.azure.com/openai/v1"
wire_api = "responses"
"#,
    )?;
    assert_eq!(
        persisted_config, expected_config,
        "model selection should leave the configured provider intact"
    );

    app_server.shutdown().await?;
    Ok(())
}
