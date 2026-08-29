use super::*;
use codex_protocol::mcp::CallToolResult;
use pretty_assertions::assert_eq;
use serde_json::json;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

fn invocation(server: &str, tool: &str, title: &str) -> McpInvocation {
    McpInvocation {
        server: server.to_string(),
        tool: tool.to_string(),
        arguments: Some(json!({"title": title})),
    }
}

fn result(text: &str) -> CallToolResult {
    CallToolResult {
        content: vec![json!({"type": "text", "text": text})],
        structured_content: None,
        is_error: None,
        meta: None,
    }
}

fn rendered(lines: Vec<Line<'static>>) -> String {
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn one_member_group_matches_standalone_surfaces() {
    let standalone = new_active_mcp_tool_call(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    let group = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );

    assert_eq!(
        group.display_lines(/*width*/ 80),
        standalone.display_lines(/*width*/ 80)
    );
    assert_eq!(
        group.transcript_lines(/*width*/ 80),
        standalone.transcript_lines(/*width*/ 80)
    );
    assert_eq!(group.raw_lines(), standalone.raw_lines());
    assert_eq!(
        group.transcript_animation_tick(),
        standalone.transcript_animation_tick()
    );

    let mut standalone = new_active_mcp_tool_call(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    standalone.complete(Duration::ZERO, Ok(result("result A")));
    let mut group = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    assert!(group.complete_call("call-a", Duration::ZERO, Ok(result("result A"))));
    assert_eq!(
        group.display_lines(/*width*/ 80),
        standalone.display_lines(/*width*/ 80)
    );
    assert_eq!(
        group.transcript_lines(/*width*/ 80),
        standalone.transcript_lines(/*width*/ 80)
    );
    assert_eq!(group.raw_lines(), standalone.raw_lines());
    assert_eq!(
        group.transcript_animation_tick(),
        standalone.transcript_animation_tick()
    );
}

#[test]
fn group_rendering_snapshots_cover_lifecycle_states() {
    let one = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    insta::assert_snapshot!("mcp_group_one_call", rendered(one.display_lines(80)), @r#"
    • Calling server.lookup({"title":"A"})
    "#);

    let mut group = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    assert!(group.add_started_call(
        "call-b".to_string(),
        invocation("server", "lookup", "B"),
        /*animations_enabled*/ false,
    ));
    insta::assert_snapshot!("mcp_group_two_active", rendered(group.display_lines(80)), @r#"
    • Calling server.lookup({"title":"A"})
    • Calling server.lookup({"title":"B"})
    "#);

    assert!(group.complete_call("call-a", Duration::ZERO, Ok(result("result A"))));
    insta::assert_snapshot!("mcp_group_one_complete", rendered(group.display_lines(80)), @r#"
    • Called server.lookup({"title":"A"})
      └ result A
    • Calling server.lookup({"title":"B"})
    "#);

    assert!(group.complete_call("call-b", Duration::ZERO, Ok(result("result B"))));
    insta::assert_snapshot!("mcp_group_all_complete", rendered(group.display_lines(80)), @r#"
    • Called server.lookup({"title":"A"})
      └ result A
    • Called server.lookup({"title":"B"})
      └ result B
    "#);

    let mut interrupted = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    interrupted.add_started_call(
        "call-b".to_string(),
        invocation("server", "lookup", "B"),
        /*animations_enabled*/ false,
    );
    interrupted.mark_all_incomplete_failed();
    insta::assert_snapshot!("mcp_group_interrupted", rendered(interrupted.display_lines(80)), @r#"
    • Called server.lookup({"title":"A"})
      └ Error: interrupted
    • Called server.lookup({"title":"B"})
      └ Error: interrupted
    "#);
}

#[test]
fn image_follow_up_stays_with_owning_member() {
    let mut group = McpToolCallGroupCell::new(
        "call-a".to_string(),
        invocation("server", "lookup", "A"),
        /*animations_enabled*/ false,
    );
    group.add_started_call(
        "call-b".to_string(),
        invocation("server", "lookup", "B"),
        /*animations_enabled*/ false,
    );
    let image = CallToolResult {
        content: vec![json!({"type": "image", "mimeType": "image/png", "data": PNG})],
        structured_content: None,
        is_error: None,
        meta: None,
    };
    assert!(group.complete_call("call-a", Duration::ZERO, Ok(image)));

    insta::assert_snapshot!(rendered(group.display_lines(80)), @r#"
    • Called server.lookup({"title":"A"})
      └ <image content>
    tool result (image output)
    • Calling server.lookup({"title":"B"})
    "#);
    assert_eq!(
        rendered(group.transcript_lines(/*width*/ 80)),
        "• Called server.lookup({\"title\":\"A\"})\n  └ <image content>\ntool result (image output)\n• Calling server.lookup({\"title\":\"B\"})"
    );
    assert_eq!(
        rendered(group.raw_lines()),
        "Called server.lookup({\"title\":\"A\"})\n<image content>\ntool result (image output)\nCalling server.lookup({\"title\":\"B\"})"
    );
}

#[test]
fn node_repl_group_transcript_preserves_expanded_detail() {
    let mut group = McpToolCallGroupCell::new(
        "call-node".to_string(),
        invocation("node_repl", "js", "Inspect workspace"),
        /*animations_enabled*/ false,
    );
    assert!(group.complete_call(
        "call-node",
        Duration::ZERO,
        Ok(result("Script completed\nOutput:\nexpanded result"))
    ));

    insta::assert_snapshot!(rendered(group.transcript_lines(80)), @r#"
    • Called node_repl.js({"title":"Inspect workspace"})
      └ Script completed
        Output:
        expanded result
    "#);
}
