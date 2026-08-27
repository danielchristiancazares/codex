use super::*;
use crate::app_event::ModelSelectionScope;

fn preset_with_windows(
    chat: &ChatWidget,
    context_window: Option<i64>,
    max_context_window: Option<i64>,
) -> ModelPreset {
    let mut preset = get_available_model(chat, "gpt-5.6-sol");
    preset.context_window = context_window;
    preset.max_context_window = max_context_window;
    preset
}

#[tokio::test]
async fn context_window_picker_two_windows_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
    let preset = preset_with_windows(
        &chat,
        /*context_window*/ Some(400_000),
        /*max_context_window*/ Some(922_000),
    );
    chat.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset]));
    chat.set_model_context_window(Some(922_000));

    chat.open_context_window_picker(
        "gpt-5.6-sol".to_string(),
        /*effort*/ Some(ReasoningEffortConfig::High),
        ModelSelectionScope::Global,
    );

    assert_chatwidget_snapshot!(
        "context_window_picker_two_windows",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn context_window_picker_emits_normal_and_maximum_numeric_choices() {
    for (current_context_window, key_events, expected_context_window) in [
        (None, Vec::new(), 400_000),
        (Some(400_000), Vec::new(), 400_000),
        (Some(400_000), vec![KeyEvent::from(KeyCode::Down)], 922_000),
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
        let preset = preset_with_windows(
            &chat,
            /*context_window*/ Some(400_000),
            /*max_context_window*/ Some(922_000),
        );
        chat.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset]));
        chat.set_model_context_window(current_context_window);
        while rx.try_recv().is_ok() {}

        chat.open_context_window_picker(
            "gpt-5.6-sol".to_string(),
            /*effort*/ Some(ReasoningEffortConfig::High),
            ModelSelectionScope::Global,
        );
        for key_event in key_events {
            chat.handle_key_event(key_event);
        }
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

        assert_matches!(
            rx.try_recv(),
            Ok(AppEvent::CommitModelSelection {
                model,
                effort: Some(ReasoningEffortConfig::High),
                context_window: Some(context_window),
                scope: ModelSelectionScope::Global,
            }) if model == "gpt-5.6-sol" && context_window == expected_context_window
        );
    }
}

#[tokio::test]
async fn context_window_picker_escape_preserves_active_selection() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
    let preset = preset_with_windows(
        &chat,
        /*context_window*/ Some(400_000),
        /*max_context_window*/ Some(922_000),
    );
    chat.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset]));
    chat.set_model("gpt-5.4");
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Medium));
    chat.set_model_context_window(Some(400_000));
    while rx.try_recv().is_ok() {}

    chat.open_context_window_picker(
        "gpt-5.6-sol".to_string(),
        /*effort*/ Some(ReasoningEffortConfig::High),
        ModelSelectionScope::Global,
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));

    assert_eq!(chat.current_model(), "gpt-5.4");
    assert_eq!(
        chat.effective_reasoning_effort(),
        Some(ReasoningEffortConfig::Medium)
    );
    assert_eq!(chat.config.model_context_window, Some(400_000));
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .all(|event| !matches!(event, AppEvent::CommitModelSelection { .. }))
    );
}

#[tokio::test]
async fn context_window_picker_skips_unusable_catalog_windows() {
    for (context_window, max_context_window) in [
        (None, None),
        (Some(400_000), None),
        (None, Some(922_000)),
        (Some(0), Some(922_000)),
        (Some(400_000), Some(400_000)),
        (Some(922_000), Some(400_000)),
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
        let mut preset = preset_with_windows(&chat, context_window, max_context_window);
        preset.default_reasoning_effort = ReasoningEffortConfig::Medium;
        preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
            effort: ReasoningEffortConfig::Medium,
            description: "Balanced reasoning".to_string(),
        }];
        chat.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset.clone()]));
        while rx.try_recv().is_ok() {}

        chat.open_reasoning_popup(preset);

        let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            events.iter().any(
                |event| matches!(event, AppEvent::UpdateModel(model) if model == "gpt-5.6-sol")
            )
        );
        assert!(events.iter().any(|event| matches!(
            event,
            AppEvent::UpdateReasoningEffort(Some(ReasoningEffortConfig::Medium))
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            AppEvent::PersistModelSelection {
                model,
                effort: Some(ReasoningEffortConfig::Medium),
                context_window: None,
            } if model == "gpt-5.6-sol"
        )));
        assert!(events.iter().all(|event| !matches!(
            event,
            AppEvent::OpenContextWindowPicker { .. } | AppEvent::CommitModelSelection { .. }
        )));
        assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("Select Context Window"));
    }
}

#[tokio::test]
async fn single_reasoning_choice_advances_to_context_window_stage() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
    let mut preset = preset_with_windows(
        &chat,
        /*context_window*/ Some(400_000),
        /*max_context_window*/ Some(922_000),
    );
    preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
        effort: ReasoningEffortConfig::Medium,
        description: "Balanced reasoning".to_string(),
    }];
    chat.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset.clone()]));
    while rx.try_recv().is_ok() {}

    chat.open_reasoning_popup(preset);

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenContextWindowPicker {
            model,
            effort: Some(ReasoningEffortConfig::Medium),
            scope: ModelSelectionScope::Global,
        }) if model == "gpt-5.6-sol"
    );
}
