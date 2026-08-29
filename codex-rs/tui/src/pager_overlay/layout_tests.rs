use super::*;
use pretty_assertions::assert_eq;

#[test]
fn frame_layout_prioritizes_content_for_heights_zero_through_six() {
    let expected = [
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 0),
            title_area: None,
            content_area: Rect::new(0, 0, 20, 0),
            separator_area: None,
            navigation_hint_area: None,
            close_hint_area: None,
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 1),
            title_area: None,
            content_area: Rect::new(0, 0, 20, 1),
            separator_area: None,
            navigation_hint_area: None,
            close_hint_area: None,
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 2),
            title_area: None,
            content_area: Rect::new(0, 0, 20, 1),
            separator_area: None,
            navigation_hint_area: None,
            close_hint_area: Some(Rect::new(0, 1, 20, 1)),
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 3),
            title_area: Some(Rect::new(0, 0, 20, 1)),
            content_area: Rect::new(0, 1, 20, 1),
            separator_area: None,
            navigation_hint_area: None,
            close_hint_area: Some(Rect::new(0, 2, 20, 1)),
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 4),
            title_area: Some(Rect::new(0, 0, 20, 1)),
            content_area: Rect::new(0, 1, 20, 1),
            separator_area: None,
            navigation_hint_area: Some(Rect::new(0, 2, 20, 1)),
            close_hint_area: Some(Rect::new(0, 3, 20, 1)),
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 5),
            title_area: Some(Rect::new(0, 0, 20, 1)),
            content_area: Rect::new(0, 1, 20, 1),
            separator_area: Some(Rect::new(0, 2, 20, 1)),
            navigation_hint_area: Some(Rect::new(0, 3, 20, 1)),
            close_hint_area: Some(Rect::new(0, 4, 20, 1)),
            trailing_spacer_area: None,
        },
        PagerFrameLayout {
            frame_area: Rect::new(0, 0, 20, 6),
            title_area: Some(Rect::new(0, 0, 20, 1)),
            content_area: Rect::new(0, 1, 20, 1),
            separator_area: Some(Rect::new(0, 2, 20, 1)),
            navigation_hint_area: Some(Rect::new(0, 3, 20, 1)),
            close_hint_area: Some(Rect::new(0, 4, 20, 1)),
            trailing_spacer_area: Some(Rect::new(0, 5, 20, 1)),
        },
    ];

    for (height, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            PagerFrameLayout::new(Rect::new(0, 0, 20, height as u16)),
            expected,
        );
    }
}
