use super::*;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::cell::RefCell;

struct HeightRenderable(u16);

impl HeightRenderable {
    fn with_height(height: u16) -> Self {
        Self(height)
    }
}

impl Renderable for HeightRenderable {
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}

    fn desired_height(&self, _width: u16) -> u16 {
        self.0
    }
}

#[test]
fn flex_redistributes_space_unused_by_short_children() {
    let mut flex = FlexRenderable::new();
    flex.push(
        /*flex*/ 1,
        RenderableItem::Owned(Box::new(HeightRenderable::with_height(/*height*/ 20))),
    );
    flex.push(
        /*flex*/ 1,
        RenderableItem::Owned(Box::new(HeightRenderable::with_height(/*height*/ 2))),
    );

    let allocated = flex.allocate(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
    ));

    assert_eq!(
        allocated
            .into_iter()
            .map(|area| area.height)
            .collect::<Vec<_>>(),
        vec![8, 2],
    );
}

#[test]
fn flex_reserves_non_flex_space_before_flexible_children() {
    let mut flex = FlexRenderable::new();
    flex.push(
        /*flex*/ 1,
        RenderableItem::Owned(Box::new(HeightRenderable::with_height(/*height*/ 20))),
    );
    flex.push(
        /*flex*/ 0,
        RenderableItem::Owned(Box::new(HeightRenderable::with_height(/*height*/ 2))),
    );
    flex.push(
        /*flex*/ 1,
        RenderableItem::Owned(Box::new(HeightRenderable::with_height(/*height*/ 20))),
    );

    let allocated = flex.allocate(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
    ));

    assert_eq!(
        allocated
            .into_iter()
            .map(|area| area.height)
            .collect::<Vec<_>>(),
        vec![4, 2, 4],
    );
}

#[test]
fn flex_caches_child_height_across_frame_passes() {
    struct CountingRenderable<'a>(&'a Cell<usize>);

    impl Renderable for CountingRenderable<'_> {
        fn render(&self, _area: Rect, _buf: &mut Buffer) {}

        fn desired_height(&self, _width: u16) -> u16 {
            self.0.set(self.0.get() + 1);
            1
        }

        fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
            Some((area.x, area.y))
        }
    }

    let calls = Cell::new(0);
    let renderable = CountingRenderable(&calls);
    let mut flex = FlexRenderable::new();
    flex.push(/*flex*/ 1, RenderableItem::Borrowed(&renderable));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
    );
    let mut buf = Buffer::empty(area);

    assert_eq!(flex.desired_height(area.width), 1);
    flex.render(area, &mut buf);
    assert_eq!(flex.cursor_pos(area), Some((0, 0)));
    assert!(matches!(
        flex.cursor_style(area),
        crossterm::cursor::SetCursorStyle::DefaultUserShape
    ));
    assert_eq!(calls.get(), 1);

    assert_eq!(flex.desired_height(/*width*/ 100), 1);
    assert_eq!(calls.get(), 2);
}

#[test]
fn inset_desired_height_saturates_at_tiny_widths() {
    struct WidthSensitiveRenderable<'a>(&'a RefCell<Vec<u16>>);

    impl Renderable for WidthSensitiveRenderable<'_> {
        fn render(&self, _area: Rect, _buf: &mut Buffer) {}

        fn desired_height(&self, width: u16) -> u16 {
            self.0.borrow_mut().push(width);
            if width == 0 { u16::MAX } else { width }
        }
    }

    let forwarded_widths = RefCell::new(Vec::new());
    let child = WidthSensitiveRenderable(&forwarded_widths);
    let inset = InsetRenderable::new(
        RenderableItem::Borrowed(&child),
        Insets::tlbr(
            /*top*/ 1, /*left*/ 1, /*bottom*/ 1, /*right*/ 1,
        ),
    );

    let heights = [0, 1, 2, 3]
        .into_iter()
        .map(|width| inset.desired_height(width))
        .collect::<Vec<_>>();

    assert_eq!(*forwarded_widths.borrow(), vec![0, 0, 0, 1]);
    assert_eq!(heights, vec![u16::MAX, u16::MAX, u16::MAX, 3]);
}
