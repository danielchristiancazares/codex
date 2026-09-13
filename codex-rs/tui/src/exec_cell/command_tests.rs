use super::*;
use pretty_assertions::assert_eq;

fn command_cell(state: CallState) -> ExecCell {
    ExecCell::new(
        ExecCall {
            call_id: "verification".to_string(),
            command: vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "just test -p atlas-client".to_string(),
            ],
            parsed: Vec::new(),
            output: Some(CommandOutput::new(
                if state == CallState::Failed { 7 } else { 0 },
                "stdout: verification\n\x1b[31mstderr: verification\x1b[0m".to_string(),
            )),
            source: ExecCommandSource::Agent,
            start_time: (state == CallState::Active).then(Instant::now),
            duration: (state != CallState::Active)
                .then_some(std::time::Duration::from_millis(5)),
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    )
}

#[test]
fn terminal_failure_output_replaces_running_state_before_duration_arrives() {
    let mut cell = command_cell(CallState::Active);
    let before = cell.display_lines(/*width*/ 80);
    cell.calls[0]
        .output
        .as_mut()
        .expect("live output")
        .exit_code = 1;
    let after = cell.display_lines(/*width*/ 80);
    let transcript = cell
        .transcript_lines(/*width*/ 80)
        .iter()
        .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect::<String>())
        .join("\n");

    assert_eq!(
        (
            before[0].spans.iter().map(|span| span.content.as_ref()).collect::<String>(),
            after[0].spans.iter().map(|span| span.content.as_ref()).collect::<String>(),
            transcript.contains("✗ (1)"),
        ),
        (
            "• Running just test -p atlas-client".to_string(),
            "• Ran just test -p atlas-client".to_string(),
            true,
        )
    );
}

#[test]
fn command_outcomes_keep_ansi_color_and_completion_state() {
    let presentations = [
        CallState::Active,
        CallState::Succeeded,
        CallState::Failed,
    ]
    .map(|state| (state, command_cell(state).display_lines(/*width*/ 80)));
    insta::assert_debug_snapshot!("command_outcomes", presentations);
}
