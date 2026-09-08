//! Exercise real popup transitions at both compact and wide terminal sizes.

use super::*;
use crate::tui::test_support::TestScrollback;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

#[derive(Debug)]
enum Stage {
    Composer,
    Model,
    Reasoning,
    Back,
    Closed,
    History,
}

#[tokio::test]
async fn composer_and_popups_keep_the_bottom_anchor_through_history_and_resizes() -> Result<()> {
    let mut snapshots = Vec::new();
    for scrollback in [TestScrollback::Standard, TestScrollback::FullScreen] {
        let mut app = crate::app::test_support::make_test_app().await;
        app.transcript_cells = (1..=40)
            .map(|index| {
                Arc::new(history_cell::PlainHistoryCell::new(vec![Line::from(
                    format!("Retained conversation line {index}"),
                )])) as Arc<dyn HistoryCell>
            })
            .collect();
        let presets = app
            .model_catalog
            .try_list_models()
            .expect("test model catalog");
        let reasoning_model = presets
            .iter()
            .find(|preset| preset.model == "gpt-5.6-sol")
            .cloned()
            .expect("reasoning model");
        let mut tui = crate::tui::test_support::make_test_tui_with_scrollback(scrollback)?;
        app.chat_widget
            .handle_paste("Keep this draft intact".to_string());

        for size in [
            Size::new(/*width*/ 132, /*height*/ 32),
            Size::new(/*width*/ 80, /*height*/ 24),
            Size::new(/*width*/ 48, /*height*/ 20),
        ] {
            // A shorter incoming history batch can leave this former popup anchor above bottom.
            tui.terminal.last_known_screen_size = size;
            tui.terminal.set_viewport_area(Rect::new(
                /*x*/ 0, /*y*/ 2, size.width, /*height*/ 8,
            ));
            app.reflow_transcript_now(&mut tui, size.into())?;
            let mut draft_cursor = None;
            for stage in [
                Stage::Composer,
                Stage::Model,
                Stage::Reasoning,
                Stage::Back,
                Stage::Closed,
                Stage::History,
            ] {
                match stage {
                    Stage::Model => app
                        .chat_widget
                        .open_model_popup_with_presets(presets.clone()),
                    Stage::Reasoning => app
                        .chat_widget
                        .open_reasoning_popup(reasoning_model.clone()),
                    Stage::Back | Stage::Closed => app
                        .chat_widget
                        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                    Stage::History => {
                        tui.insert_history_lines(vec![Line::from("New output after the popup")])
                    }
                    Stage::Composer => {}
                }
                let area = app.render_chat_widget_frame(&mut tui, size)?;
                assert_eq!(area.bottom(), size.height, "{stage:?} at {size:?}");
                assert_eq!(
                    app.chat_widget.composer_text_with_pending(),
                    "Keep this draft intact"
                );
                if matches!(stage, Stage::Composer | Stage::Closed | Stage::History) {
                    assert_eq!(
                        tui.terminal.visible_history_rows(),
                        area.top(),
                        "conversation should refill the rows vacated by {stage:?}"
                    );
                    assert_eq!(tui.terminal.docked_history_gap_rows(), 0);
                }
                let mut buffer = Buffer::empty(area);
                let widget = app.chat_widget.as_renderable();
                widget.render(area, &mut buffer);
                match stage {
                    Stage::Composer => draft_cursor = widget.cursor_pos(area),
                    Stage::Closed | Stage::History => {
                        assert_eq!(widget.cursor_pos(area), draft_cursor)
                    }
                    Stage::Model | Stage::Reasoning | Stage::Back => {}
                }
                let last_content_row = (area.top()..area.bottom())
                    .rev()
                    .find(|y| {
                        (area.left()..area.right())
                            .any(|x| !buffer[(x, *y)].symbol().trim().is_empty())
                    })
                    .expect("visible popup or composer");
                assert!(
                    area.bottom() - last_content_row <= 2,
                    "unexpected trailing blank rows in {stage:?}: {buffer:?}"
                );
                if size.width == 132 {
                    let text = (area.top()..area.bottom())
                        .map(|y| {
                            (area.left()..area.right())
                                .map(|x| buffer[(x, y)].symbol())
                                .collect::<String>()
                                .trim_end()
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let text = text.replace(
                        app.config.cwd.to_path_buf().to_string_lossy().as_ref(),
                        "<WORKSPACE>",
                    );
                    snapshots.push(format!("{stage:?}: {area:?}\n{text}"));
                }
            }
        }
    }
    insta::assert_snapshot!("popup_bottom_anchor_transitions", snapshots.join("\n\n"));
    Ok(())
}

#[tokio::test]
async fn shrinking_owned_viewport_refills_the_visible_history_band() -> Result<()> {
    for scrollback in [TestScrollback::Standard, TestScrollback::FullScreen] {
        let mut app = crate::app::test_support::make_test_app().await;
        app.transcript_cells = (1..=40)
            .map(|index| {
                Arc::new(history_cell::PlainHistoryCell::new(vec![Line::from(
                    format!("Retained conversation line {index}"),
                )])) as Arc<dyn HistoryCell>
            })
            .collect();
        let mut tui = crate::tui::test_support::make_test_tui_with_scrollback(scrollback)?;
        let size = Size::new(/*width*/ 48, /*height*/ 20);
        tui.terminal.last_known_screen_size = size;
        tui.set_alt_screen_enabled(/*enabled*/ true);
        tui.enter_alt_screen()?;
        app.reflow_transcript_now(&mut tui, size.into())?;
        let composer_height = app.chat_widget.as_renderable().desired_height(size.width);
        let active_height = composer_height + 2;
        tui.draw_with_resize_reflow(
            active_height,
            size,
            tui::InlineViewportPlacement::BottomDocked,
            tui::InlineViewportRole::Persistent,
            |_| {},
        )?;
        assert_eq!(
            tui.terminal.visible_history_rows(),
            size.height - active_height
        );

        let area = app.render_chat_widget_frame(&mut tui, size)?;
        assert_eq!(area.height, composer_height);
        assert_eq!(area.bottom(), size.height);
        assert_eq!(tui.terminal.visible_history_rows(), area.top());
        assert_eq!(tui.terminal.docked_history_gap_rows(), 0);
        assert_eq!(app.transcript_cells.len(), 40);
        tui.leave_alt_screen()?;
    }
    Ok(())
}

#[tokio::test]
async fn leaving_full_height_landing_keeps_the_submitted_prompt_visible() -> Result<()> {
    for scrollback in [TestScrollback::Standard, TestScrollback::FullScreen] {
        let mut app = crate::app::test_support::make_test_app().await;
        let mut tui = crate::tui::test_support::make_test_tui_with_scrollback(scrollback)?;
        let size = Size::new(/*width*/ 120, /*height*/ 36);
        tui.terminal.last_known_screen_size = size;
        tui.terminal.set_viewport_area(Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            size.width,
            size.height,
        ));
        app.insert_history_cell(
            &mut tui,
            Box::new(history_cell::PlainHistoryCell::new(vec![Line::from(
                "Describe the interface in a short, well-structured response.",
            )])),
        );

        let area = app.render_chat_widget_frame(&mut tui, size)?;
        assert_eq!(
            tui.terminal.inline_viewport_state(),
            crate::custom_terminal::InlineViewportState {
                area,
                visible_history_rows: 1,
                docked_history_gap_rows: 0,
            }
        );
        assert_eq!(area.bottom(), size.height);
        assert_eq!(app.transcript_cells.len(), 1);
    }
    Ok(())
}
