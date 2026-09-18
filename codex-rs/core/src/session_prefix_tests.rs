use codex_protocol::AgentPath;
use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::approx_token_count;

use super::COMPLETION_MESSAGE_MAX_TOKENS;
use super::ERROR_NEXT_ACTION;
use super::format_inter_agent_completion_message;
use crate::context::ContextualUserFragment;
use crate::context::SubagentNotification;

#[test]
fn error_completion_message_stays_below_manual_review_threshold() {
    let message = format_inter_agent_completion_message(
        AgentPath::root(),
        AgentPath::try_from("/root/worker").expect("valid agent path"),
        &AgentStatus::Errored("stream disconnected ".repeat(1_000)),
    )
    .expect("error status should produce a completion message");

    assert!(approx_token_count(&message) < COMPLETION_MESSAGE_MAX_TOKENS);
    assert!(message.contains(ERROR_NEXT_ACTION));
}

#[test]
fn successful_completion_message_stays_below_manual_review_threshold() {
    let message = format_inter_agent_completion_message(
        AgentPath::root(),
        AgentPath::try_from("/root/worker").expect("valid agent path"),
        &AgentStatus::Completed(Some("large result ".repeat(100_000))),
    )
    .expect("completed status should produce a completion message");

    assert!(approx_token_count(&message) < COMPLETION_MESSAGE_MAX_TOKENS);
    assert!(message.contains("tokens truncated"));
}

#[test]
fn v1_subagent_notification_bounds_successful_completion() {
    let message = SubagentNotification::new(
        "worker",
        AgentStatus::Completed(Some("large result ".repeat(100_000))),
    )
    .render();

    assert!(approx_token_count(&message) < COMPLETION_MESSAGE_MAX_TOKENS);
    assert!(message.contains("tokens truncated"));
}

#[test]
fn escaped_completion_payloads_and_long_paths_stay_within_cap() {
    let payloads = ["\"".repeat(8_000), "\n".repeat(8_000), "\0".repeat(8_000)];
    let long_component = "worker".repeat(500);
    let sender =
        AgentPath::try_from(format!("/root/{long_component}")).expect("valid long agent path");

    for payload in payloads {
        let v2 = format_inter_agent_completion_message(
            AgentPath::root(),
            sender.clone(),
            &AgentStatus::Completed(Some(payload.clone())),
        )
        .expect("completed status should render");
        let v1 =
            SubagentNotification::new(sender.to_string(), AgentStatus::Completed(Some(payload)))
                .render();

        assert!(approx_token_count(&v2) <= COMPLETION_MESSAGE_MAX_TOKENS);
        assert!(approx_token_count(&v1) <= COMPLETION_MESSAGE_MAX_TOKENS);
        assert!(v1.starts_with("<subagent_notification>"));
        assert!(v1.ends_with("</subagent_notification>"));
    }
}
