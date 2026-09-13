//! The optional numeric-capacity stage, after model/effort and Plan scope selection.

use super::*;
use crate::app::model_selection::ContextWindowSelection;
use crate::app::model_selection::ModelSelection;
use crate::app::model_selection::ModelSelectionScope;
use codex_protocol::num_format::format_with_separators;
use codex_protocol::openai_models::ContextWindowCapacity;
use codex_protocol::openai_models::ModelContextWindow;

impl ChatWidget {
    fn catalog_context_windows(&self, model: &str) -> (ModelContextWindow, ModelContextWindow) {
        match self.model_catalog.try_list_models() {
            Ok(models) => match models.into_iter().find(|preset| preset.model == model) {
                Some(preset) => (preset.context_window, preset.max_context_window),
                None => (ModelContextWindow::default(), ModelContextWindow::default()),
            },
            Err(_) => (ModelContextWindow::default(), ModelContextWindow::default()),
        }
    }

    pub(super) fn context_window_stage(
        &self,
        model: &str,
        effort: Option<ReasoningEffortConfig>,
        scope: ModelSelectionScope,
    ) -> Vec<SelectionAction> {
        let selection = match effort {
            Some(effort) => ModelSelection::new(model, effort, scope),
            None => ModelSelection::with_default_reasoning(model, scope),
        };
        let selection = match selection {
            Ok(selection) => selection,
            Err(error) => {
                return vec![Box::new(move |tx: &AppEventSender| {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event(error.to_string()),
                    )));
                })];
            }
        };
        let warning = selection
            .effort()
            .and_then(|effort| self.ultra_reasoning_concurrency_warning(effort));
        let (normal, maximum) = self.catalog_context_windows(model);
        let choose_capacity = normal.select(maximum, || false, |_, _| true);
        vec![Box::new(move |tx: &AppEventSender| {
            if choose_capacity {
                tx.send(AppEvent::OpenContextWindowPicker(selection.clone()));
            } else {
                tx.send(AppEvent::CommitModelSelection(
                    selection.clone().commit(ContextWindowSelection::default()),
                ));
                if let Some(warning) = warning.clone() {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_warning_event(warning),
                    )));
                }
            }
        })]
    }

    pub(crate) fn open_context_window_picker(&mut self, selection: ModelSelection) {
        let (normal, maximum) = self.catalog_context_windows(selection.model());
        let choice = selection.clone();
        // A refreshed catalog can remove a capacity pair between the two picker stages.
        normal
            .select(
                maximum,
                || ContextWindowStep::CommitConfigured(selection),
                |normal, maximum| ContextWindowStep::Choose {
                    selection: choice,
                    normal,
                    maximum,
                },
            )
            .present(self);
    }

    pub(crate) fn apply_context_window_selection(&mut self, selection: ContextWindowSelection) {
        selection.apply(&mut self.config);
    }
}

enum ContextWindowStep {
    CommitConfigured(ModelSelection),
    Choose {
        selection: ModelSelection,
        normal: ContextWindowCapacity,
        maximum: ContextWindowCapacity,
    },
}

impl ContextWindowStep {
    fn present(self, chat: &mut ChatWidget) {
        match self {
            Self::CommitConfigured(selection) => chat.app_event_tx.send(
                AppEvent::CommitModelSelection(selection.commit(ContextWindowSelection::default())),
            ),
            Self::Choose {
                selection: choice,
                normal,
                maximum,
            } => {
                let current_context_window = chat.config.model_context_window;
                let items = [("Normal (default)", normal), ("Maximum", maximum)]
                    .into_iter()
                    .map(|(label, capacity)| {
                        let selection = choice.clone();
                        let warning = selection
                            .effort()
                            .and_then(|effort| chat.ultra_reasoning_concurrency_warning(effort));
                        let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                            tx.send(AppEvent::CommitModelSelection(
                                selection
                                    .clone()
                                    .commit(ContextWindowSelection::capacity(capacity)),
                            ));
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
                                format_with_separators(capacity.tokens())
                            )),
                            is_current: current_context_window == Some(capacity.tokens()),
                            actions,
                            dismiss_on_select: true,
                            ..Default::default()
                        }
                    })
                    .collect();
                chat.bottom_pane.show_selection_view(SelectionViewParams {
                    title: Some(format!("Select Context Window for {}", choice.model())),
                    footer_hint: Some(standard_popup_hint_line()),
                    items,
                    initial_selected_idx: Some(usize::from(
                        current_context_window == Some(maximum.tokens()),
                    )),
                    ..Default::default()
                });
            }
        }
    }
}
