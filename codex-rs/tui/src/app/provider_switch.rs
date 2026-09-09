//! History-preserving model-provider transitions for local sessions.

use super::session_lifecycle::ThreadAttachPresentation;
use super::*;
use crate::app_server_session::ForkGoalContinuation::DeferUntilNextTurn;
use crate::chatwidget::ThreadInputStateRestoreMode;
use crate::connection_switch::SwitchHandoff;
use crate::connection_switch::SwitchTarget;
use crate::history_cell::McpInventoryLoadingCell as LoadingCell;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ConnectionSelection;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadBackgroundTerminalsListParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsListResponse;
use codex_protocol::openai_models::ReasoningEffort;
use std::time::Duration;

const PROVIDER_SWITCH_PREPARATION_TIMEOUT: Duration = Duration::from_secs(30);

async fn prepare_model_provider_switch(
    request_handle: AppServerRequestHandle,
    tracked_thread_ids: Vec<ThreadId>,
    target: SwitchTarget,
) -> std::result::Result<Vec<ModelPreset>, String> {
    let terminal_request_handle = request_handle.clone();
    let background_terminal_check = async move {
        let mut checks = tokio::task::JoinSet::new();
        for thread_id in tracked_thread_ids {
            let request_handle = terminal_request_handle.clone();
            checks.spawn(async move {
                request_handle
                    .request_typed::<ThreadBackgroundTerminalsListResponse>(
                        ClientRequest::ThreadBackgroundTerminalsList {
                            request_id: RequestId::String(format!(
                                "provider-terminal-check-{}",
                                Uuid::new_v4()
                            )),
                            params: ThreadBackgroundTerminalsListParams {
                                thread_id: thread_id.to_string(),
                                cursor: None,
                                limit: Some(1),
                            },
                        },
                    )
                    .await
            });
        }
        while let Some(result) = checks.join_next().await {
            if !matches!(result, Ok(Ok(response)) if response.data.is_empty()) {
                return Err("Active background terminals block provider switching.".to_string());
            }
        }
        Ok(())
    };
    let models_request = async move {
        target
            .prepare(request_handle)
            .await
            .map_err(|error| format!("Could not load models for `{}`: {error}", target.name()))
    };
    let ((), models) = tokio::try_join!(background_terminal_check, models_request)?;
    Ok(models)
}

pub(super) async fn reconcile_session_model_environment(
    config: &mut Config,
    app_server: &mut AppServerSession,
    session: &ThreadSessionState,
    available_models: &mut Vec<ModelPreset>,
) -> Result<()> {
    // Remote configuration belongs to the server. Ordinary resumes retain upstream's handling
    // of model IDs that may no longer appear in the picker catalog.
    if app_server.uses_remote_workspace() || config.model_provider_id == session.model_provider_id {
        return Ok(());
    }

    let provider_id = session.model_provider_id.as_str();
    let provider = config
        .model_providers
        .get(provider_id)
        .cloned()
        .ok_or_else(|| color_eyre::eyre::eyre!("Model provider `{provider_id}` is unavailable."))?;
    let models = app_server
        .list_models_for_provider(provider_id.to_string())
        .await?;
    if models.is_empty() {
        return Err(color_eyre::eyre::eyre!(
            "Model provider `{provider_id}` returned no available models."
        ));
    }
    if !models.iter().any(|model| model.model == session.model) {
        return Err(color_eyre::eyre::eyre!(
            "Model `{}` is unavailable from provider `{provider_id}`.",
            session.model
        ));
    }
    *available_models = models;

    let selected_model = available_models
        .iter()
        .find(|model| model.model == session.model)
        .ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "Model `{}` is unavailable from provider `{provider_id}`.",
                session.model
            )
        })?;
    let plan_effort_is_supported =
        config
            .plan_mode_reasoning_effort
            .as_ref()
            .is_some_and(|effort| {
                selected_model
                    .supported_reasoning_efforts
                    .iter()
                    .any(|preset| &preset.effort == effort)
            });
    if !plan_effort_is_supported {
        config.plan_mode_reasoning_effort = Some(selected_model.default_reasoning_effort.clone());
    }
    config.model_provider_id = provider_id.to_string();
    config.model_provider = provider;
    config.model = Some(session.model.clone());
    config.model_reasoning_effort = session.reasoning_effort.clone();
    config.service_tier.clone_from(&session.service_tier);
    app_server.set_active_model_catalog(session.model.clone(), available_models.clone());
    Ok(())
}

