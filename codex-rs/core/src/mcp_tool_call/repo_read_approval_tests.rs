use super::approval_not_required_for_repo_read;
use crate::mcp_tool_call::McpToolApprovalMetadata;
use crate::mcp_tool_call::McpToolApprovalPolicy;
use crate::mcp_tool_call::maybe_request_mcp_tool_approval;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::session::tests::mcp_config_for_test;
use crate::test_support::models_manager_with_provider;
use crate::tools::hook_names::HookToolName;
use codex_config::types::AppToolApproval;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::McpServerConfig;
use codex_mcp::McpServerRegistration;
use codex_mcp::ResolvedMcpCatalog;
use codex_model_provider::create_model_provider;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::ReviewDecision;
use codex_tools::ToolName;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[tokio::test]
async fn repo_reads_skip_guardian_while_symlink_escapes_are_reviewed() -> anyhow::Result<()> {
    let repo = tempdir()?;
    std::fs::create_dir(repo.path().join(".git"))?;
    let direct_path = repo.path().join("direct.txt");
    std::fs::write(&direct_path, "inside")?;

    let outside = tempdir()?;
    let outside_path = outside.path().join("outside.txt");
    std::fs::write(&outside_path, "outside")?;
    let escape_path = repo.path().join("escape.txt");
    create_file_symlink(&outside_path, &escape_path)?;

    let server = start_mock_server().await;
    let guardian_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-guardian-repo-read"),
            ev_assistant_message(
                "msg-guardian-repo-read",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "The requested symlink read is authorized.",
                })
                .to_string(),
            ),
            ev_completed("resp-guardian-repo-read"),
        ]),
    )
    .await;

    let (mut session, mut turn_context) = make_session_and_context().await;
    Arc::make_mut(&mut turn_context.config)
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest)?;
    let mut config = (*turn_context.config).clone();
    config.cwd = AbsolutePathBuf::from_absolute_path(repo.path())?;
    config.model_provider.base_url = format!("{}/v1", server.uri()).into();
    config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    let config = Arc::new(config);
    let models_manager = models_manager_with_provider(
        config.codex_home.to_path_buf(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    );
    session.services.models_manager = models_manager;
    turn_context.config = Arc::clone(&config);
    #[allow(deprecated)]
    {
        turn_context.cwd = config.cwd.clone();
    }
    turn_context.provider = create_model_provider(
        config.model_provider.clone(),
        turn_context.auth_manager.clone(),
    );

    let local_server: McpServerConfig = toml::from_str(
        r#"
command = "tools-mcp-server"
"#,
    )?;
    let mut catalog = ResolvedMcpCatalog::builder();
    catalog.register(McpServerRegistration::from_config(
        "tools".to_string(),
        local_server,
    ));
    let mut mcp_config = (*mcp_config_for_test(&config)).clone();
    mcp_config.mcp_server_catalog = catalog.build();

    let direct_invocation = McpInvocation {
        server: "tools".to_string(),
        tool: "Read".to_string(),
        arguments: json!({ "path": direct_path }).into(),
    };
    let outside_invocation = McpInvocation {
        server: "tools".to_string(),
        tool: "Read".to_string(),
        arguments: json!({ "path": outside_path }).into(),
    };
    let escape_invocation = McpInvocation {
        server: "tools".to_string(),
        tool: "Read".to_string(),
        arguments: json!({ "path": escape_path }).into(),
    };
    let followed_search_invocation = McpInvocation {
        server: "tools".to_string(),
        tool: "Search".to_string(),
        arguments: json!({
            "pattern": "inside",
            "path": repo.path(),
            "follow": true,
        })
        .into(),
    };
    assert_eq!(
        [
            &direct_invocation,
            &outside_invocation,
            &escape_invocation,
            &followed_search_invocation,
        ]
        .map(|invocation| {
            approval_not_required_for_repo_read(&mcp_config, &config.cwd, invocation)
        }),
        [true, false, false, false]
    );

    let metadata = McpToolApprovalMetadata {
        annotations: Default::default(),
        connector_id: Default::default(),
        link_id: Default::default(),
        connector_name: Default::default(),
        connector_description: Default::default(),
        connected_account_email: Default::default(),
        plugin_id: Default::default(),
        tool_title: "Read".to_string().into(),
        tool_description: Default::default(),
        mcp_app_resource_uri: Default::default(),
        codex_apps_meta: Default::default(),
        openai_file_input_optional_fields: Default::default(),
    };
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let step_context = StepContext::for_test(Arc::clone(&turn_context));
    let cancellation_token = CancellationToken::new();

    let direct_decision = maybe_request_mcp_tool_approval(
        &session,
        &step_context,
        &cancellation_token,
        "call-direct-read",
        &direct_invocation,
        &ToolName::namespaced(&direct_invocation.server, &direct_invocation.tool),
        &HookToolName::new("mcp__tools__Read"),
        &metadata,
        &mcp_config,
        McpToolApprovalPolicy::for_server(AppToolApproval::Auto),
    )
    .await;
    assert_eq!(direct_decision, Default::default());
    assert_eq!(guardian_mock.requests().len(), 0);

    let escape_decision = maybe_request_mcp_tool_approval(
        &session,
        &step_context,
        &cancellation_token,
        "call-symlink-read",
        &escape_invocation,
        &ToolName::namespaced(&escape_invocation.server, &escape_invocation.tool),
        &HookToolName::new("mcp__tools__Read"),
        &metadata,
        &mcp_config,
        McpToolApprovalPolicy::for_server(AppToolApproval::Auto),
    )
    .await;
    assert_eq!(escape_decision, ReviewDecision::Approved.into());
    assert!(
        guardian_mock
            .single_request()
            .body_contains_text("escape.txt")
    );

    Ok(())
}
