//! Exercise the app-owned empty-history boundary, including the full-screen-to-inline handoff.

use super::*;
use codex_protocol::config_types::ApprovalsReviewer;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn landing_frame_yields_to_warning_and_restores_inline_history() {
    let mut app = crate::app::test_support::make_test_app().await;
    let session = ThreadSessionState {
        thread_id: ThreadId::new(),
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-5.6-sol".to_string(),
        model_provider_id: "openai".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::workspace_write(),
        active_permission_profile: None,
        cwd: app.config.cwd.clone(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::High),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: None,
    };
    let header = history_cell::new_session_info(
        &app.config,
        &app.local_settings,
        &session.model,
        &session,
        /*is_first_event*/ true,
        /*tooltip_override*/ None,
        /*auth_plan*/ None,
        /*show_fast_status*/ false,
    );
    app.chat_widget.handle_thread_session(session);
    app.transcript_cells.push(Arc::new(header));
    app.chat_widget.handle_paste("An unsent draft".to_string());
    let mut tui = crate::tui::test_support::make_test_tui().expect("terminal");
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    let area = app
        .render_chat_widget_frame(&mut tui, size)
        .expect("landing frame");
    assert_eq!(
        area,
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24
        )
    );
    let draft_cursor = app
        .chat_widget
        .landing_surface()
        .expect("ready landing")
        .cursor_pos(area);
    app.chat_widget.open_model_popup_with_presets(
        app.model_catalog
            .try_list_models()
            .expect("available models"),
    );
    let popup_area = app
        .render_chat_widget_frame(&mut tui, size)
        .expect("empty-session model picker");
    assert_eq!(popup_area, area);
    let popup_text = {
        let popup = app
            .chat_widget
            .landing_surface()
            .expect("selection landing");
        assert!(popup.cursor_pos(popup_area).is_none());
        let mut buffer = ratatui::buffer::Buffer::empty(popup_area);
        popup.render(popup_area, &mut buffer);
        buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
    };
    assert!(popup_text.contains("Select Model"));
    assert!(!popup_text.contains("Make something worth keeping."));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let restored_area = app
        .render_chat_widget_frame(&mut tui, size)
        .expect("restored landing");
    assert_eq!(restored_area, area);
    assert_eq!(
        app.chat_widget
            .landing_surface()
            .expect("ready after closing picker")
            .cursor_pos(restored_area),
        draft_cursor
    );
    app.transcript_cells
        .push(Arc::new(history_cell::new_warning_event(
            "Workspace needs attention".to_string(),
        )));
    assert!(
        app.render_landing_frame(&mut tui, size)
            .expect("warning boundary")
            .is_none()
    );
    let area = app
        .render_chat_widget_frame(&mut tui, size)
        .expect("inline conversation");
    assert!(area.height < size.height);
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "An unsent draft"
    );
    assert_eq!(app.transcript_cells.len(), 2);
}
