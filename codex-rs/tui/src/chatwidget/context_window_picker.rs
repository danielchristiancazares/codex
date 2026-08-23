//! Numeric context-window picker stage for model selection.

use super::*;
use crate::app_event::ModelSelectionScope;
use codex_protocol::num_format::format_with_separators;

fn usable_context_windows(preset: &ModelPreset) -> Option<(i64, i64)> {
    let normal = preset.context_window?;
    let maximum = preset.max_context_window?;
    (normal > 0 && maximum > normal).then_some((normal, maximum))
}

impl ChatWidget {
    pub(super) fn model_supports_context_window_selection(&self, model: &str) -> bool {
        self.model_catalog.try_list_models().is_ok_and(|models| {
            models
                .iter()
                .find(|preset| preset.model == model)
                .and_then(usable_context_windows)
                .is_some()
        })
    }

    pub(crate) fn open_context_window_picker(
        &mut self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        scope: ModelSelectionScope,
    ) {
        let windows = self
            .model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .iter()
                    .find(|preset| preset.model == model)
                    .and_then(usable_context_windows)
            });
        let Some((normal, maximum)) = windows else {
            self.app_event_tx.send(AppEvent::CommitModelSelection {
                model,
                effort,
                context_window: None,
                scope,
            });
            return;
        };

        let current_context_window = self.config.model_context_window;
        let initial_selected_idx = usize::from(current_context_window == Some(maximum));
        let items = [("Normal (default)", normal), ("Maximum", maximum)]
            .into_iter()
            .map(|(label, context_window)| {
                let model = model.clone();
                let effort = effort.clone();
                let warning = effort
                    .as_ref()
                    .and_then(|effort| self.ultra_reasoning_concurrency_warning(effort));
                let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                    tx.send(AppEvent::CommitModelSelection {
                        model: model.clone(),
                        effort: effort.clone(),
                        context_window: Some(context_window),
                        scope,
                    });
                    if let Some(warning) = warning.clone() {
                        tx.send(AppEvent::InsertHistoryCell(Box::new(
                            history_cell::new_warning_event(warning),
                        )));
                    }
                })];
                SelectionItem {
                    name: label.to_string(),
                    description: Some(format!(
                        "{} token context window",
                        format_with_separators(context_window)
                    )),
                    is_current: current_context_window == Some(context_window),
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(format!("Select Context Window for {model}")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx: Some(initial_selected_idx),
            ..Default::default()
        });
    }
}
