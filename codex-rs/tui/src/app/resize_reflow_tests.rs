use super::*;
use crate::app::test_support::make_test_app;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::PlainHistoryCell;
use crate::legacy_core::config::TerminalResizeReflowMaxRows;
use crate::transcript_reflow::TranscriptReplayPolicy;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use std::path::Path;
use std::path::PathBuf;

fn plain_history_cells(count: usize) -> Vec<Arc<dyn HistoryCell>> {
    (0..count)
        .map(|index| {
            Arc::new(PlainHistoryCell::new(vec![Line::from(format!(
                "cell {index}"
            ))])) as Arc<dyn HistoryCell>
        })
        .collect()
}

fn rendered_line_text(line: &HyperlinkLine) -> String {
    line.line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[tokio::test]
async fn resize_reflow_preserves_configured_scrollback_beyond_the_visible_viewport() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.transcript_cells = plain_history_cells(/*count*/ 64);
    let screen_size = Size::new(/*width*/ 80, /*height*/ 24);
    let chat_height = app.with_chat_widget_frame(screen_size.width, |height, _| height);
    let visible_history_rows = screen_size
        .height
        .saturating_sub(chat_height)
        .max(/*other*/ 1);

    app.update_visible_history_rows(screen_size);
    let rendered = app.render_transcript_lines_for_reflow(screen_size.width);

    assert_eq!(app.resize_reflow_max_rows(), Some(32));
    assert_eq!(rendered.lines.len(), 32);
    assert!(rendered.lines.len() > usize::from(visible_history_rows));
    assert_eq!(
        rendered.lines.last().map(rendered_line_text),
        Some("cell 63".to_string())
    );
}

#[tokio::test]
async fn initial_resume_replay_retains_scrollback_beyond_the_visible_viewport() -> Result<()> {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    let screen_size = Size::new(/*width*/ 80, /*height*/ 24);
    app.update_visible_history_rows(screen_size);
    let visible_history_rows = app
        .transcript_reflow
        .visible_history_rows()
        .expect("visible history row budget");

    app.begin_initial_history_replay_buffer();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for cell in plain_history_cells(/*count*/ 24) {
        app.insert_history_cell_lines_with_initial_replay_buffer(
            &mut tui,
            cell.as_ref(),
            screen_size.width,
        );
    }

    let retained_lines = &app
        .initial_history_replay_buffer
        .as_ref()
        .expect("initial replay buffer should remain active")
        .retained_lines;
    assert_eq!(retained_lines.len(), 32);
    assert!(retained_lines.len() > usize::from(visible_history_rows));
    assert!(
        app.initial_history_replay_buffer
            .as_ref()
            .is_some_and(|buffer| buffer.was_truncated)
    );
    insta::assert_snapshot!(
        retained_lines
            .iter()
            .rev()
            .take(/*n*/ 3)
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    cell 23

    cell 22
    "
    );
    Ok(())
}

#[tokio::test]
async fn resize_reflow_preserves_configured_scrollback_when_the_terminal_height_changes() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(48);
    app.transcript_cells = plain_history_cells(/*count*/ 64);

    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 24));
    let smaller = app.render_transcript_lines_for_reflow(/*width*/ 80);
    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 48));
    let larger = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(smaller.lines.len(), 48);
    assert_eq!(larger.lines.len(), 48);
    assert_eq!(
        smaller
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>(),
        larger
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        larger.lines.last().map(rendered_line_text),
        Some("cell 63".to_string())
    );
}

#[tokio::test]
async fn resize_reflow_preserves_explicitly_unlimited_history() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Disabled;
    app.transcript_cells = plain_history_cells(/*count*/ 20);

    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 24));
    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(app.resize_reflow_max_rows(), None);
    assert_eq!(rendered.lines.len(), 39);
    assert_eq!(
        rendered.lines.first().map(rendered_line_text),
        Some("cell 0".to_string())
    );
    assert_eq!(
        rendered.lines.last().map(rendered_line_text),
        Some("cell 19".to_string())
    );
}

#[tokio::test]
async fn capped_resize_reflow_prepends_transcript_notice_without_changing_transcript() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(8);
    app.transcript_cells = plain_history_cells(/*count*/ 12);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(rendered.lines.len(), 8);
    assert_eq!(app.transcript_cells.len(), 12);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are available — press ctrl + t to view the full transcript
    cell 8

    cell 9

    cell 10

    cell 11
    "
    );
}

