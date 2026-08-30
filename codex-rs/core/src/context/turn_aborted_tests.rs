use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use super::ContextualUserFragment;
use super::TurnAborted;

#[test]
fn resumed_interruption_boundary_marks_old_process_handles_terminated() {
    let mut item: ResponseItem =
        ContextualUserFragment::into(TurnAborted::new(TurnAborted::INTERRUPTED_GUIDANCE));

    TurnAborted::rewrite_response_item_for_resume(&mut item);

    let ResponseItem::Message { content, .. } = item else {
        panic!("turn-aborted fragment should render as a message");
    };
    assert_eq!(
        content,
        vec![ContentItem::InputText {
            text: TurnAborted::new(TurnAborted::RESUMED_GUIDANCE).render(),
        }]
    );
}
