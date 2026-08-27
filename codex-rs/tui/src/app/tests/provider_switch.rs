use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_model_provider_info::WireApi;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffortPreset;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

const SOURCE_PROVIDER: &str = "provider-switch-source";
const TARGET_PROVIDER: &str = "provider-switch-target";
const SOURCE_MODEL: &str = "source-only-model";
const TARGET_MODEL: &str = "target-only-model";

#[tokio::test]
async fn switching_provider_preserves_draft_and_separate_plan_effort() -> Result<()> {
    let server = MockServer::start().await;
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::File;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token"),
        AuthCredentialsStoreMode::File,
    )
    .expect("write ChatGPT authentication");

    let mut source_model = construct_model_info_offline_for_tests(
        SOURCE_MODEL,
        &app.config.to_models_manager_config(),
    );
    source_model.visibility = ModelVisibility::List;
    source_model.supported_in_api = true;
    source_model.priority = 0;
    source_model.default_reasoning_level = Some(ReasoningEffortConfig::Low);
    source_model.supported_reasoning_levels = vec![
        ReasoningEffortPreset {
            effort: ReasoningEffortConfig::Low,
            description: String::new(),
        },
        ReasoningEffortPreset {
            effort: ReasoningEffortConfig::High,
            description: String::new(),
        },
    ];
    let mut target_model = source_model.clone();
    target_model.slug = TARGET_MODEL.to_string();
    target_model.display_name = "Target model".to_string();
    target_model.priority = -100;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ModelsResponse {
            models: vec![target_model],
        }))
        .expect(2)
        .mount(&server)
        .await;

    let source_provider = ModelProviderInfo {
        name: "Source provider".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        wire_api: WireApi::Responses,
        requires_openai_auth: true,
        ..ModelProviderInfo::default()
    };
    let target_provider = ModelProviderInfo {
        name: "OpenAI".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        wire_api: WireApi::Responses,
        requires_openai_auth: true,
        ..ModelProviderInfo::default()
    };
    app.config
        .model_providers
        .insert(SOURCE_PROVIDER.to_string(), source_provider.clone());
    app.config
        .model_providers
        .insert(TARGET_PROVIDER.to_string(), target_provider);
    app.config.model_provider_id = SOURCE_PROVIDER.to_string();
    app.config.model_provider = source_provider;
    app.config.model = Some(SOURCE_MODEL.to_string());
    app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Low);
    app.config.plan_mode_reasoning_effort = Some(ReasoningEffortConfig::High);
    app.config.model_catalog = Some(ModelsResponse {
        models: vec![source_model.clone()],
    });
    std::fs::write(
        codex_home.path().join("source-models.json"),
        serde_json::to_vec(
            app.config
                .model_catalog
                .as_ref()
                .expect("source model catalog"),
        )?,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "{SOURCE_MODEL}"
model_provider = "{SOURCE_PROVIDER}"
model_catalog_json = "source-models.json"

[model_providers.{SOURCE_PROVIDER}]
name = "Source provider"
base_url = "{base_url}/v1"
wire_api = "responses"
requires_openai_auth = true

[model_providers.{TARGET_PROVIDER}]
name = "OpenAI"
base_url = "{base_url}/v1"
wire_api = "responses"
requires_openai_auth = true
"#,
            base_url = server.uri(),
        ),
    )?;
    let mut source_presets = vec![ModelPreset::from(source_model)];
    ModelPreset::mark_default_by_picker_visibility(&mut source_presets);
    app.model_catalog = Arc::new(ModelCatalog::new(source_presets.clone()));

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = app_server.start_thread(&app.config).await?;
    let source_thread_id = started.session.thread_id;
    let expected_target_base_url = format!("{}/v1", server.uri());
    let mut resumed_target = started.session.clone();
    resumed_target.model_provider_id = TARGET_PROVIDER.to_string();
    resumed_target.model = TARGET_MODEL.to_string();
    resumed_target.reasoning_effort = Some(ReasoningEffortConfig::Low);
    let mut resumed_config = app.config.clone();
    let mut resumed_models = source_presets.clone();
    let mut resumed_runtime_base_url = resumed_config.model_provider.base_url.clone();
    crate::app::provider_switch::reconcile_session_model_environment(
        &mut resumed_config,
        &mut app_server,
        &app.app_server_target,
        &resumed_target,
        &mut resumed_models,
        &mut resumed_runtime_base_url,
    )
    .await?;
    assert_eq!(
        (
            resumed_config.model_provider_id.as_str(),
            resumed_config.model.as_deref(),
            resumed_config.model_reasoning_effort,
            resumed_config.plan_mode_reasoning_effort,
            resumed_runtime_base_url.as_deref(),
            resumed_models
                .iter()
                .any(|model| model.model == TARGET_MODEL),
        ),
        (
            TARGET_PROVIDER,
            Some(TARGET_MODEL),
            Some(ReasoningEffortConfig::Low),
            Some(ReasoningEffortConfig::High),
            Some(expected_target_base_url.as_str()),
            true,
        )
    );
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("recorded model requests")
            .len(),
        1
    );
    std::fs::remove_file(codex_home.path().join("models_cache.json"))?;
    app_server.set_active_model_catalog(SOURCE_MODEL.to_string(), source_presets);
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        &mut app_server,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::Low));
    app.chat_widget
        .set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
    app.chat_widget
        .set_collaboration_mask(CollaborationModeMask {
            name: "Plan".to_string(),
            mode: Some(ModeKind::Plan),
            model: Some(SOURCE_MODEL.to_string()),
            reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
            developer_instructions: None,
        });
    app.chat_widget
        .apply_external_edit("preserve this draft".to_string());

    app.start_model_provider_switch(&app_server, TARGET_PROVIDER.to_string());
    let prepared = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = app_event_rx.recv().await.ok_or_else(|| {
                color_eyre::eyre::eyre!("provider switch preparation channel closed")
            })?;
            if matches!(event, AppEvent::ModelProviderSwitchPrepared(..)) {
                return Ok::<_, color_eyre::Report>(event);
            }
        }
    })
    .await??;
    match prepared {
        AppEvent::ModelProviderSwitchPrepared(request_id, thread_id, provider_id, result) => {
            app.complete_model_provider_switch(
                &mut tui,
                &mut app_server,
                request_id,
                thread_id,
                provider_id,
                result,
            )
            .await;
        }
        other => {
            return Err(color_eyre::eyre::eyre!(
                "expected provider switch preparation, got {other:?}"
            ));
        }
    }

    assert_ne!(app.chat_widget.thread_id(), Some(source_thread_id));
    assert_eq!(app.config.model_provider_id, TARGET_PROVIDER);
    assert_eq!(app.chat_widget.current_model(), TARGET_MODEL);
    assert_eq!(
        app.chat_widget
            .current_collaboration_mode()
            .reasoning_effort(),
        Some(ReasoningEffortConfig::Low)
    );
    assert_eq!(
        app.chat_widget.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(
        app.config.plan_mode_reasoning_effort,
        Some(ReasoningEffortConfig::High)
    );
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "preserve this draft"
    );
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("recorded model requests")
            .len(),
        2
    );

    app_server.shutdown().await?;
    Ok(())
}
