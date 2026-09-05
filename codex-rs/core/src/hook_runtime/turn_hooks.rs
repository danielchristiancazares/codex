//! Terminal turn hooks use selected settings and executor discovery.
//! Turns with no captured step retain their initial local-hook context.

use super::emit_hook_completed_events;
use super::emit_hook_started_events;
use super::hook_permission_mode;
use super::subagent_hook_context;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::step_settings::ResolvedStepSettings;
use crate::session::turn_context::TurnContext;
use crate::state::TurnState;
use crate::turn_metadata::McpTurnMetadataContext;
use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_core_plugins::executor_plugin_hook_sources;
use codex_hooks::InterruptRequest;
use codex_hooks::StopHookTarget;
use codex_hooks::StopOutcome;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_plugin::ExecutorPluginHookSource;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_thread_store::ReadThreadParams;
use serde_json::Map;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

fn build_request_metadata(
    settings: &ResolvedStepSettings,
    turn_context: &TurnContext,
) -> Map<String, Value> {
    turn_context
        .turn_metadata_state
        .current_meta_value_for_mcp_request(McpTurnMetadataContext {
            model: settings.model_info.slug.as_str(),
            reasoning_effort: settings.effective_reasoning_effort(),
            node_repl_disabled: settings.model_info.node_repl_disabled,
        })
        .map(|turn_metadata| {
            Map::from_iter([(
                crate::X_CODEX_TURN_METADATA_HEADER.to_string(),
                turn_metadata,
            )])
        })
        .unwrap_or_default()
}

fn executor_hook_sources_for_step(step_context: &StepContext) -> Vec<ExecutorPluginHookSource> {
    step_context
        .executor_capability_discovery
        .as_deref()
        .map(|snapshot| {
            let app_tool_policy =
                AppToolPolicyEvaluator::new(&step_context.mcp.config().config_layer_stack);
            executor_plugin_hook_sources(snapshot, |server, tool| {
                step_context
                    .mcp
                    .tool_info(server, tool)
                    .filter(|tool_info| {
                        if server != CODEX_APPS_MCP_SERVER_NAME {
                            return true;
                        }
                        let annotations = tool_info.tool.annotations.as_ref();
                        app_tool_policy
                            .policy(AppToolPolicyInput {
                                connector_id: tool_info.connector_id.as_deref(),
                                link_id: None,
                                tool_name: &tool_info.tool.name,
                                tool_title: tool_info.tool.title.as_deref(),
                                destructive_hint: annotations
                                    .and_then(|annotations| annotations.destructive_hint),
                                open_world_hint: annotations
                                    .and_then(|annotations| annotations.open_world_hint),
                            })
                            .enabled
                    })
            })
        })
        .unwrap_or_default()
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn run_turn_stop_hooks(
    sess: &Arc<Session>,
    step_context: &Arc<StepContext>,
    stop_hook_active: bool,
    last_assistant_message: Option<String>,
) -> StopOutcome {
    let turn_context = &step_context.turn;
    // Resolve the stop hook kind from the session source before building the
    // request. Root turns run Stop; thread-spawned child turns run SubagentStop.
    let (target, transcript_path) = match &turn_context.session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            agent_role,
            parent_thread_id,
            ..
        }) => {
            let context = subagent_hook_context(sess, agent_role);
            let agent_transcript_path = sess.hook_transcript_path().await;
            let parent_transcript_path = match sess
                .services
                .thread_store
                .read_thread(ReadThreadParams {
                    thread_id: *parent_thread_id,
                    include_archived: true,
                    include_history: false,
                })
                .await
            {
                Ok(thread) => thread.rollout_path,
                Err(error) => {
                    tracing::warn!(
                        parent_thread_id = %parent_thread_id,
                        error = %error,
                        "failed to resolve parent transcript path for subagent hook"
                    );
                    None
                }
            };
            (
                StopHookTarget::SubagentStop {
                    agent_id: context.agent_id,
                    agent_type: context.agent_type,
                    agent_transcript_path,
                },
                parent_transcript_path,
            )
        }
        // Internal/synthetic subagents do not expose user-configured lifecycle
        // hooks, so there is no Stop or SubagentStop request to dispatch.
        SessionSource::SubAgent(_) => return StopOutcome::default(),
        SessionSource::Internal(InternalSessionSource::MemoryConsolidation) => (
            StopHookTarget::MemoryConsolidation,
            sess.hook_transcript_path().await,
        ),
        _ => (StopHookTarget::Stop, sess.hook_transcript_path().await),
    };
    let settings = step_context.settings.as_ref();
    let request_metadata = build_request_metadata(settings, turn_context);
    let request = codex_hooks::StopRequest {
        session_id: sess.session_id().into(),
        turn_id: turn_context.sub_id.clone(),
        #[allow(deprecated)]
        cwd: turn_context.cwd.clone(),
        transcript_path,
        model: settings.model_info.slug.clone(),
        permission_mode: hook_permission_mode(settings.approval_policy()),
        request_metadata: (!request_metadata.is_empty()).then_some(request_metadata),
        stop_hook_active,
        last_assistant_message,
        target,
    };
    let executor_hook_sources = executor_hook_sources_for_step(step_context);
    let hooks = sess.hooks().with_executor_hooks(executor_hook_sources);
    emit_hook_started_events(sess, turn_context, hooks.preview_stop(&request)).await;

    let mut outcome = hooks.run_stop(request).await;
    emit_hook_completed_events(sess, turn_context, std::mem::take(&mut outcome.hook_events)).await;
    outcome
}

pub(crate) async fn run_turn_interrupt_hooks(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    turn_state: &Mutex<TurnState>,
) {
    if matches!(&turn_context.session_source, SessionSource::SubAgent(_)) {
        return;
    }

    // The active turn has already been detached. Reuse only its last executing step's discovery.
    let last_known_step_context = turn_state.lock().await.last_known_step_context.clone();
    let executor_hook_sources = last_known_step_context
        .as_ref()
        .map(|step_context| executor_hook_sources_for_step(step_context))
        .unwrap_or_default();
    let has_executor_hooks = !executor_hook_sources.is_empty();
    let hooks = sess.hooks().with_executor_hooks(executor_hook_sources);
    let preview_runs = hooks.preview_interrupt();
    if preview_runs.is_empty() && !has_executor_hooks {
        return;
    }

    let settings = last_known_step_context
        .as_ref()
        .map(|step_context| step_context.settings.as_ref())
        .unwrap_or(turn_context.initial_settings.as_ref());
    let request_metadata = build_request_metadata(settings, turn_context);
    let request = InterruptRequest {
        session_id: sess.session_id().into(),
        turn_id: turn_context.sub_id.clone(),
        #[allow(deprecated)]
        cwd: turn_context.cwd.clone(),
        transcript_path: sess.hook_transcript_path().await,
        model: settings.model_info.slug.clone(),
        permission_mode: hook_permission_mode(settings.approval_policy()),
        request_metadata: (!request_metadata.is_empty()).then_some(request_metadata),
    };
    if let Err(err) = sess.flush_rollout().await {
        tracing::warn!("failed to flush transcript before Interrupt hook: {err}");
    }
    emit_hook_started_events(sess, turn_context, preview_runs).await;

    let outcome = hooks.run_interrupt(request).await;
    emit_hook_completed_events(sess, turn_context, outcome.hook_events).await;
}
