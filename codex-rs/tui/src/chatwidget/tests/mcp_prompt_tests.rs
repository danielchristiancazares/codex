use super::*;
use codex_app_server_protocol::McpToolCallStatus;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy, Debug)]
enum PromptKind {
    Exec,
    Patch,
    Elicitation,
    Permissions,
    Question,
}

fn mcp_item(status: McpToolCallStatus) -> AppServerThreadItem {
    AppServerThreadItem::McpToolCall {
        id: "mcp-call".to_string(),
        server: "test_server".to_string(),
        tool: "query".to_string(),
        status,
        arguments: json!({}),
        app_context: None,
        mcp_app_resource_uri: None,
        plugin_id: None,
        read_only_hint: None,
        result: Some(Box::new(codex_app_server_protocol::McpToolCallResult {
            content: vec![json!({"type": "text", "text": "Finished"})],
            structured_content: None,
            meta: None,
        })),
        error: None,
        duration_ms: Some(5),
    }
}

fn question_params(
    thread_id: ThreadId,
    item_id: &str,
    question: &str,
) -> ToolRequestUserInputParams {
    serde_json::from_value(json!({
        "threadId": thread_id.to_string(),
        "turnId": "turn", "itemId": item_id, "isBlocking": true,
        "questions": [{
            "id": "choice", "header": "Result", "question": question,
            "options": [
                {"label": "Summary", "description": "Return a short summary."},
                {"label": "Details", "description": "Return the full result."},
            ],
        }],
    }))
    .unwrap()
}

fn request_prompt(chat: &mut ChatWidget, kind: PromptKind) {
    match kind {
        PromptKind::Exec => chat.on_exec_approval_request(
            "approval".to_string(),
            serde_json::from_value(json!({
                "call_id": "approval", "turn_id": "turn",
                "command": ["echo", "approved"], "cwd": chat.config.cwd,
                "reason": "Allow the pending call to continue.",
            }))
            .unwrap(),
        ),
        PromptKind::Patch => chat.on_apply_patch_approval_request(
            "approval".to_string(),
            serde_json::from_value(json!({
                "call_id": "approval", "turn_id": "turn", "changes": {},
                "reason": "Allow the pending call to continue.",
            }))
            .unwrap(),
        ),
        PromptKind::Elicitation => chat.on_elicitation_request(
            codex_app_server_protocol::RequestId::Integer(1),
            serde_json::from_value(json!({
                "threadId": chat.thread_id.unwrap().to_string(), "turnId": "turn",
                "serverName": "test_server", "mode": "form",
                "message": "Allow the pending call to continue?",
                "requestedSchema": {"type": "object", "properties": {}},
            }))
            .unwrap(),
        ),
        PromptKind::Permissions => chat.on_request_permissions(
            serde_json::from_value(json!({
                "call_id": "approval", "turn_id": "turn", "started_at_ms": 0,
                "reason": "Allow the pending call to continue.",
                "permissions": {"network": {"enabled": true}},
            }))
            .unwrap(),
        ),
        PromptKind::Question => chat.on_request_user_input(question_params(
            chat.thread_id.unwrap(),
            "question",
            "Which result should the call return?",
        )),
    }
}

#[tokio::test]
async fn interactive_prompts_unblock_active_mcp_calls_and_bypass_deferred_activity() {
    let mut snapshots = Vec::new();
    for kind in [
        PromptKind::Exec,
        PromptKind::Patch,
        PromptKind::Elicitation,
        PromptKind::Permissions,
        PromptKind::Question,
    ] {
        for queued_activity in [false, true] {
            let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
            chat.thread_id = Some(ThreadId::new());
            chat.show_welcome_banner = false;
            chat.local_settings.tui.animations = false;
            chat.on_mcp_tool_call_started(mcp_item(McpToolCallStatus::InProgress));
            if queued_activity {
                chat.on_web_search_begin("deferred-search".to_string());
            }

            request_prompt(&mut chat, kind);

            assert!(chat.bottom_pane.has_active_view(), "{kind:?}");
            assert!(chat.active_mcp_group_has_incomplete_members());
            assert!(!chat.interrupts.has_pending_prompt());
            if !queued_activity {
                snapshots.push(format!(
                    "{kind:?}\n{}",
                    render_bottom_popup(&chat, /*width*/ 80)
                ));
            }
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            let responded = std::iter::from_fn(|| rx.try_recv().ok()).any(|event| {
                matches!(
                    event,
                    AppEvent::CodexOp(
                        Op::ExecApproval { .. }
                            | Op::PatchApproval { .. }
                            | Op::ResolveElicitation { .. }
                            | Op::RequestPermissionsResponse { .. }
                            | Op::UserInputAnswer { .. }
                    ) | AppEvent::SubmitThreadOp {
                        op: Op::ExecApproval { .. }
                            | Op::PatchApproval { .. }
                            | Op::ResolveElicitation { .. }
                            | Op::RequestPermissionsResponse { .. }
                            | Op::UserInputAnswer { .. },
                        ..
                    }
                )
            });
            assert!(responded, "{kind:?} must answer before MCP completion");
            assert!(!chat.bottom_pane.has_active_view(), "{kind:?}");
            assert!(chat.active_mcp_group_has_incomplete_members());
            assert_eq!(chat.interrupts.is_empty(), !queued_activity);

            chat.on_mcp_tool_call_completed(mcp_item(McpToolCallStatus::Completed));
            assert!(!chat.active_mcp_group_has_incomplete_members());
            assert!(chat.interrupts.is_empty());
        }
    }
    insta::assert_snapshot!(
        "interactive_prompts_during_mcp_call",
        snapshots.join("\n\n")
    );
}

#[tokio::test]
async fn streamed_prompts_preserve_question_order_while_mcp_activity_waits() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_streaming_delta("Working on the request.\n".to_string());
    chat.on_mcp_tool_call_started(mcp_item(McpToolCallStatus::InProgress));
    chat.on_web_search_begin("deferred-search".to_string());
    request_prompt(&mut chat, PromptKind::Question);
    chat.on_request_user_input(question_params(
        chat.thread_id.unwrap(),
        "second-question",
        "What should happen next?",
    ));
    assert!(!chat.bottom_pane.has_active_view());
    assert!(chat.interrupts.has_pending_prompt());

    chat.flush_answer_stream_with_separator();
    chat.handle_stream_finished();

    assert!(chat.active_mcp_group_has_incomplete_members());
    assert!(!chat.interrupts.has_pending_prompt());
    assert!(
        render_bottom_popup(&chat, /*width*/ 80).contains("Which result should the call return?")
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("What should happen next?"));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(!chat.bottom_pane.has_active_view());
    assert!(chat.active_mcp_group_has_incomplete_members());
    assert!(!chat.interrupts.is_empty());

    chat.on_mcp_tool_call_completed(mcp_item(McpToolCallStatus::Completed));
    assert!(chat.interrupts.is_empty());
}
