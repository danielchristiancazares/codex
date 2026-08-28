use super::ElicitationRequestManager;
use super::ElicitationRequestRouter;
use crate::mcp::tests::test_elicitation_config;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_KEY;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::ElicitationResponse;
use pretty_assertions::assert_eq;
use rmcp::model::ElicitationAction;
use rmcp::model::RequestId;
use serde_json::json;

#[derive(Clone, Copy)]
enum InputCapability {
    Enabled,
    Disabled,
}

#[tokio::test]
async fn openai_elicitation_full_access_requires_enabled_nonapproval_user_input() {
    let fields = json!({"properties": {"name": {"type": "string"}}});
    for (capability, schema, meta) in [
        (InputCapability::Disabled, fields.clone(), None),
        (InputCapability::Enabled, json!({"properties": {}}), None),
        (InputCapability::Enabled, json!({"properties": []}), None),
        (InputCapability::Enabled, json!("opaque schema"), None),
        (
            InputCapability::Enabled,
            fields,
            Some(json!({(APPROVAL_KIND_KEY): "mcp_tool_call"})),
        ),
    ] {
        let router = ElicitationRequestRouter::default();
        if matches!(capability, InputCapability::Enabled) {
            router.enable_full_access_form_input();
        }
        let manager = ElicitationRequestManager::new(
            test_elicitation_config("server", AskForApproval::Never, PermissionProfile::Disabled),
            /*reviewer*/ None,
            /*lifecycle*/ None,
            router,
        );
        let (tx_event, rx_event) = async_channel::bounded(/*cap*/ 1);
        let sender = manager.make_sender("server".to_string(), Some(tx_event));
        let response = tokio::select! {
            biased;
            event = rx_event.recv() => panic!("unexpected user-input event: {event:?}"),
            response = sender(RequestId::Number(1), Elicitation::OpenAiElicitationForm {
                meta,
                message: "Choose a value".to_string(),
                requested_schema: schema,
            }) => response.expect("form must receive a response"),
        };
        assert_eq!(
            response,
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            }
        );
    }
}

#[tokio::test]
async fn openai_elicitation_full_access_preserves_opaque_input_and_requires_user_response() {
    let router = ElicitationRequestRouter::default();
    router.enable_full_access_form_input();
    let manager = ElicitationRequestManager::new(
        test_elicitation_config("server", AskForApproval::Never, PermissionProfile::Disabled),
        /*reviewer*/ None,
        /*lifecycle*/ None,
        router.clone(),
    );
    let (tx_event, rx_event) = async_channel::bounded(/*cap*/ 1);
    let sender = manager.make_sender("server".to_string(), Some(tx_event));
    let meta = Some(json!({"example/request": [1, "opaque"]}));
    let schema = json!({
        "type": "object",
        "properties": {"name": {"type": "string", "x-openai-preview": {"custom": true}}},
        "x-openai-layout": {"unknown": [1, 2]},
    });
    let mut pending = tokio::spawn(sender(
        RequestId::Number(1),
        Elicitation::OpenAiElicitationForm {
            meta: meta.clone(),
            message: "Choose a value".to_string(),
            requested_schema: schema.clone(),
        },
    ));
    let request = tokio::select! {
        event = rx_event.recv() => {
            let EventMsg::ElicitationRequest(request) = event.expect("input event").msg else {
                panic!("expected elicitation request");
            };
            request
        }
        response = &mut pending => panic!("form resolved without user input: {response:?}"),
    };
    assert_eq!(
        (request.turn_id, request.server_name, request.request),
        (
            None,
            "server".to_string(),
            ElicitationRequest::OpenAiElicitationForm {
                meta,
                message: "Choose a value".to_string(),
                requested_schema: schema,
            }
        ),
    );
    let codex_protocol::mcp::RequestId::String(request_id) = request.id else {
        panic!("expected a routed string request ID");
    };
    let expected = ElicitationResponse {
        action: ElicitationAction::Accept,
        content: Some(json!({"name": "Ada"})),
        meta: Some(json!({"example/response": "opaque"})),
    };
    router
        .resolve(
            "server".to_string(),
            RequestId::String(request_id.into()),
            expected.clone(),
        )
        .await
        .expect("resolve routed request");
    assert_eq!(
        pending
            .await
            .expect("response task")
            .expect("user response"),
        expected
    );
}