#[tokio::test]
async fn capped_resize_reflow_counts_wrapped_notice_rows() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(8);
    app.transcript_cells = plain_history_cells(/*count*/ 12);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 28);

    assert_eq!(rendered.lines.len(), 8);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are
    available — press ctrl + t
    to view the full transcript
    cell 9

    cell 10

    cell 11
    "
    );
}

#[tokio::test]
async fn one_row_history_cap_preserves_conversation_instead_of_notice() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(1);
    app.scrollback_has_older_history = true;
    app.transcript_cells = plain_history_cells(/*count*/ 2);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(rendered.lines.len(), 1);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"cell 1"
    );
}

#[tokio::test]
async fn configured_pet_load_reflows_existing_transcript_before_next_draw() -> Result<()> {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Disabled;
    app.config.tui_pet = Some("test".to_string());
    app.transcript_cells = vec![Arc::new(AgentMarkdownCell::new(
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu".to_string(),
        Path::new("/tmp"),
    ))];
    app.chat_widget
        .set_pet_image_support_for_tests(crate::pets::PetImageSupport::Supported(
            crate::pets::ImageProtocol::Kitty,
        ));
    let screen_size = Size::new(/*width*/ 40, /*height*/ 12);
    let before = app.render_transcript_lines_for_reflow(screen_size.width);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.last_known_screen_size = screen_size;
    let pet =
        crate::pets::test_ambient_pet(tui.frame_requester(), /*animations_enabled*/ false);

    app.handle_configured_pet_loaded(&mut tui, "test".to_string(), Ok(Some(pet)))?;

    let reflowed = tui.pending_history_lines_for_test();
    assert!(
        reflowed.len() > before.lines.len(),
        "pet load should reflow existing transcript before the image is drawn"
    );
    insta::assert_snapshot!(
        "configured_pet_load_reflows_existing_transcript",
        reflowed
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

#[tokio::test]
async fn paginated_resize_reflow_prepends_transcript_notice_for_unloaded_history() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.scrollback_has_older_history = true;
    app.transcript_cells = plain_history_cells(/*count*/ 2);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are available — press ctrl + t to view the full transcript
    cell 0

    cell 1
    "
    );
}

#[tokio::test]
async fn scrollback_refill_only_loads_older_pages_for_an_underfilled_row_cap() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.scrollback_has_older_history = true;

    assert!(app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 32));

    app.scrollback_has_older_history = false;
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));

    app.scrollback_has_older_history = true;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Disabled;
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));
}

#[tokio::test]
async fn model_selection_stages_keep_inline_viewport_bottom_docked() -> Result<()> {
    let mut app = make_test_app().await;
    let presets = app
        .model_catalog
        .try_list_models()
        .expect("test model catalog");
    let reasoning_model = presets
        .iter()
        .find(|preset| preset.model == "gpt-5.6-sol")
        .cloned()
        .expect("reasoning model");
    app.chat_widget.open_model_popup_with_presets(presets);

    let screen_size = Size::new(/*width*/ 80, /*height*/ 10);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.last_known_screen_size = screen_size;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 6,
        screen_size.width,
        /*height*/ 4,
    ));
    tui.terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    let model_area = app.render_chat_widget_frame(&mut tui, screen_size)?;
    app.chat_widget.open_reasoning_popup(reasoning_model);
    let reasoning_area = app.render_chat_widget_frame(&mut tui, screen_size)?;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let restored_model_area = app.render_chat_widget_frame(&mut tui, screen_size)?;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let composer_area = app.render_chat_widget_frame(&mut tui, screen_size)?;

    assert_eq!(
        [
            model_area.bottom(),
            reasoning_area.bottom(),
            restored_model_area.bottom(),
            composer_area.bottom(),
        ],
        [screen_size.height; 4]
    );
    assert!(
        model_area.top() > 0,
        "model picker must preserve its bottom-docked viewport provenance: {model_area:?}"
    );
    assert_eq!(
        tui.terminal.docked_history_gap_rows(),
        0,
        "model popup transitions must not create blank rows above transcript history"
    );
    insta::assert_debug_snapshot!(
        "model_selection_stages_keep_inline_viewport_bottom_docked",
        [
            model_area,
            reasoning_area,
            restored_model_area,
            composer_area,
        ]
    );
    Ok(())
}

