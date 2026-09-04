use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;

use crate::custom_terminal::InlineViewportState;
use crate::tui::InlineViewportPlacement;
use crate::tui::InlineViewportRole;
use crate::tui::scrollback::ScrollbackStrategy;

#[tokio::test]
async fn queued_history_keeps_composer_bottom_docked_after_transient_popup() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let screen_size = Size::new(/*width*/ 32, /*height*/ 12);
    tui.scrollback = ScrollbackStrategy::FullScreen;
    tui.terminal.last_known_screen_size = screen_size;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 8,
        screen_size.width,
        /*height*/ 4,
    ));
    tui.terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    tui.draw_with_resize_reflow(
        /*height*/ 8,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        InlineViewportRole::Transient,
        |_| {},
    )
    .expect("draw popup viewport");
    assert_eq!(
        tui.terminal.inline_viewport_state(),
        InlineViewportState {
            area: Rect::new(
                /*x*/ 0,
                /*y*/ 4,
                screen_size.width,
                /*height*/ 8,
            ),
            visible_history_rows: 2,
            docked_history_gap_rows: 0,
        }
    );

    tui.insert_history_lines(vec![Line::from("queued history")]);
    tui.draw_with_resize_reflow(
        /*height*/ 4,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        InlineViewportRole::Persistent,
        |_| {},
    )
    .expect("restore persistent viewport and flush history");

    assert_eq!(
        tui.terminal.inline_viewport_state(),
        InlineViewportState {
            area: Rect::new(
                /*x*/ 0,
                /*y*/ 8,
                screen_size.width,
                /*height*/ 4,
            ),
            visible_history_rows: 3,
            docked_history_gap_rows: 0,
        }
    );
}
