//! Run isolated structured requests through existing app-server methods.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::openai_models::ReasoningEffort;
use color_eyre::eyre::eyre;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

const STRUCTURED_TURN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const STRUCTURED_CLEANUP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const STRUCTURED_RESPONSE_MAX_BYTES: usize = 8 * 1024;

struct TemporaryStructuredTurnRequest {
    thread_id: String,
    prompt: String,
    output_schema: Value,
    effort: Option<ReasoningEffort>,
    notifications: UnboundedReceiver<ServerNotification>,
}

struct TemporaryStructuredTurnCleanup {
    request_handle: AppServerRequestHandle,
    thread_id: String,
    turn_id: Option<String>,
    armed: bool,
}

impl TemporaryStructuredTurnCleanup {
    fn new(request_handle: AppServerRequestHandle, thread_id: String) -> Self {
        Self {
            request_handle,
            thread_id,
            turn_id: None,
            armed: true,
        }
    }

    async fn finish(&mut self, interrupt: bool) {
        self.armed = false;
        let task = tokio::spawn(cleanup_temporary_structured_turn(
            self.request_handle.clone(),
            self.thread_id.clone(),
            self.turn_id.clone(),
            interrupt,
        ));
        if let Err(error) = task.await {
            tracing::debug!(%error, "temporary structured turn cleanup task failed");
        }
    }
}

impl Drop for TemporaryStructuredTurnCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(cleanup_temporary_structured_turn(
            self.request_handle.clone(),
            self.thread_id.clone(),
            self.turn_id.clone(),
            /*interrupt*/ true,
        ));
    }
}

/// Preserve the visible thread's provider, permissions, and external-tool isolation.
pub(crate) struct TemporaryStructuredThreadOptions {
    pub(crate) model: String,
    pub(crate) model_provider: String,
    pub(crate) cwd: String,
    pub(crate) active_permission_profile: Option<String>,
    pub(crate) mcp_server_names: Vec<String>,
}

