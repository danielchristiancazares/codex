use super::*;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use crate::test_support::export_visual_review_buffer;
use crate::test_support::sanitize_codex_version;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn startup_visual_review_gallery() {
    for (palette, fg, bg) in [
        ("dark", (220, 220, 216), (32, 32, 32)),
        ("light", (36, 36, 36), (250, 249, 246)),
    ] {
        for (width, height) in [(120, 36), (48, 20), (18, 12)] {
            let (tx, _rx) = unbounded_channel();
            let pane = startup_draft_bottom_pane(
                AppEventSender::new(tx),
                FrameRequester::test_dummy(),
                /*enhanced_keys_supported*/ false,
            );
            let header = startup_session_header(/*config*/ None);
            let renderable =
                startup_draft_renderable(&header, &pane, StartupDraftSessionAction::New);
            let mut buffer = Buffer::empty(Rect::new(/*x*/ 0, /*y*/ 0, width, height));
            let name = format!("startup_{palette}_{width}x{height}");
            with_test_default_colors(DefaultColors { fg, bg }, || {
                let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                renderable.render(area, &mut buffer);
                export_visual_review_buffer(&name, &buffer);
            });
            insta::assert_snapshot!(
                name.as_str(),
                sanitize_codex_version(&format!("{buffer:?}"))
            );
        }
    }
}

#[tokio::test]
async fn startup_to_ready_keeps_the_draft_cursor_in_place() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("startup config");
    let header = startup_session_header(Some(&config));
    for text in [
        "Review the retry fix",
        "Review the retry fix\nKeep this draft intact.",
    ] {
        let (tx, _rx) = unbounded_channel();
        let mut pane = startup_draft_bottom_pane(
            AppEventSender::new(tx),
            FrameRequester::test_dummy(),
            /*enhanced_keys_supported*/ false,
        );
        pane.set_composer_text(text.to_string(), Vec::new(), Vec::new());
        let (mut chat, _tx, _rx, _ops) =
            crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
        chat.handle_thread_session(crate::session_state::ThreadSessionState {
            thread_id: codex_protocol::ThreadId::new(),
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: "gpt-5.6-sol".into(),
            model_provider_id: "openai".into(),
            service_tier: codex_protocol::config_types::ServiceTier::Default,
            approval_policy: codex_app_server_protocol::AskForApproval::OnRequest,
            approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer::User,
            permission_profile: codex_protocol::models::PermissionProfile::workspace_write(),
            active_permission_profile: None,
            cwd: config.cwd.clone(),
            runtime_workspace_roots: Vec::new(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: Some(codex_protocol::openai_models::ReasoningEffort::High),
            reasoning_mode: codex_protocol::config_types::ReasoningMode::Standard,
            collaboration_mode: None,
            personality: None,
            message_history: None,
            network_proxy: None,
            rollout_path: None,
        });
        chat.restore_startup_draft(pane.composer_draft_snapshot());
        for (width, height) in [(120, 36), (80, 24), (48, 20)] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let startup = startup_draft_renderable(&header, &pane, StartupDraftSessionAction::New);
            let ready = chat.landing_surface().expect("ready empty session");
            pretty_assertions::assert_eq!(
                startup.cursor_pos(area),
                ready.cursor_pos(area),
                "{width}x{height}: {text}"
            );
            pretty_assertions::assert_eq!(chat.composer_text_with_pending(), text);
        }
    }
}
