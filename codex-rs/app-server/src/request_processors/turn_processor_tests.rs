use std::collections::HashMap;

use codex_protocol::turn_input::AdditionalContextAction;
use pretty_assertions::assert_eq;

use super::map_additional_context;

#[test]
fn omitted_additional_context_preserves_source_state_while_an_empty_map_clears_it() {
    assert_eq!(
        (
            map_additional_context(None),
            map_additional_context(Some(HashMap::new())),
        ),
        (
            AdditionalContextAction::KeepSourceState,
            AdditionalContextAction::ClearSourceState,
        )
    );
}