/// Start an ephemeral thread without widening permissions or exposing tools and environment access.
///
/// Structured prompts can contain untrusted transcript text, so the effective app-server config is
/// read first and every MCP server is explicitly disabled alongside built-in and extension tools.
pub(crate) async fn start_temporary_thread(
    request_handle: &AppServerRequestHandle,
    options: TemporaryStructuredThreadOptions,
) -> color_eyre::Result<ThreadStartResponse> {
    let TemporaryStructuredThreadOptions {
        model,
        model_provider,
        cwd,
        active_permission_profile,
        mcp_server_names,
    } = options;
    let custom_permission_profile =
        active_permission_profile.filter(|profile| !profile.starts_with(':'));
    let mut config = std::collections::HashMap::from([
        ("features.apps".to_string(), false.into()),
        ("features.code_mode".to_string(), false.into()),
        ("features.code_mode_only".to_string(), false.into()),
        ("features.context_management".to_string(), false.into()),
        ("features.current_time_reminder".to_string(), false.into()),
        ("features.deferred_executor".to_string(), false.into()),
        ("features.enable_fanout".to_string(), false.into()),
        ("features.goals".to_string(), false.into()),
        ("features.hooks".to_string(), false.into()),
        ("features.image_generation".to_string(), false.into()),
        ("features.memories".to_string(), false.into()),
        ("features.multi_agent".to_string(), false.into()),
        ("features.multi_agent_v2".to_string(), false.into()),
        ("features.plugins".to_string(), false.into()),
        (
            "features.request_permissions_tool".to_string(),
            false.into(),
        ),
        ("features.shell_snapshot".to_string(), false.into()),
        ("features.shell_tool".to_string(), false.into()),
        ("features.standalone_web_search".to_string(), false.into()),
        ("features.token_budget".to_string(), false.into()),
        ("features.tool_suggest".to_string(), false.into()),
        ("features.unified_exec".to_string(), false.into()),
        ("features.view_image".to_string(), false.into()),
        ("orchestrator.skills.enabled".to_string(), false.into()),
        ("skills.include_instructions".to_string(), false.into()),
        (
            "token_budget.use_history_notes_extension".to_string(),
            false.into(),
        ),
        (
            "tools.experimental_request_user_input.enabled".to_string(),
            false.into(),
        ),
        ("tools.update_plan.enabled".to_string(), false.into()),
        ("web_search".to_string(), "disabled".into()),
    ]);
    let response: ThreadStartResponse = tokio::time::timeout(STRUCTURED_TURN_TIMEOUT, async {
        // Fail closed if the remote-effective MCP configuration cannot be read.
        let effective_config: ConfigReadResponse = request_handle
            .request_typed(ClientRequest::ConfigRead {
                request_id: RequestId::String(format!("temporary-config-{}", Uuid::new_v4())),
                params: ConfigReadParams {
                    include_layers: false,
                    cwd: Some(cwd.clone()),
                },
            })
            .await?;
        let mut mcp_server_names = mcp_server_names
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(effective_mcp_servers) = effective_config
            .config
            .additional
            .get("mcp_servers")
            .and_then(Value::as_object)
        {
            mcp_server_names.extend(effective_mcp_servers.keys().cloned());
        }
        config.insert(
            "mcp_servers".to_string(),
            Value::Object(
                mcp_server_names
                    .into_iter()
                    .map(|name| (name, serde_json::json!({ "enabled": false })))
                    .collect(),
            ),
        );

        request_handle
            .request_typed(ClientRequest::ThreadStart {
                request_id: RequestId::String(format!("temporary-structured-{}", Uuid::new_v4())),
                params: ThreadStartParams {
                    model: Some(model),
                    model_provider: Some(model_provider),
                    cwd: Some(cwd),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox: custom_permission_profile
                        .is_none()
                        .then_some(SandboxMode::ReadOnly),
                    permissions: custom_permission_profile.clone(),
                    runtime_workspace_roots: Some(Vec::new()),
                    ephemeral: Some(true),
                    thread_source: Some(ThreadSource::Feature("system".to_string())),
                    environments: Some(Vec::new()),
                    dynamic_tools: Some(Vec::new()),
                    selected_capability_roots: Some(Vec::new()),
                    config: Some(config),
                    ..ThreadStartParams::default()
                },
            })
            .await
    })
    .await
    .map_err(|_| eyre!("temporary structured thread start timed out"))??;

    if let Some(expected_profile) = custom_permission_profile {
        if response
            .active_permission_profile
            .as_ref()
            .is_none_or(|profile| profile.id != expected_profile)
        {
            return Err(eyre!(
                "temporary structured thread did not preserve permission profile {expected_profile}"
            ));
        }
    } else if !matches!(response.sandbox, SandboxPolicy::ReadOnly { .. }) {
        return Err(eyre!(
            "temporary structured thread did not start with read-only permissions"
        ));
    }

    Ok(response)
}

/// Submit a structured turn using the temporary thread's existing settings.
pub(crate) async fn start_structured_turn(
    request_handle: &AppServerRequestHandle,
    thread_id: String,
    prompt: String,
    output_schema: Value,
    effort: Option<ReasoningEffort>,
) -> Result<TurnStartResponse, TypedRequestError> {
    request_handle
        .request_typed(ClientRequest::TurnStart {
            request_id: RequestId::String(format!("temporary-structured-turn-{}", Uuid::new_v4())),
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                output_schema: Some(output_schema),
                effort,
                ..TurnStartParams::default()
            },
        })
        .await
}

/// Return the latest assistant message when the requested turn completes.
pub(crate) async fn collect_structured_response(
    mut notifications: UnboundedReceiver<ServerNotification>,
    turn_id: &str,
) -> color_eyre::Result<String> {
    let mut response = None;

    while let Some(notification) = notifications.recv().await {
        match notification {
            ServerNotification::ItemCompleted(completed) if completed.turn_id == turn_id => {
                if let ThreadItem::AgentMessage { text, .. } = completed.item {
                    if text.len() > STRUCTURED_RESPONSE_MAX_BYTES {
                        return Err(eyre!(
                            "temporary structured response exceeds {STRUCTURED_RESPONSE_MAX_BYTES} bytes"
                        ));
                    }
                    response = Some(text);
                }
            }
            ServerNotification::TurnCompleted(completed) if completed.turn.id == turn_id => {
                if completed.turn.status != TurnStatus::Completed {
                    return Err(eyre!(
                        "temporary structured turn ended with status {:?}",
                        completed.turn.status,
                    ));
                }

                return response.ok_or_else(|| {
                    eyre!("temporary structured turn completed without a response")
                });
            }
            _ => {}
        }
    }

    Err(eyre!(
        "temporary structured turn notification channel closed"
    ))
}

