//! Commits model-picker selections through one settings and persistence operation.

use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use crate::config_update::format_config_error;
use crate::legacy_core::config::Config;
use codex_app_server_protocol::ConfigEdit;
use codex_protocol::openai_models::ContextWindowCapacity;
use codex_protocol::openai_models::ReasoningEffort;

#[derive(Clone, Debug, PartialEq, Eq)]
struct NonEmptyString(String);

impl NonEmptyString {
    fn new(model: &str) -> Result<Self, InvalidModelSelection> {
        if model.trim().is_empty() {
            return Err(InvalidModelSelection);
        }
        Ok(Self(model.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the selected model must have a nonempty name")]
pub(crate) struct InvalidModelSelection;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ModelSelectionScope {
    #[default]
    Global,
    GlobalAndPlan,
}

impl ModelSelectionScope {
    pub(crate) fn append_plan_edit(self, edits: &mut Vec<ConfigEdit>, edit: ConfigEdit) {
        match self {
            Self::Global => {}
            Self::GlobalAndPlan => edits.push(edit),
        }
    }

    fn apply(self, app: &mut App, effort: Option<ReasoningEffort>) {
        match self {
            Self::Global => {}
            Self::GlobalAndPlan => app.on_update_plan_mode_reasoning_effort(effort),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ContextWindowSelection(WindowChange);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum WindowChange {
    #[default]
    KeepConfigured,
    SetCapacity(ContextWindowCapacity),
}

impl ContextWindowSelection {
    pub(crate) fn capacity(capacity: ContextWindowCapacity) -> Self {
        Self(WindowChange::SetCapacity(capacity))
    }

    pub(crate) fn apply(self, config: &mut Config) {
        match self.0 {
            WindowChange::KeepConfigured => {}
            WindowChange::SetCapacity(capacity) => {
                config.model_context_window = Some(capacity.tokens())
            }
        }
    }

    pub(crate) fn append_edits(self, edits: &mut Vec<ConfigEdit>) {
        match self.0 {
            WindowChange::KeepConfigured => {}
            WindowChange::SetCapacity(capacity) => {
                edits.push(crate::config_update::replace_config_value(
                    "model_context_window",
                    serde_json::json!(capacity.tokens()),
                ))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelSelection {
    model: NonEmptyString,
    effort: Option<ReasoningEffort>,
    scope: ModelSelectionScope,
}

impl ModelSelection {
    pub(crate) fn new(
        model: &str,
        effort: ReasoningEffort,
        scope: ModelSelectionScope,
    ) -> Result<Self, InvalidModelSelection> {
        Ok(Self {
            model: NonEmptyString::new(model)?,
            effort: Some(effort),
            scope,
        })
    }

    pub(crate) fn with_default_reasoning(
        model: &str,
        scope: ModelSelectionScope,
    ) -> Result<Self, InvalidModelSelection> {
        Ok(Self {
            model: NonEmptyString::new(model)?,
            effort: None,
            scope,
        })
    }

    pub(crate) fn model(&self) -> &str {
        &self.model.0
    }

    pub(crate) fn effort(&self) -> Option<&ReasoningEffort> {
        self.effort.as_ref()
    }

    pub(crate) fn commit(self, context_window: ContextWindowSelection) -> ModelSelectionCommit {
        ModelSelectionCommit {
            selection: self,
            context_window,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ModelSelectionCommit {
    selection: ModelSelection,
    context_window: ContextWindowSelection,
}

impl App {
    pub(super) async fn commit_model_selection(
        &mut self,
        app_server: &mut AppServerSession,
        commit: ModelSelectionCommit,
    ) {
        let ModelSelectionCommit {
            selection,
            context_window,
        } = commit;
        let ModelSelection {
            model: NonEmptyString(model),
            effort,
            scope,
        } = selection;
        // Model changes keep the current permission-profile transaction and cyber-model policy.
        if self
            .active_thread_model_setting_update_params(model.clone())
            .is_some_and(|params| params.permissions.is_some())
            && self.reject_pending_permission_change()
        {
            return;
        }

        let model_changed = self.chat_widget.current_model() != model
            || self.chat_widget.current_collaboration_mode().model() != model;
        let defaults = if effort == Some(ReasoningEffort::Ultra) {
            // Ultra remains conversation-scoped; the saved ordinary effort is preserved.
            ModelDefaults::Conversation(
                self.on_apply_advanced_reasoning(&model, ReasoningEffort::Ultra),
            )
        } else {
            self.config.model = Some(model.clone());
            self.chat_widget.set_model(&model);
            self.on_update_reasoning_effort(effort.clone());
            ModelDefaults::Save(effort.clone())
        };
        scope.apply(self, effort.clone());
        context_window.apply(&mut self.config);
        self.chat_widget
            .apply_context_window_selection(context_window);

        if model_changed {
            self.sync_active_thread_model_setting(app_server, model.clone(), effort.clone())
                .await;
        } else if let Some(mut params) =
            self.active_thread_reasoning_setting_update_params(effort)
        {
            params.collaboration_mode = Some(self.chat_widget.effective_collaboration_mode());
            self.send_thread_settings_update(app_server, params).await;
        }
        self.sync_active_thread_service_tier_to_cached_session()
            .await;
        defaults
            .persist(
                self,
                app_server,
                NonEmptyString(model),
                context_window,
                scope,
            )
            .await;
    }
}

enum ModelDefaults {
    Save(Option<ReasoningEffort>),
    Conversation(Option<ReasoningEffort>),
}

impl ModelDefaults {
    async fn persist(
        self,
        app: &mut App,
        app_server: &AppServerSession,
        model: NonEmptyString,
        context_window: ContextWindowSelection,
        scope: ModelSelectionScope,
    ) {
        match self {
            Self::Save(effort) => app.app_event_tx.send(AppEvent::PersistModelSelection {
                model: model.0,
                effort,
                context_window,
                scope,
            }),
            Self::Conversation(default_effort) => {
                let model = model.0;
                let mut edits = default_effort.as_ref().map_or_else(Vec::new, |effort| {
                    crate::config_update::build_model_selection_edits(&model, Some(effort))
                });
                context_window.append_edits(&mut edits);
                if let Err(error) = app
                    .persist_model_defaults(
                        app_server.request_handle(),
                        edits,
                        "default model and reasoning effort",
                    )
                    .await
                {
                    let error = format_config_error(&error);
                    tracing::error!(%error, "failed to persist conversation model");
                    app.chat_widget
                        .add_error_message(format!("Failed to save default model: {error}"));
                } else {
                    app.chat_widget.add_info_message(
                        format!("Model changed to {model} ultra for this conversation"),
                        /*hint*/ None,
                    );
                }
            }
        }
    }
}
