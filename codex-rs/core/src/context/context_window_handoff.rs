use std::path::PathBuf;

use codex_protocol::ThreadId;

use super::ContextualUserFragment;

pub(crate) const MAX_CONTEXT_WINDOW_HANDOFF_BYTES: usize = 8_000;
const MAX_PREVIOUS_ROLLOUT_PATH_BYTES: usize = 768;

pub(crate) const CONTEXT_WINDOW_HANDOFF_INSTRUCTIONS: &str = r#"Before starting a new context window, prepare a self-contained handoff and pass it to `new_context` in the `handoff` field. The handoff becomes a normal user message in the fresh context window. Core adds the continuation preamble, thread ID, and rollout path.

Write the handoff as plain Markdown beginning with `# Active task`. Include:

- `## Goal` with the active user goal quoted verbatim where practical.
- `## What completion means` with observable completion criteria.
- `# User messages that govern the work` with instruction-bearing user messages quoted exactly and in chronological order.
- `# Preserved inputs and large pasted content` with durable paths, original temporary paths, hashes or sizes when known, and the sections already examined.
- `# Work completed` and `# Current workspace state`, separating task-owned changes from pre-existing or unrelated changes.
- `# Important decisions already made`, `# Commands and validation`, and `# Approaches already attempted` with exact commands, results, and errors that matter.
- `# Remaining work`, `# Next action`, `# Open questions and uncertainties`, `# Sensitive artifacts and cleanup`, and `# Completion checkpoint`.

Preserve exact paths, identifiers, commands, validation status, and unresolved requirements. Materialize temporary pasted-content references in durable storage before rollover when their contents are needed later. Keep the handoff focused enough to fit the tool limit, and use the previous rollout for details that do not need to stay active."#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RolloverHandoff {
    AgentGenerated(String),
    RolloutReferenceOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreviousRollout {
    At(PathBuf),
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextWindowHandoff {
    handoff: RolloverHandoff,
    thread_id: ThreadId,
    rollout: PreviousRollout,
}

impl ContextWindowHandoff {
    pub(crate) fn new(
        handoff: RolloverHandoff,
        thread_id: ThreadId,
        rollout: PreviousRollout,
    ) -> Self {
        Self {
            handoff,
            thread_id,
            rollout,
        }
    }
}

impl ContextualUserFragment for ContextWindowHandoff {
    fn role(&self) -> &'static str {
        "user"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let rollout_reference = match &self.rollout {
            PreviousRollout::At(path) => {
                let path = path
                    .to_string_lossy()
                    .replace('`', "\\`")
                    .replace('\r', "\\r")
                    .replace('\n', "\\n");
                if path.len() <= MAX_PREVIOUS_ROLLOUT_PATH_BYTES {
                    format!(
                        "The complete previous conversation is available in the previous rollout at `{path}`."
                    )
                } else {
                    "The previous rollout path is too long to include safely; use the thread ID below to locate the persisted conversation.".to_string()
                }
            }
            PreviousRollout::Unavailable => "The previous rollout path is unavailable; use the thread ID below to locate the persisted conversation.".to_string(),
        };
        let handoff = match &self.handoff {
            RolloverHandoff::AgentGenerated(handoff)
                if handoff.trim().len() <= MAX_CONTEXT_WINDOW_HANDOFF_BYTES =>
            {
                handoff.trim()
            }
            RolloverHandoff::AgentGenerated(_) | RolloverHandoff::RolloutReferenceOnly => {
                "# Active task\n\nA complete agent-authored handoff was unavailable. Recover the active goal, governing user messages, current state, and next action from the previous rollout before continuing."
            }
        };

        format!(
            "The following is a message generated after compaction from a previous session. Continue naturally on the active task.\n\n{rollout_reference} The prior context window belonged to thread `{}`. Read the rollout whenever exact wording, chronology, or omitted context could affect the work.\n\nCurrent system, developer, and repository instructions still apply. Read the relevant `AGENTS.md` files before editing. Verify workspace state before relying on these notes.\n\n{handoff}",
            self.thread_id
        )
    }
}

#[cfg(test)]
#[path = "context_window_handoff_tests.rs"]
mod tests;
