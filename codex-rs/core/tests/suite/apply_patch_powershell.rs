use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::PatchApplyStatus;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_shell_command::shell_detect::DetectedShell;
use codex_shell_command::shell_detect::ShellType;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use test_case::test_case;

#[test_case("\n"; "lf")]
#[test_case("\r\n"; "crlf")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn powershell_patch_pipeline_emits_file_diff(newline: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "requires a controlled local PowerShell session shell"
    );

    // Interception must happen before shell execution, so PowerShell need not be installed.
    let shell = DetectedShell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from("pwsh"),
    };
    let builder = test_codex().with_user_shell(shell.into());
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let file_name = "notes/experiment-log.md";
    harness.write_file(file_name, "old\n").await?;
    let content = "The $model index resolves **2,194 tensors**; `ticks` stay literal.";
    let patch = format!(
        "*** Begin Patch\n*** Update File: {file_name}\n@@\n-old\n+{content}\n*** End Patch"
    );
    let script = format!("@'\n{patch}\n'@ | apply_patch").replace('\n', newline);
    let call_id = "powershell-patch";
    let args = json!({ "cmd": script, "login": false, "yield_time_ms": 5_000 });
    let mock = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let test = harness.test();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, &test.config.cwd);
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Update the experiment log using the patch.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;

    let mut patch_changes = Vec::new();
    let mut completion = None;
    let mut command_events = Vec::new();
    wait_for_event(&test.codex, |event| {
        match event {
            EventMsg::PatchApplyBegin(begin) if begin.call_id == call_id => {
                patch_changes.push(begin.changes.clone());
            }
            EventMsg::PatchApplyEnd(end) if end.call_id == call_id => {
                patch_changes.push(end.changes.clone());
                completion = Some((end.success, end.status.clone(), end.stderr.clone()));
            }
            EventMsg::ExecCommandBegin(begin) if begin.call_id == call_id => {
                command_events.push("begin");
            }
            EventMsg::ExecCommandEnd(end) if end.call_id == call_id => {
                command_events.push("end");
            }
            _ => {}
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let expected_changes = HashMap::from([(
        harness.path(file_name),
        FileChange::Update {
            unified_diff: format!("@@ -1 +1 @@\n-old\n+{content}\n"),
            move_path: None,
        },
    )]);
    assert_eq!(
        patch_changes,
        vec![expected_changes.clone(), expected_changes]
    );
    assert_eq!(
        completion,
        Some((true, PatchApplyStatus::Completed, String::new()))
    );
    assert_eq!(command_events, Vec::<&str>::new());
    assert_eq!(
        harness.read_file_text(file_name).await?,
        format!("{content}\n")
    );
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("patch output");
    assert!(
        output.contains("Success. Updated the following files:"),
        "{output}"
    );
    Ok(())
}
