//! Model-provider selection for the active conversation.

use super::*;
use crate::history_cell::McpInventoryLoadingCell;
use codex_model_provider_info::COPILOT_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_ID;

pub(super) const PROVIDER_SELECTION_VIEW_ID: &str = "provider-selection";
pub(super) const PROVIDER_SWITCH_LOADING_VIEW_ID: &str = "provider-switch-loading";

impl ChatWidget {
    pub(crate) fn can_switch_model_provider(&self, thread_id: ThreadId) -> bool {
        self.thread_id == Some(thread_id)
            && !self.active_side_conversation
            && !self.blocks_direct_input
            && !self.config.ephemeral
            && self.unified_exec_processes.is_empty()
            && !self.is_user_turn_pending_or_running()
            && self
                .transcript
                .active_cell
                .as_ref()
                .is_none_or(|cell| !cell.as_any().is::<McpInventoryLoadingCell>())
    }

    pub(crate) fn open_provider_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Provider selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let providers = [
            (OPENAI_PROVIDER_ID, "OpenAI", "Use your OpenAI account"),
            (
                COPILOT_PROVIDER_ID,
                "GitHub Copilot",
                "Use your GitHub Copilot account over WebSocket",
            ),
        ];
        let current_provider = self.config.model_provider_id.as_str();
        let mut initial_selected_idx = None;
        let items = providers
            .into_iter()
            .enumerate()
            .map(|(index, (provider_id, name, description))| {
                let is_current = provider_id == current_provider;
                if is_current {
                    initial_selected_idx = Some(index);
                }
                SelectionItem {
                    name: name.to_string(),
                    description: Some(description.to_string()),
                    is_current,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SwitchModelProvider(provider_id.to_string()));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!("{name} {provider_id}")),
                    ..Default::default()
                }
            })
            .collect();

        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(PROVIDER_SELECTION_VIEW_ID),
            title: Some("Select Provider".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    pub(crate) fn show_provider_switch_loading(&mut self, provider_name: &str) {
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(PROVIDER_SWITCH_LOADING_VIEW_ID),
            title: Some("Switching Provider".to_string()),
            subtitle: Some(format!("Preparing {provider_name} for this conversation.")),
            items: vec![SelectionItem {
                name: "Loading models and session state...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            footer_hint: Some("Press esc to hide; switching continues".dim().into()),
            ..Default::default()
        });
    }

    pub(crate) fn finish_provider_switch_loading(&mut self) {
        self.bottom_pane
            .dismiss_view_by_id(PROVIDER_SWITCH_LOADING_VIEW_ID);
    }
}
