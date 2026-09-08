use super::*;
use pretty_assertions::assert_eq;

#[test]
fn compact_layout_pins_provider_controls_by_height() {
    let area = |height| Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 40, height);

    assert_eq!(
        OssSelectionLayout::new(area(0), /*prompt_preferred_height*/ 10),
        OssSelectionLayout {
            prompt_area: area(0),
            title_area: None,
            button_area: None,
            description_area: None,
        },
    );
    assert_eq!(
        OssSelectionLayout::new(area(1), /*prompt_preferred_height*/ 10),
        OssSelectionLayout {
            prompt_area: Rect::new(0, 0, 40, 0),
            title_area: None,
            button_area: Some(Rect::new(0, 0, 40, 1)),
            description_area: None,
        },
    );
    assert_eq!(
        OssSelectionLayout::new(area(2), /*prompt_preferred_height*/ 10),
        OssSelectionLayout {
            prompt_area: Rect::new(0, 0, 40, 0),
            title_area: None,
            button_area: Some(Rect::new(0, 0, 40, 1)),
            description_area: Some(Rect::new(0, 1, 40, 1)),
        },
    );
    assert_eq!(
        OssSelectionLayout::new(area(3), /*prompt_preferred_height*/ 10),
        OssSelectionLayout {
            prompt_area: Rect::new(0, 0, 40, 0),
            title_area: Some(Rect::new(0, 0, 40, 1)),
            button_area: Some(Rect::new(0, 1, 40, 1)),
            description_area: Some(Rect::new(0, 2, 40, 1)),
        },
    );
}
