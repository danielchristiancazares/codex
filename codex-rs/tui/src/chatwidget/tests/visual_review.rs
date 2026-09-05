//! Full-frame review fixtures use production widgets, including their actual cell styles.
//! Set CODEX_TUI_REVIEW_DIR to export the cells for pixel review alongside the snapshots.

use super::*;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::with_test_terminal_palette;
use crate::terminal_probe::DefaultColors;
use crate::test_support::export_visual_review_buffer;
use ratatui::buffer::Buffer;

#[tokio::test]
async fn visual_review_gallery() {
    for (palette, fg, bg, level) in [
        (
            "dark",
            (220, 220, 216),
            (32, 32, 32),
            StdoutColorLevel::TrueColor,
        ),
        (
            "light",
            (36, 36, 36),
            (250, 249, 246),
            StdoutColorLevel::TrueColor,
        ),
        (
            "ansi16",
            (220, 220, 216),
            (32, 32, 32),
            StdoutColorLevel::Ansi16,
        ),
    ] {
        for (width, height) in [(120_u16, 36_u16), (80, 24), (48, 20)] {
            for scene in [
                "welcome",
                "draft",
                "working",
                "loading",
                "approval",
                "error",
                "commands",
                "conversation",
            ] {
                let (mut chat, mut rx, _ops) = make_chatwidget_manual(Some("gpt-5.6-sol")).await;
                chat.config.cwd = test_path_buf("/work/atlas").abs();
                chat.current_cwd = Some(chat.config.cwd.to_path_buf());
                chat.local_settings.tui.animations = false;
                chat.refresh_status_line();
                if matches!(scene, "working" | "approval" | "error" | "conversation") {
                    complete_user_message(
                        &mut chat,
                        "review-request",
                        "Review the retry logic, fix the missing error handling, and verify the patch.",
                    );
                }
                if scene == "working" {
                    handle_turn_started(&mut chat, "review-turn");
                    for text in [
                        "Check the narrow layout.",
                        "Review the keyboard controls.",
                        "Keep the workspace portable.",
                        "Include the light theme.",
                    ] {
                        chat.queue_user_message(text.into());
                    }
                    set_active_cell(
                        &mut chat,
                        Box::new(history_cell::AgentMessageCell::new(
                            vec!["I'll refine the composer and check every terminal width.".into()],
                            /*is_first_line*/ true,
                        )),
                    );
                } else if scene == "approval" {
                    handle_turn_started(&mut chat, "review-turn");
                    let cwd = chat.config.cwd.clone();
                    handle_exec_approval_request(
                        &mut chat,
                        "review-approval",
                        ExecApprovalRequestEvent {
                            kind: Default::default(),
                            call_id: "verify-patch".into(),
                            approval_id: Some("verify-patch".into()),
                            turn_id: "review-turn".into(),
                            environment_id: None,
                            command: vec![
                                "just".into(),
                                "test".into(),
                                "-p".into(),
                                "atlas-client".into(),
                            ],
                            cwd,
                            reason: Some(
                                "Run the focused client tests to verify the retry fix.".into(),
                            ),
                            network_approval_context: None,
                            proposed_execpolicy_amendment: None,
                            proposed_network_policy_amendments: None,
                            additional_permissions: None,
                            available_decisions: None,
                        },
                    );
                } else if scene == "error" {
                    set_active_cell(&mut chat, Box::new(history_cell::new_error_event("Connection interrupted after 3 retries. Your draft is preserved. Check the connection and try again.".to_string())));
                    chat.handle_paste("Continue with the retry fix.".to_string());
                } else if scene == "conversation" {
                    let mut command = crate::exec_cell::ExecCell::new(
                        crate::exec_cell::ExecCall {
                            call_id: "verify-patch".into(),
                            command: vec![
                                "just".into(),
                                "test".into(),
                                "-p".into(),
                                "atlas-client".into(),
                            ],
                            parsed: Vec::new(),
                            output: None,
                            source: codex_app_server_protocol::CommandExecutionSource::Agent,
                            start_time: Some(Instant::now()),
                            duration: None,
                            interaction_input: None,
                        },
                        /*animations_enabled*/ false,
                    );
                    command.complete_call(
                        "verify-patch",
                        crate::exec_cell::CommandOutput::new(
                            /*exit_code*/ 0,
                            "running 4 tests\n4 passed; 0 failed".to_string(),
                        ),
                        Duration::from_secs(3),
                    );
                    chat.app_event_tx
                        .send(AppEvent::InsertHistoryCell(Box::new(command)));
                    let mut response = Vec::new();
                    crate::markdown::append_markdown(
                        "## Retry handling fixed\n\nThe timeout path now records the original error before returning the cached response.\n\n- Added a regression case for the timeout branch.\n- Preserved the existing backoff and cache behavior.\n- **Verification:** 4 focused tests passed.\n\nThe patch is ready for your review.",
                        Some(width.saturating_sub(2) as usize),
                        /*cwd*/ None,
                        &mut response,
                    );
                    set_active_cell(
                        &mut chat,
                        Box::new(history_cell::AgentMessageCell::new(
                            response, /*is_first_line*/ true,
                        )),
                    );
                } else if scene == "loading" {
                    let header = history_cell::SessionHeaderHistoryCell::new(
                        crate::text_formatting::format_model_status_label(chat.model_display_name()),
                        chat.effective_reasoning_effort(),
                        /*show_fast_status*/ false,
                        chat.config.cwd.to_path_buf(),
                        crate::version::CODEX_CLI_VERSION,
                    );
                    set_active_cell(&mut chat, Box::new(header));
                } else {
                    chat.thread_id = Some(ThreadId::new());
                    if scene == "draft" {
                        chat.handle_paste("Make this workspace feel beautifully considered.\nKeep the interface calm and the next action clear.".to_string());
                    }
                    if scene == "commands" {
                        chat.handle_paste("/".to_string());
                    }
                }
                let history = drain_insert_history_cells(&mut rx);
                let screen = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                let mut buffer = Buffer::empty(screen);
                let mut render_frame = |buffer: &mut Buffer| with_test_terminal_palette(DefaultColors { fg, bg }, level, || {
                    chat.refresh_status_line();
                    let landing = chat.landing_surface();
                    let renderable: &dyn Renderable = landing
                        .as_ref()
                        .map_or(&chat as &dyn Renderable, |surface| surface);
                    let desired = if landing.is_some() {
                        height
                    } else {
                        chat.desired_height(width).min(height)
                    };
                    let area = Rect::new(/*x*/ 0, height - desired, width, desired);
                    let history_lines = history
                        .iter()
                        .flat_map(|cell| {
                            cell.display_lines(width)
                                .into_iter()
                                .chain(std::iter::once(Line::default()))
                        })
                        .collect::<Vec<_>>();
                    let visible = history_lines.len().min(usize::from(area.y));
                    for (row, line) in history_lines
                        .iter()
                        .skip(history_lines.len() - visible)
                        .enumerate()
                    {
                        Renderable::render(
                            line,
                            Rect::new(
                                /*x*/ 0,
                                area.y - visible as u16 + row as u16,
                                width,
                                /*height*/ 1,
                            ),
                            buffer,
                        );
                    }
                    renderable.render(area, buffer);
                    if let Some((x, y)) = renderable.cursor_pos(area) {
                        buffer[(x, y)].set_style(Style::default().reversed());
                    }
                });
                render_frame(&mut buffer);
                let name = format!("{scene}_{palette}_{width}x{height}");
                let text = (0..height)
                    .map(|y| {
                        (0..width)
                            .map(|x| buffer[(x, y)].symbol())
                            .collect::<String>()
                            .trim_end()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                insta::assert_snapshot!(name.as_str(), crate::test_support::sanitize_codex_version(&normalize_snapshot_paths(text)));
                if std::env::var_os("CODEX_TUI_REVIEW_DIR").is_some() {
                    let mut native_buffer = Buffer::empty(screen);
                    crate::key_hint::with_test_native_key_labels(|| render_frame(&mut native_buffer));
                    with_test_terminal_palette(DefaultColors { fg, bg }, level, || {
                        export_visual_review_buffer(&name, &native_buffer);
                    });
                }
            }
        }
    }
}