#[tokio::test]
async fn inline_resize_preserves_pending_history_and_acknowledges_width_and_height() -> Result<()> {
    let mut app = make_test_app().await;
    app.transcript_replay_policy = TranscriptReplayPolicy::InlinePreserveScrollback;
    app.chat_widget
        .set_transcript_replay_policy_for_tests(TranscriptReplayPolicy::InlinePreserveScrollback);
    app.transcript_cells = plain_history_cells(/*count*/ 3);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.insert_history_lines(vec![
        Line::from("pre-codex-shell-output"),
        Line::from("existing codex row"),
    ]);
    let pending_before = tui
        .pending_history_lines_for_test()
        .iter()
        .map(rendered_line_text)
        .collect::<Vec<_>>();

    let width_resize = Size::new(/*width*/ 64, /*height*/ 24);
    app.handle_draw_pre_render(&mut tui, width_resize)?;
    let height_resize = Size::new(/*width*/ 64, /*height*/ 18);
    tui.terminal.last_known_screen_size = width_resize;
    app.handle_draw_pre_render(&mut tui, height_resize)?;

    assert_eq!(
        tui.pending_history_lines_for_test()
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>(),
        pending_before,
        "inline resize must not purge shell or Codex rows awaiting terminal insertion"
    );
    assert!(!app.transcript_reflow.has_pending_reflow());
    assert!(!app.transcript_reflow.reflow_needed_for_width(/*width*/ 64));
    assert!(
        app.transcript_reflow.visible_history_rows().is_some(),
        "inline acknowledgement must preserve the cached visible-history budget"
    );
    Ok(())
}

#[tokio::test]
async fn owned_resize_still_schedules_source_backed_replay() -> Result<()> {
    let mut app = make_test_app().await;
    app.transcript_replay_policy = TranscriptReplayPolicy::OwnedBufferReplay;
    app.transcript_cells = plain_history_cells(/*count*/ 2);
    let tui = crate::tui::test_support::make_test_tui()?;
    let initial = Size::new(/*width*/ 80, /*height*/ 24);
    app.handle_draw_size_change(initial, initial, &tui.frame_requester());

    let resized = Size::new(/*width*/ 72, /*height*/ 20);
    assert!(app.handle_draw_size_change(resized, initial, &tui.frame_requester(),));
    assert!(app.transcript_reflow.has_pending_reflow());
    Ok(())
}

#[tokio::test]
async fn inline_mismatch_appends_one_terminal_only_correction_without_clearing_history()
-> Result<()> {
    let mut app = make_test_app().await;
    app.transcript_replay_policy = TranscriptReplayPolicy::InlinePreserveScrollback;
    app.chat_widget
        .set_transcript_replay_policy_for_tests(TranscriptReplayPolicy::InlinePreserveScrollback);
    app.transcript_cells = vec![Arc::new(AgentMessageCell::new(
        vec![Line::from("streamed provisional response")],
        /*is_first_line*/ true,
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.insert_history_lines(vec![Line::from("pre-codex-shell-output")]);

    app.handle_consolidate_agent_message(
        &mut tui,
        "Authoritative **corrected** response.".to_string(),
        PathBuf::from("/workspace"),
        /*inline_visualization_context*/ None,
        crate::app_event::ConsolidationScrollbackReflow::InlinePreserve(
            crate::app_event::InlineCanonicalCorrection::AppendAuthoritativeSource,
        ),
        /*deferred_history_cell*/ None,
    )?;

    assert_eq!(app.transcript_cells.len(), 1);
    assert!(
        app.transcript_cells[0].as_any().is::<AgentMarkdownCell>(),
        "canonical transcript should contain only the authoritative markdown cell"
    );
    let pending = tui
        .pending_history_lines_for_test()
        .iter()
        .map(rendered_line_text)
        .collect::<Vec<_>>();
    assert_eq!(
        pending
            .iter()
            .filter(|line| line.contains("Final response (corrected)"))
            .count(),
        1
    );
    assert!(
        pending
            .iter()
            .any(|line| line.contains("pre-codex-shell-output"))
    );
    insta::assert_snapshot!(
        "inline_mismatch_preserves_shell_history_and_appends_correction",
        pending.join("\n")
    );
    Ok(())
}