/// Make a bounded best-effort attempt to detach an ephemeral thread.
pub(crate) async fn unsubscribe_temporary_thread(
    request_handle: &AppServerRequestHandle,
    thread_id: String,
) {
    match tokio::time::timeout(
        STRUCTURED_CLEANUP_TIMEOUT,
        request_handle.request_typed::<ThreadUnsubscribeResponse>(
            ClientRequest::ThreadUnsubscribe {
                request_id: RequestId::String(format!(
                    "temporary-structured-unsubscribe-{}",
                    Uuid::new_v4()
                )),
                params: ThreadUnsubscribeParams { thread_id },
            },
        ),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::debug!(%error, "failed to unsubscribe from temporary structured thread");
        }
        Err(_) => {
            tracing::debug!("temporary structured thread unsubscribe timed out");
        }
    }
}

async fn interrupt_temporary_structured_turn(
    request_handle: &AppServerRequestHandle,
    thread_id: String,
    turn_id: String,
) {
    match tokio::time::timeout(
        STRUCTURED_CLEANUP_TIMEOUT,
        request_handle.request_typed::<TurnInterruptResponse>(ClientRequest::TurnInterrupt {
            request_id: RequestId::String(format!(
                "temporary-structured-interrupt-{}",
                Uuid::new_v4()
            )),
            params: TurnInterruptParams { thread_id, turn_id },
        }),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::debug!(%error, "failed to interrupt temporary structured turn");
        }
        Err(_) => {
            tracing::debug!("temporary structured turn interrupt timed out");
        }
    }
}

async fn cleanup_temporary_structured_turn(
    request_handle: AppServerRequestHandle,
    thread_id: String,
    turn_id: Option<String>,
    interrupt: bool,
) {
    if interrupt {
        interrupt_temporary_structured_turn(
            &request_handle,
            thread_id.clone(),
            turn_id.unwrap_or_default(),
        )
        .await;
    }
    unsubscribe_temporary_thread(&request_handle, thread_id).await;
}

async fn run_temporary_structured_turn_with_timeout(
    request_handle: AppServerRequestHandle,
    request: TemporaryStructuredTurnRequest,
    timeout: Duration,
) -> color_eyre::Result<String> {
    let TemporaryStructuredTurnRequest {
        thread_id,
        prompt,
        output_schema,
        effort,
        notifications,
    } = request;
    let mut cleanup =
        TemporaryStructuredTurnCleanup::new(request_handle.clone(), thread_id.clone());
    let result = tokio::time::timeout(timeout, async {
        let turn = start_structured_turn(
            &request_handle,
            thread_id.clone(),
            prompt,
            output_schema,
            effort,
        )
        .await?;

        cleanup.turn_id = Some(turn.turn.id.clone());
        collect_structured_response(notifications, &turn.turn.id).await
    })
    .await
    .unwrap_or_else(|_| Err(eyre!("temporary structured turn timed out")));

    cleanup.finish(result.is_err()).await;
    result
}

/// Run a bounded structured turn and make a bounded temporary-thread cleanup attempt.
pub(crate) async fn run_temporary_structured_turn(
    request_handle: AppServerRequestHandle,
    thread_id: String,
    prompt: String,
    output_schema: Value,
    effort: Option<ReasoningEffort>,
    notifications: UnboundedReceiver<ServerNotification>,
) -> color_eyre::Result<String> {
    run_temporary_structured_turn_with_timeout(
        request_handle,
        TemporaryStructuredTurnRequest {
            thread_id,
            prompt,
            output_schema,
            effort,
            notifications,
        },
        STRUCTURED_TURN_TIMEOUT,
    )
    .await
}

#[cfg(test)]
#[path = "temporary_structured_request_tests.rs"]
mod tests;
