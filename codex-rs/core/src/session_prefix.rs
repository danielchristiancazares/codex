use codex_protocol::AgentPath;
use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;

use crate::context::ContextualUserFragment;
use crate::context::InterAgentCompletionMessage;

const COMPLETION_MESSAGE_MAX_TOKENS: usize = 1_000;
const COMPLETION_MESSAGE_ENVELOPE_TOKEN_RESERVE: usize = 100;
const COMPLETION_PAYLOAD_MAX_TOKENS: usize =
    COMPLETION_MESSAGE_MAX_TOKENS - COMPLETION_MESSAGE_ENVELOPE_TOKEN_RESERVE;
const ERROR_NEXT_ACTION: &str = "This agent's turn failed. If you still need this agent, use the available collaboration tools to give it another task.";

pub(crate) fn bounded_completion_status(status: &AgentStatus) -> AgentStatus {
    match status {
        AgentStatus::Completed(Some(message)) => AgentStatus::Completed(Some(truncate_text(
            message,
            TruncationPolicy::Tokens(COMPLETION_PAYLOAD_MAX_TOKENS),
        ))),
        AgentStatus::Errored(error) => AgentStatus::Errored(truncate_text(
            error,
            TruncationPolicy::Tokens(COMPLETION_PAYLOAD_MAX_TOKENS),
        )),
        AgentStatus::Completed(None)
        | AgentStatus::PendingInit
        | AgentStatus::Running
        | AgentStatus::Interrupted
        | AgentStatus::Shutdown
        | AgentStatus::NotFound => status.clone(),
    }
}

pub(crate) fn bounded_completion_fragment(
    body: String,
    start_marker: &str,
    end_marker: &str,
) -> String {
    let rendered = format!("{start_marker}{body}{end_marker}");
    if approx_token_count(&rendered) <= COMPLETION_MESSAGE_MAX_TOKENS {
        return rendered;
    }

    let marker_tokens = approx_token_count(&format!("{start_marker}{end_marker}"));
    let mut body_budget = COMPLETION_MESSAGE_MAX_TOKENS.saturating_sub(marker_tokens);
    loop {
        let body = truncate_text(&body, TruncationPolicy::Tokens(body_budget));
        let rendered = format!("{start_marker}{body}{end_marker}");
        let rendered_tokens = approx_token_count(&rendered);
        if rendered_tokens <= COMPLETION_MESSAGE_MAX_TOKENS || body_budget == 0 {
            return rendered;
        }
        body_budget = body_budget.saturating_sub(
            rendered_tokens
                .saturating_sub(COMPLETION_MESSAGE_MAX_TOKENS)
                .max(1),
        );
    }
}

// Helpers for model-visible session state markers that are stored in user-role
// messages but are not user intent.

// TODO(jif) unify with structured schema
pub(crate) fn format_inter_agent_completion_message(
    task_name: AgentPath,
    sender: AgentPath,
    status: &AgentStatus,
) -> Option<String> {
    let status = bounded_completion_status(status);
    let payload = match &status {
        AgentStatus::Completed(Some(message)) => message.clone(),
        AgentStatus::Completed(None) => String::new(),
        AgentStatus::Errored(error) => {
            format!("Agent errored: {error}\n\n{ERROR_NEXT_ACTION}")
        }
        AgentStatus::Shutdown => "Agent shut down.".to_string(),
        AgentStatus::NotFound => "Agent was not found.".to_string(),
        AgentStatus::PendingInit | AgentStatus::Running | AgentStatus::Interrupted => return None,
    };
    let message = InterAgentCompletionMessage::new(task_name, sender, payload);
    Some(bounded_completion_fragment(message.body(), "", ""))
}

#[cfg(test)]
#[path = "session_prefix_tests.rs"]
mod tests;

pub(crate) fn format_subagent_context_line(
    agent_reference: &str,
    agent_nickname: Option<&str>,
) -> String {
    match agent_nickname.filter(|nickname| !nickname.is_empty()) {
        Some(agent_nickname) => format!("- {agent_reference}: {agent_nickname}"),
        None => format!("- {agent_reference}"),
    }
}
