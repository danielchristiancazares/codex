//! Root-turn lineage fences across mailbox injection and reserved task start.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn active_turn_mailbox_injection_preserves_only_selected_root_lineage() {
    for (trigger_turn, incoming_root_turn_id, expected_root_turn_id) in [
        (false, Some("conflicting-root"), Some("active-root")),
        (true, Some("active-root"), Some("active-root")),
        (true, Some("conflicting-root"), None),
        (true, None, None),
    ] {
        let (sess, tc, _rx) = make_session_and_context_with_rx().await;
        tc.turn_metadata_state
            .set_root_turn_id("active-root".to_string());
        sess.spawn_task(
            Arc::clone(&tc),
            Vec::new(),
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: true,
            },
        )
        .await;
        let communication = InterAgentCommunication::new(
            AgentPath::try_from("/root/worker").expect("worker path should parse"),
            AgentPath::root(),
            Vec::new(),
            "mailbox update".to_string(),
            trigger_turn,
        );
        sess.input_queue
            .enqueue_mailbox_communication(
                communication.clone(),
                codex_protocol::turn_input::TurnStartOptions {
                    parent_turn_id: Some("incoming-parent".to_string()),
                    root_turn_id: incoming_root_turn_id.map(str::to_string),
                    ..Default::default()
                },
            )
            .await;

        assert_eq!(
            (sess.input_queue.get_pending_input(&sess.active_turn).await).0,
            vec![TurnInput::InterAgentCommunication(communication)]
        );
        assert_eq!(
            tc.turn_metadata_state.root_turn_id().as_deref(),
            expected_root_turn_id
        );

        sess.abort_all_tasks(TurnAbortReason::Replaced).await;
    }
}

#[tokio::test]
async fn reserved_turn_start_preserves_only_matching_new_mail_lineage() {
    for (trigger_turn, incoming_root_turn_id, expected_root_turn_id) in [
        (false, Some("conflicting-root"), Some("reserved-root")),
        (true, Some("reserved-root"), Some("reserved-root")),
        (true, Some("conflicting-root"), None),
        (true, None, None),
    ] {
        let (sess, tc, _rx) = make_session_and_context_with_rx().await;
        tc.turn_metadata_state
            .set_root_turn_id("reserved-root".to_string());
        let communication = InterAgentCommunication::new(
            AgentPath::try_from("/root/worker").expect("worker path should parse"),
            AgentPath::root(),
            Vec::new(),
            "mail after reservation".to_string(),
            trigger_turn,
        );
        sess.input_queue
            .enqueue_mailbox_communication(
                communication.clone(),
                codex_protocol::turn_input::TurnStartOptions {
                    parent_turn_id: Some("incoming-parent".to_string()),
                    root_turn_id: incoming_root_turn_id.map(str::to_string),
                    ..Default::default()
                },
            )
            .await;

        sess.start_task(
            Arc::clone(&tc),
            Vec::new(),
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: true,
            },
        )
        .await;

        assert_eq!(
            (sess.input_queue.get_pending_input(&sess.active_turn).await).0,
            vec![TurnInput::InterAgentCommunication(communication)]
        );
        assert_eq!(
            tc.turn_metadata_state.root_turn_id().as_deref(),
            expected_root_turn_id
        );

        sess.abort_all_tasks(TurnAbortReason::Replaced).await;
    }
}
