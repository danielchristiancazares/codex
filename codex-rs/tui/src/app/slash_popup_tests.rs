//! Slash-command completion must leave older terminal scrollback untouched.

use super::*;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::set_fast_mode_test_catalog;
use crate::tui::test_support::TestScrollback;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct ScrollbackSentinel {
    renders: Arc<AtomicUsize>,
}

impl HistoryCell for ScrollbackSentinel {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.renders.fetch_add(/*val*/ 1, Ordering::Relaxed);
        self.raw_lines()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(
            "Older thread content outside the visible screen",
        )]
    }
}

#[tokio::test]
async fn slash_command_completion_preserves_older_scrollback() -> Result<()> {
    for scrollback in [TestScrollback::Standard, TestScrollback::FullScreen] {
        for size in [
            Size::new(/*width*/ 120, /*height*/ 36),
            Size::new(/*width*/ 48, /*height*/ 20),
        ] {
            for command in ["/fast", "/status"] {
                let mut app = crate::app::test_support::make_test_app().await;
                let (mut chat, _tx, mut events, _ops) = make_chatwidget_manual_with_sender().await;
                set_fast_mode_test_catalog(&mut chat);
                chat.set_model("gpt-5.4");
                chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);
                app.chat_widget = chat;
                let renders = Arc::new(AtomicUsize::new(/*v*/ 0));
                app.transcript_cells.push(Arc::new(ScrollbackSentinel {
                    renders: Arc::clone(&renders),
                }));
                app.transcript_cells.extend((1..=80).map(|index| {
                    Arc::new(history_cell::PlainHistoryCell::new(vec![Line::from(
                        format!("Retained thread line {index}"),
                    )])) as Arc<dyn HistoryCell>
                }));
                let mut tui = crate::tui::test_support::make_test_tui_with_scrollback(scrollback)?;
                tui.terminal.last_known_screen_size = size;
                app.reflow_transcript_now(&mut tui, size.into())?;
                app.render_chat_widget_frame(&mut tui, size)?;
                assert!(renders.load(Ordering::Relaxed) > 0);
                renders.store(/*val*/ 0, Ordering::Relaxed);

                for fragment in ["/", &command[1..]] {
                    app.chat_widget.handle_paste(fragment.to_string());
                    app.render_chat_widget_frame(&mut tui, size)?;
                }
                assert_eq!(
                    tui.last_resize_reflow_role,
                    tui::InlineViewportRole::Transient
                );
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                let mut command_effect = false;
                while let Ok(event) = events.try_recv() {
                    match event {
                        AppEvent::InsertHistoryCell(cell) => {
                            command_effect = true;
                            app.insert_history_cell(&mut tui, cell);
                        }
                        AppEvent::PersistServiceTierSelection { .. } => command_effect = true,
                        _ => {}
                    }
                }
                assert!(command_effect, "{command} must execute its normal action");
                let area = app.render_chat_widget_frame(&mut tui, size)?;
                assert_eq!(
                    renders.load(Ordering::Relaxed),
                    0,
                    "{command} at {size:?} replayed older history"
                );
                assert_eq!(area.bottom(), size.height);
                assert_eq!(tui.terminal.visible_history_rows(), area.top());
                assert!(!app.transcript_reflow.has_pending_reflow());
            }
        }
    }
    Ok(())
}