impl App {
    pub(super) fn start_model_provider_switch(
        &mut self,
        app_server: &AppServerSession,
        target: SwitchTarget,
    ) {
        if matches!(
            target.selection(&self.config.model_provider_id),
            ConnectionSelection::Retain
        ) {
            self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
            return;
        }
        let provider_id = target.provider_id().to_owned();
        if self.pending_provider_switch.is_some() {
            self.chat_widget.add_info_message(
                "A provider switch is already in progress.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if self.app_server_target.uses_remote_workspace() {
            self.chat_widget.add_error_message(
                "Provider switching is unavailable for remote workspaces.".to_string(),
            );
            return;
        }
        let Some(thread_id) = self.chat_widget.thread_id() else {
            return;
        };
        if self.primary_thread_id != Some(thread_id)
            || !self.chat_widget.can_switch_model_provider(thread_id)
        {
            self.chat_widget.add_error_message(
                "Changing providers requires an idle primary session.".to_string(),
            );
            return;
        }
        if self
            .transcript_cells
            .iter()
            .any(|cell| cell.as_any().is::<LoadingCell>())
        {
            self.chat_widget
                .add_error_message("MCP inventory is still loading.".to_string());
            return;
        }
        if !self.config.model_providers.contains_key(&provider_id) {
            self.chat_widget
                .add_error_message(format!("Model provider `{provider_id}` is unavailable."));
            return;
        }

        let agents = self.agent_navigation.ordered_threads();
        let another_thread_is_active = self.thread_event_channels.iter().any(|(id, channel)| {
            let store = channel.store.try_lock();
            *id != thread_id && !store.is_ok_and(|store| store.active_turn_id().is_none())
        });
        if another_thread_is_active
            || agents
                .iter()
                .any(|(id, agent)| *id != thread_id && agent.is_running)
        {
            self.chat_widget.add_error_message(
                "Cannot change providers while another agent is running.".to_string(),
            );
            return;
        }

        let rollout = self.chat_widget.rollout_path();
        let has_rollout = rollout.as_deref().is_some_and(rollout_path_is_resumable);
        let channels = &self.thread_event_channels;
        if !has_rollout
            && (!self.chat_widget.token_usage().is_zero()
                || channels.get(&thread_id).is_some_and(|channel| {
                    channel.store.try_lock().map_or(/*default*/ true, |store| {
                        !store.turns.is_empty()
                            || store.buffer.iter().any(|event| {
                                matches!(
                                    event,
                                    ThreadBufferedEvent::Notification(notification)
                                        if matches!(notification.as_ref(),
                                            ServerNotification::TurnStarted(_)
                                            | ServerNotification::TurnCompleted(_))
                                )
                            })
                    })
                }))
        {
            self.chat_widget
                .add_error_message("Conversation history is not saved.".to_string());
            return;
        }

        let mut tracked_ids: HashSet<_> = channels.keys().copied().collect();
        for (id, agent) in &agents {
            tracked_ids.extend((!agent.is_closed).then_some(*id));
        }
        let descendants = tracked_ids
            .iter()
            .copied()
            .filter(|tracked_id| *tracked_id != thread_id)
            .collect::<Vec<_>>();
        let tracked_thread_ids = std::iter::once(thread_id).chain(descendants).collect();
        let request_id = Uuid::new_v4();
        self.pending_provider_switch = Some(request_id);
        self.chat_widget.show_provider_switch_loading(target.name());
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = tokio::time::timeout(
                PROVIDER_SWITCH_PREPARATION_TIMEOUT,
                prepare_model_provider_switch(request_handle, tracked_thread_ids, target.clone()),
            )
            .await
            .unwrap_or_else(|_| {
                Err(format!(
                    "Timed out while preparing model provider `{provider_id}`."
                ))
            });
            app_event_tx.send(AppEvent::ModelProviderSwitchPrepared(
                request_id, thread_id, target, result,
            ));
        });
    }

    pub(super) async fn complete_model_provider_switch(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        request_id: Uuid,
        thread_id: ThreadId,
        target: SwitchTarget,
        result: std::result::Result<Vec<ModelPreset>, String>,
    ) {
        if self.pending_provider_switch != Some(request_id) {
            return;
        }
        let provider_id = target.provider_id().to_owned();
        if self.chat_widget.thread_id() != Some(thread_id)
            || self.primary_thread_id != Some(thread_id)
            || !self.chat_widget.can_switch_model_provider(thread_id)
        {
            self.fail_model_provider_switch(
                "Changing providers requires an idle primary session.".to_string(),
            );
            return;
        }
        let provider = match self.config.model_providers.get(&provider_id).cloned() {
            Some(provider) => provider,
            None => {
                self.fail_model_provider_switch(format!(
                    "Model provider `{provider_id}` is unavailable."
                ));
                return;
            }
        };
        let available_models = match result {
            Ok(models) if !models.is_empty() => models,
            Ok(_) => {
                self.fail_model_provider_switch(format!(
                    "Model provider `{provider_id}` returned no available models."
                ));
                return;
            }
            Err(error) => {
                self.fail_model_provider_switch(error);
                return;
            }
        };
        let agents = self.agent_navigation.ordered_threads();
        let another_thread_is_active = self.thread_event_channels.iter().any(|(id, channel)| {
            let store = channel.store.try_lock();
            *id != thread_id && !store.is_ok_and(|store| store.active_turn_id().is_none())
        });
        if another_thread_is_active
            || agents
                .iter()
                .any(|(id, agent)| *id != thread_id && agent.is_running)
        {
            self.fail_model_provider_switch(
                "Cannot change providers while another agent is running.".to_string(),
            );
            return;
        }
        let mut tracked_ids: HashSet<_> = self.thread_event_channels.keys().copied().collect();
        for (id, agent) in agents {
            tracked_ids.extend((!agent.is_closed).then_some(id));
        }
        let rollout = self.chat_widget.rollout_path();
        let has_rollout = rollout.as_deref().is_some_and(rollout_path_is_resumable);
        let current_model = self.chat_widget.current_model();
        let Some(selected_model) = available_models
            .iter()
            .find(|model| model.model == current_model)
            .or_else(|| available_models.iter().find(|model| model.is_default))
            .or_else(|| available_models.first())
        else {
            self.fail_model_provider_switch(format!(
                "Model provider `{provider_id}` returned no available models."
            ));
            return;
        };
        let compatible_effort = |current: Option<ReasoningEffort>| {
            current
                .filter(|effort| {
                    selected_model
                        .supported_reasoning_efforts
                        .iter()
                        .any(|preset| &preset.effort == effort)
                })
                .unwrap_or_else(|| selected_model.default_reasoning_effort.clone())
        };
        let selected_model_id = selected_model.model.clone();
        let selected_effort = compatible_effort(
            self.chat_widget
                .current_collaboration_mode()
                .reasoning_effort(),
        );
        let selected_plan_effort =
            compatible_effort(self.config.plan_mode_reasoning_effort.clone());
        let input_state = self.chat_widget.capture_thread_input_state();

        let mut config = self.config.clone();
        config.model_provider_id = provider_id.clone();
        config.model_provider = provider.clone();
        config.model = Some(selected_model_id.clone());
        config.model_reasoning_effort = Some(selected_effort);
        config.plan_mode_reasoning_effort = Some(selected_plan_effort);
        config.service_tier = None;
        config.notices.fast_default_opt_out = Some(true);

        let handoff = match target.activate(app_server.request_handle()).await {
            Ok(handoff) => handoff,
            Err(error) => {
                self.fail_model_provider_switch(format!(
                    "Could not select {}: {error}",
                    target.name()
                ));
                return;
            }
        };

        let transitioned = if has_rollout {
            app_server
                .fork_thread_at(
                    &self.local_settings,
                    config.clone(),
                    thread_id,
                    /*last_turn_id*/ None,
                    /*before_turn_id*/ None,
                    DeferUntilNextTurn,
                    /*selected_profile*/ None,
                )
                .await
        } else {
            app_server
                .start_thread_with_session_start_source(
                    &self.local_settings,
                    &config,
                    /*session_start_source*/ None,
                    /*remote_cwd_override*/ None,
                    /*selected_profile*/ None,
                )
                .await
        };
        let transitioned = match transitioned {
            Ok(transitioned) => transitioned,
            Err(error) => {
                self.fail_connection_handoff(
                    handoff,
                    format!("Failed to change model provider: {error}"),
                )
                .await;
                return;
            }
        };
        let replacement_id = transitioned.session.thread_id;
        let actual_model = transitioned.session.model.clone();
        let valid_replacement = replacement_id != thread_id
            && transitioned.session.model_provider_id == provider_id
            && available_models
                .iter()
                .any(|model| model.model == actual_model);
        if !valid_replacement {
            let _ = app_server.thread_unsubscribe(replacement_id).await;
            let _ = app_server.thread_archive(replacement_id).await;
            self.fail_connection_handoff(
                handoff,
                "The replacement session did not apply the requested provider and model."
                    .to_string(),
            )
            .await;
            return;
        }
        if let Err(error) = app_server.thread_unsubscribe(thread_id).await {
            let _ = app_server.thread_unsubscribe(replacement_id).await;
            let _ = app_server.thread_archive(replacement_id).await;
            self.fail_connection_handoff(
                handoff,
                format!("Could not detach the previous provider session: {error}"),
            )
            .await;
            return;
        }
        for tracked_id in tracked_ids
            .into_iter()
            .filter(|tracked_id| *tracked_id != thread_id)
        {
            if let Err(error) = app_server.thread_unsubscribe(tracked_id).await {
                tracing::warn!("failed to unsubscribe tracked thread {tracked_id}: {error}");
            }
        }

        // The server-side replacement is now usable and the source detached.
        // Finish credential ownership before changing presentation state.
        if let Err(error) = handoff.commit().await {
            self.chat_widget.add_warning_message(format!("The replacement session is ready. Account-switch confirmation failed: {error}. Check /switch before continuing."));
        }
        config.model = Some(actual_model.clone());
        config.model_reasoning_effort = transitioned.session.reasoning_effort.clone();
        self.config = config;
        self.local_settings.notices.fast_default_opt_out = Some(true);
        self.model_catalog = Arc::new(
            ModelCatalog::new(available_models.clone())
                .with_collaboration_modes(self.model_catalog.collaboration_modes.clone()),
        );
        app_server.set_active_model_catalog(actual_model.clone(), available_models);
        if let Err(error) = self
            .replace_chat_widget_with_app_server_thread(
                tui,
                transitioned,
                ThreadAttachPresentation::SessionLineage,
                /*initial_user_message*/ None,
            )
            .await
        {
            self.chat_widget.restore_thread_input_state(
                input_state,
                ThreadInputStateRestoreMode {
                    preserve_in_flight_turn: false,
                },
            );
            self.fail_model_provider_switch(format!(
                "The account changed. Could not display session {replacement_id}: {error}. Use /resume to reopen it."
            ));
            return;
        }
        self.chat_widget.restore_thread_input_state(
            input_state,
            ThreadInputStateRestoreMode {
                preserve_in_flight_turn: false,
            },
        );
        self.chat_widget.set_model(&actual_model);
        self.chat_widget
            .set_reasoning_effort(self.config.model_reasoning_effort.clone());
        self.chat_widget
            .set_plan_mode_reasoning_effort(self.config.plan_mode_reasoning_effort.clone());
        self.pending_provider_switch = None;
        self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
        self.cancel_pending_key_chord();
        let mut persistence_edits = crate::config_update::build_model_selection_edits(
            &actual_model,
            self.config.model_reasoning_effort.clone(),
        );
        persistence_edits.insert(
            0,
            crate::config_update::replace_config_value(
                "model_provider",
                serde_json::json!(provider_id.clone()),
            ),
        );
        persistence_edits.extend(crate::config_update::build_service_tier_selection_edits(
            self.config.service_tier.as_deref(),
        ));
        persistence_edits.push(crate::config_update::replace_config_value(
            "notice.fast_default_opt_out",
            serde_json::json!(true),
        ));
        let plan_effort_edit = self.config.plan_mode_reasoning_effort.as_ref().map_or_else(
            || crate::config_update::clear_config_value("plan_mode_reasoning_effort"),
            |effort| {
                crate::config_update::replace_config_value(
                    "plan_mode_reasoning_effort",
                    serde_json::json!(effort.to_string()),
                )
            },
        );
        persistence_edits.push(plan_effort_edit);
        let persistence_warning = match crate::config_update::write_config_batch(
            app_server.request_handle(),
            persistence_edits,
        )
        .await
        {
            Ok(response) if response.status == WriteStatus::OkOverridden => Some(
                "The saved provider default is overridden by a higher-priority config layer."
                    .to_string(),
            ),
            Ok(_) => None,
            Err(error) => {
                tracing::error!(%error, "failed to persist provider selection");
                Some(format!("The provider default could not be saved: {error}"))
            }
        };
        self.chat_widget.add_info_message(
            format!("Switched to {} using {actual_model}.", target.name()),
            /*hint*/ None,
        );
        if let Some(warning) = persistence_warning {
            self.chat_widget.add_warning_message(warning);
        }
        tui.frame_requester().schedule_frame();
    }

    fn fail_model_provider_switch(&mut self, message: String) {
        self.pending_provider_switch = None;
        self.chat_widget.finish_provider_switch_loading();
        self.chat_widget.add_error_message(message);
        self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
    }

    async fn fail_connection_handoff(&mut self, handoff: SwitchHandoff, message: String) {
        match handoff.restore().await {
            Ok(()) => self.fail_model_provider_switch(message),
            Err(error) => self.fail_model_provider_switch(format!("{message} Account restoration also failed: {error}. Use /switch to select the account again.")),
        }
    }
}
