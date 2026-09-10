use super::*;
use crate::app::model_selection::ContextWindowSelection;
use crate::app::model_selection::ModelSelection;
use crate::app::model_selection::ModelSelectionScope;
use codex_protocol::openai_models::ContextWindowCapacity;
use codex_protocol::openai_models::ModelContextWindow;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn context_window_workflow_keeps_narrow_reasoning_choices_readable() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.4")).await;
    chat.open_reasoning_popup(get_available_model(&chat, "gpt-5.4"));
    assert_chatwidget_snapshot!(
        "model_reasoning_selection_popup_narrow",
        render_bottom_popup(&chat, /*width*/ 40)
    );
}

#[tokio::test]
async fn context_window_picker_preserves_settings_until_capacity_is_selected() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.4")).await;
    let mut preset = get_available_model(&chat, "gpt-5.4");
    preset.context_window = ModelContextWindow::try_from(272_000).expect("normal capacity");
    preset.max_context_window = ModelContextWindow::try_from(1_000_000).expect("maximum capacity");
    chat.model_catalog = Arc::new(ModelCatalog::new(vec![preset]));
    chat.config.model_context_window = Some(272_000);
    while events.try_recv().is_ok() {}

    chat.open_context_window_picker(
        ModelSelection::new(
            "gpt-5.4",
            ReasoningEffortConfig::High,
            ModelSelectionScope::Global,
        )
        .expect("valid model selection"),
    );
    assert_chatwidget_snapshot!(
        "context_window_picker",
        render_bottom_popup(&chat, /*width*/ 60)
    );
    assert_eq!(chat.config.model_context_window, Some(272_000));
    assert!(events.try_recv().is_err());

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::CommitModelSelection(commit) = events.try_recv().expect("selection event") else {
        panic!("expected a complete model selection");
    };
    assert_eq!(
        commit,
        ModelSelection::new(
            "gpt-5.4",
            ReasoningEffortConfig::High,
            ModelSelectionScope::Global
        )
        .expect("valid model selection")
        .commit(ContextWindowSelection::capacity(
            ContextWindowCapacity::try_from(1_000_000).expect("maximum capacity")
        ))
    );
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn context_window_picker_cancellation_preserves_the_current_selection() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.4")).await;
    let mut preset = get_available_model(&chat, "gpt-5.4");
    preset.context_window = ModelContextWindow::try_from(272_000).expect("normal capacity");
    preset.max_context_window = ModelContextWindow::try_from(1_000_000).expect("maximum capacity");
    chat.model_catalog = Arc::new(ModelCatalog::new(vec![preset]));
    chat.config.model_context_window = Some(1_000_000);
    while events.try_recv().is_ok() {}

    chat.open_context_window_picker(
        ModelSelection::new(
            "gpt-5.4",
            ReasoningEffortConfig::High,
            ModelSelectionScope::Global,
        )
        .expect("valid model selection"),
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));

    assert_eq!(chat.config.model_context_window, Some(1_000_000));
    assert!(events.try_recv().is_err());
}
