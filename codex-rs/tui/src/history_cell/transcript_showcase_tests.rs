//! A representative end-to-end transcript covering the full history-cell vocabulary in one
//! place: user input, de-emphasized reasoning, grouped exploration, an active command with live
//! output, a completed command, a diff summary, a plan update, final assistant prose, and the
//! turn-completion separator.
//!
//! This exists specifically to make transcript-wide regressions (spacing, indentation drift,
//! inconsistent state markers) visible in one review instead of scattered across many
//! single-cell snapshots. Prefer adding a new single-cell test for anything narrower than this.

use super::*;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::ExecCall;
use crate::exec_cell::ExecCell;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use std::collections::HashMap;
use std::path::PathBuf;

fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

fn showcase_cwd() -> PathBuf {
    std::env::temp_dir()
}

/// Builds one cell per transcript concern in narrative order: a user ask, the assistant's
/// de-emphasized reasoning, a grouped read/search exploration, a finished command, a diff, a plan,
/// the assistant's final answer, a still-running follow-up command, and the turn separator.
fn build_showcase_transcript() -> Vec<Box<dyn HistoryCell>> {
    let cwd = showcase_cwd();

    let user_message: Box<dyn HistoryCell> = Box::new(new_user_prompt(
        "Can you check why the Grafana client silently drops retry errors, then send me a patch \
         and a plan for finishing the cleanup?"
            .to_string(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ));

    let reasoning: Box<dyn HistoryCell> = Box::new(ReasoningSummaryCell::new(
        String::new(),
        "Looked at the retry helper and its tests; the timeout branch returns early without \
         logging, so failures never surface. I'll patch that path and outline the rest as a plan."
            .to_string(),
        &cwd,
        /*transcript_only*/ false,
    ));

    let mut exploring = ExecCell::new(
        ExecCall {
            call_id: "explore-1".to_string(),
            command: vec!["bash".into(), "-lc".into(), "rg grafana".into()],
            parsed: vec![
                ParsedCommand::Search {
                    query: Some("grafana".into()),
                    path: None,
                    cmd: "rg grafana".into(),
                },
                ParsedCommand::Read {
                    name: "grafana_client.rs".into(),
                    cmd: "cat grafana_client.rs".into(),
                    path: "src/net/grafana_client.rs".into(),
                },
                ParsedCommand::Read {
                    name: "grafana_client_tests.rs".into(),
                    cmd: "cat grafana_client_tests.rs".into(),
                    path: "src/net/grafana_client_tests.rs".into(),
                },
            ],
            output: None,
            source: ExecCommandSource::Agent,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    exploring.complete_call(
        "explore-1",
        CommandOutput::default(),
        Duration::from_millis(20),
    );
    let exploring: Box<dyn HistoryCell> = Box::new(exploring);

    let mut completed_command = ExecCell::new(
        ExecCall {
            call_id: "test-1".to_string(),
            command: vec![
                "bash".into(),
                "-lc".into(),
                "cargo test -p grafana_client --quiet".into(),
            ],
            parsed: Vec::new(),
            output: None,
            source: ExecCommandSource::Agent,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    completed_command.complete_call(
        "test-1",
        CommandOutput::new(
            /*exit_code*/ 0,
            "running 4 tests\n\
             test retry::backs_off_on_timeout ... ok\n\
             test retry::logs_timeout_reason ... ok\n\
             \n\
             test result: ok. 4 passed; 0 failed\n"
                .to_string(),
        ),
        Duration::from_secs(3),
    );
    let completed_command: Box<dyn HistoryCell> = Box::new(completed_command);

    let old_client = "pub fn send(&self, req: Request) -> Result<Response> {\n\
         \x20   match self.transport.call(req) {\n\
         \x20       Ok(res) => Ok(res),\n\
         \x20       Err(_) => Ok(self.cache.last_known_good()),\n\
         \x20   }\n\
         }\n";
    let new_client = "pub fn send(&self, req: Request) -> Result<Response> {\n\
         \x20   match self.transport.call(req) {\n\
         \x20       Ok(res) => Ok(res),\n\
         \x20       Err(err) => {\n\
         \x20           warn!(\"grafana request timed out: {err}\");\n\
         \x20           Ok(self.cache.last_known_good())\n\
         \x20       }\n\
         \x20   }\n\
         }\n";
    let patch = diffy::create_patch(old_client, new_client).to_string();
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("src/net/grafana_client.rs"),
        FileChange::Update {
            unified_diff: patch,
            move_path: None,
        },
    );
    let diff: Box<dyn HistoryCell> = Box::new(new_patch_event(changes, &cwd));

    let plan: Box<dyn HistoryCell> = Box::new(new_plan_update(UpdatePlanArgs {
        explanation: Some("Finish the Grafana client cleanup in three steps.".to_string()),
        plan: vec![
            PlanItemArg {
                step: "Log the timeout reason before returning the cached response".into(),
                status: StepStatus::Completed,
            },
            PlanItemArg {
                step: "Add a regression test for silent timeout drops".into(),
                status: StepStatus::InProgress,
            },
            PlanItemArg {
                step: "Document retry/backoff behavior in the client's module docs".into(),
                status: StepStatus::Pending,
            },
        ],
    }));

    let final_answer: Box<dyn HistoryCell> = Box::new(AgentMarkdownCell::new(
        "Fixed it — timeouts now log via `warn!` before falling back to the cached response, and \
         I added a regression test. Two things left:\n\
         \n\
         - Document the retry/backoff behavior in the module docs\n\
         - Consider surfacing a metric for timeout frequency\n\
         \n\
         Let me know if you'd like the metric added now or later."
            .to_string(),
        &cwd,
    ));

    let mut running_command = ExecCell::new(
        ExecCall {
            call_id: "test-2".to_string(),
            command: vec![
                "bash".into(),
                "-lc".into(),
                "cargo test -p grafana_client --quiet".into(),
            ],
            parsed: Vec::new(),
            output: None,
            source: ExecCommandSource::Agent,
            start_time: Some(Instant::now()),
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    running_command.append_output(
        "test-2",
        "running 5 tests\ntest retry::backs_off_on_timeout ... ok\n",
    );
    let running_command: Box<dyn HistoryCell> = Box::new(running_command);

    let final_status: Box<dyn HistoryCell> = Box::new(FinalMessageSeparator::new(
        Some(47),
        /*runtime_metrics*/ None,
    ));

    vec![
        user_message,
        reasoning,
        exploring,
        completed_command,
        diff,
        plan,
        final_answer,
        running_command,
        final_status,
    ]
}

/// Renders the showcase transcript the way the main viewport stacks committed history cells: each
/// cell's own lines, separated by a single blank line between non-empty cells.
fn render_showcase(width: u16) -> String {
    let cells = build_showcase_transcript();
    let mut out: Vec<String> = Vec::new();
    for cell in &cells {
        let lines = render_lines(&cell.display_lines(width));
        if lines.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(String::new());
        }
        out.extend(lines);
    }
    out.join("\n")
}

/// Representative 120-column transcript: user message, de-emphasized reasoning, grouped
/// read/search exploration, a completed command, a diff, a plan, final assistant prose, a
/// still-running command with live output, and the turn separator.
#[test]
fn showcase_transcript_120_wide() {
    insta::assert_snapshot!(
        "showcase_transcript_120_wide",
        render_showcase(/*width*/ 120)
    );
}

/// Same transcript at a narrow width to verify every cell degrades gracefully instead of
/// clipping content or losing its state markers when wrapped.
#[test]
fn showcase_transcript_narrow_50_wide() {
    insta::assert_snapshot!(
        "showcase_transcript_narrow_50_wide",
        render_showcase(/*width*/ 50)
    );
}
