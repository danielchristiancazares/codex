//! Applies complete model-picker selections to runtime and persistent settings.

use super::App;
use crate::app_event::AppEvent;
use crate::app_event::ModelSelectionScope;
use crate::app_server_session::AppServerSession;
use codex_protocol::openai_models::ReasoningEffort;

impl App {
    pub(super) async fn commit_model_selection(
        &mut self,
        app_server: &mut AppServerSession,
        model: String,
        effort: Option<ReasoningEffort>,
        context_window: Option<i64>,
        scope: ModelSelectionScope,
    ) {
        let persisted_effort = if effort == Some(ReasoningEffort::Ultra) {
            self.on_apply_advanced_reasoning(&model, ReasoningEffort::Ultra)
        } else {
            self.config.model = Some(model.clone());
            self.chat_widget.set_model(&model);
            self.on_update_reasoning_effort(effort.clone());
            effort.clone()
        };
        if scope == ModelSelectionScope::GlobalAndPlan {
            self.on_update_plan_mode_reasoning_effort(effort.clone());
            self.app_event_tx
                .send(AppEvent::PersistPlanModeReasoningEffort(effort.clone()));
        }
        if let Some(context_window) = context_window {
            self.config.model_context_window = Some(context_window);
            self.chat_widget
                .set_model_context_window(Some(context_window));
        }

        self.sync_active_thread_model_setting(app_server, model.clone(), effort)
            .await;
        self.sync_active_thread_service_tier_to_cached_session()
            .await;

        // The app-server-backed batch write also reloads the active session's numeric context
        // window. Keeping the context edit in this existing persistence event makes the next turn
        // observe the selection without introducing a separate request shape.
        self.app_event_tx.send(AppEvent::PersistModelSelection {
            model,
            effort: persisted_effort,
            context_window,
        });
    }
}
