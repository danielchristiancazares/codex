use std::path::PathBuf;

use codex_protocol::ThreadId;

use super::ContextWindowHandoff;
use super::MAX_CONTEXT_WINDOW_HANDOFF_BYTES;
use super::MAX_PREVIOUS_ROLLOUT_PATH_BYTES;
use super::PreviousRollout;
use super::RolloverHandoff;
use crate::context::ContextualUserFragment;

#[test]
fn rendered_handoff_has_a_hard_size_cap() {
    let handoff = "x".repeat(MAX_CONTEXT_WINDOW_HANDOFF_BYTES);
    let fragment = ContextWindowHandoff::new(
        RolloverHandoff::AgentGenerated(handoff),
        ThreadId::from_u128(/*value*/ 1),
        PreviousRollout::At(PathBuf::from("r".repeat(MAX_PREVIOUS_ROLLOUT_PATH_BYTES))),
    );

    let rendered = fragment.body();

    assert!(rendered.len() < 10_000);
}

#[test]
fn oversized_internal_handoff_falls_back_to_the_rollout_reference() {
    let oversized_handoff = "x".repeat(MAX_CONTEXT_WINDOW_HANDOFF_BYTES + 1);
    let fragment = ContextWindowHandoff::new(
        RolloverHandoff::AgentGenerated(oversized_handoff.clone()),
        ThreadId::from_u128(/*value*/ 1),
        PreviousRollout::Unavailable,
    );

    let rendered = fragment.body();

    assert!(rendered.contains("A complete agent-authored handoff was unavailable."));
    assert!(!rendered.contains(oversized_handoff.as_str()));
}
