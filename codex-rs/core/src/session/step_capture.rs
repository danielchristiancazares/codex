//! Captures immutable execution steps and retains only selected contexts for a turn.

use super::session::Session;
use super::step_context::StepContext;
use super::step_settings::ResolvedStepSettings;
use super::token_budget;
use super::turn;
use super::turn_context::TurnContext;
use codex_async_utils::OrCancelExt;
use codex_features::Feature;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

impl Session {
    /// Publishes a selected step's metadata and retains it for the matching active turn.
    pub(crate) async fn set_last_known_step_context(&self, step_context: &Arc<StepContext>) {
        let turn_context = &step_context.turn;
        turn_context
            .extension_data
            .insert(step_context.selected_plugins.clone());
        // Candidate preparation must not replace the selected plan's inventory.
        let tool_namespaces_info = if turn_context
            .config
            .tool_registry
            .turn_metadata_includes_tool_info
        {
            step_context
                .tool_router
                .tool_namespaces_info()
                .cloned()
                .unwrap_or_default()
        } else {
            Default::default()
        };
        turn_context
            .turn_metadata_state
            .set_tool_namespaces_info(tool_namespaces_info);
        let turn_state = {
            let active_turn = self.active_turn.lock().await;
            active_turn.as_ref().and_then(|active_turn| {
                active_turn
                    .task
                    .as_ref()
                    .filter(|task| task.turn_context.sub_id == step_context.turn.sub_id)
                    .map(|_| Arc::clone(&active_turn.turn_state))
            })
        };
        if let Some(turn_state) = turn_state {
            turn_state.lock().await.last_known_step_context = Some(Arc::clone(step_context));
        }
    }

    /// Captures one request-scoped view of dynamic state and retains it for the active turn.
    ///
    /// This may refresh filesystem-derived state. Normal turns should call it only from
    /// `run_turn` and pass the result down; standalone request or history boundaries may capture
    /// their own step. Use speculative capture for a step that may not execute.
    pub(crate) async fn capture_step_context(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: &CancellationToken,
    ) -> CodexResult<Arc<StepContext>> {
        self.capture_step_context_with_required_mcp_servers(
            turn_context,
            cancellation_token,
            /*required_servers*/ &[],
        )
        .await
    }

    pub(crate) async fn capture_step_context_with_required_mcp_servers(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: &CancellationToken,
        required_servers: &[String],
    ) -> CodexResult<Arc<StepContext>> {
        let step_context = self
            .capture_step_context_inner(turn_context, cancellation_token, required_servers)
            .await?;
        self.set_last_known_step_context(&step_context).await;
        Ok(step_context)
    }

    /// Prepares a candidate step without replacing the active turn's retained context.
    /// The caller must retain it explicitly if it is selected for execution.
    pub(super) async fn capture_speculative_step_context(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: &CancellationToken,
    ) -> CodexResult<Arc<StepContext>> {
        self.capture_step_context_inner(
            turn_context,
            cancellation_token,
            /*required_servers*/ &[],
        )
        .await
    }

    #[tracing::instrument(name = "step_context.capture", level = "info", skip_all)]
    async fn capture_step_context_inner(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: &CancellationToken,
        required_servers: &[String],
    ) -> CodexResult<Arc<StepContext>> {
        // Capture once before asynchronous planning; all request consumers
        // retain this immutable settings version even if the turn is updated.
        let mut settings = turn_context.current_settings.load_full();
        if matches!(
            turn_context.session_source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        ) {
            let root_service_tier = self.services.agent_control.root_service_tier();
            if settings.selected().service_tier != root_service_tier {
                let mut selected = settings.selected().clone();
                selected.service_tier = root_service_tier;
                settings = Arc::new(ResolvedStepSettings::new(
                    Arc::new(selected),
                    Arc::clone(&settings.model_info),
                    self.features.enabled(Feature::FastMode),
                ));
            }
        }
        let token_budget = token_budget::resolve_token_budget(
            turn_context.configured_token_budget.as_ref(),
            turn_context.use_model_token_budget_defaults,
            settings.model_info.as_ref(),
        );
        let session_telemetry = settings.telemetry(&turn_context.session_telemetry);
        // Keep selections fixed for the turn while allowing their startup work to finish.
        let environments = turn_context.environments.refresh_readiness();
        self.services
            .agents_md_manager
            .refresh(&turn_context.config, &environments)
            .await?;
        let loaded_agents_md = self.services.agents_md_manager.get_loaded().await;
        let selected_capability_roots = self
            .resolve_selected_capability_roots_for_step(&environments)
            .await;
        let ready_selected_capability_roots =
            Self::ready_selected_capability_roots(&selected_capability_roots);
        let executor_capability_discovery = self
            .executor_capability_discovery_for_step(
                &turn_context.config,
                &ready_selected_capability_roots,
                &environments,
            )
            .or_cancel(cancellation_token)
            .await?;
        let extension_data = codex_extension_api::ExtensionData::new(turn_context.sub_id.clone());
        extension_data.insert(selected_capability_roots.clone());
        if let Some(discovery) = &executor_capability_discovery {
            extension_data.insert(discovery.as_ref().clone());
            if !discovery.sandbox_contexts().is_empty() {
                extension_data.insert(discovery.sandbox_contexts().clone());
            }
        } else if !environments
            .permission_profile_or_else(|| turn_context.permission_profile())
            .file_system_sandbox_policy()
            .has_full_disk_read_access()
        {
            let sandbox_contexts = environments
                .turn_environments()
                .map(|environment| {
                    (
                        environment.selection.environment_id.clone(),
                        environment.sandbox_context(/*additional_permissions*/ None),
                    )
                })
                .collect::<HashMap<_, _>>();
            extension_data.insert(sandbox_contexts);
        }
        let (mcp, prepared_recommendations) = async {
            tokio::join!(
                self.mcp_runtime_for_step(
                    turn_context.as_ref(),
                    &selected_capability_roots,
                    required_servers,
                ),
                turn::prepare_tool_recommendations(self.as_ref(), turn_context.as_ref()),
            )
        }
        .or_cancel(cancellation_token)
        .await?;
        let mut selected_plugins = self
            .services
            .thread_extension_data
            .get::<codex_extension_api::SelectedPluginSnapshot>()
            .map(|snapshot| snapshot.as_ref().clone())
            .unwrap_or_default();
        selected_plugins.plugins.retain(|plugin| {
            ready_selected_capability_roots
                .iter()
                .any(|root| root.id == plugin.selected_root_id)
        });
        extension_data.insert(selected_plugins.clone());
        // Tool planning still uses the admitted turn. Migrating it to the
        // captured model is a separate step from diagnostic activation.
        let tool_router = turn::built_tools(
            self.as_ref(),
            turn_context.as_ref(),
            // TODO(CDXENT-441): use the step scoped model
            turn_context.model_info(),
            &environments,
            &mcp,
            &extension_data,
            prepared_recommendations,
        )
        .or_cancel(cancellation_token)
        .await??;
        Ok(Arc::new(StepContext {
            settings,
            token_budget,
            session_telemetry,
            turn: turn_context,
            environments,
            selected_capability_roots,
            selected_plugins,
            executor_capability_discovery,
            mcp,
            tool_router,
            loaded_agents_md,
        }))
    }
}
