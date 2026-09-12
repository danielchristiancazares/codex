use crate::MaybeApplyPatch;
use crate::maybe_parse_apply_patch;
use crate::parse_patch;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn powershell_literal_patch_pipelines_preserve_payload() {
    let patch = "*** Begin Patch\n*** Update File: notes/experiment-log.md\n@@\n-old\n+The $model index resolves **2,194 tensors**; `ticks` and 'quotes' stay literal.\n*** End Patch";
    for (cwd, shell) in [
        ("file:///workspace", "pwsh"),
        ("file:///workspace", "/usr/bin/pwsh"),
        ("file:///C:/workspace", "powershell.exe"),
        (
            "file:///C:/workspace",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
        ),
    ] {
        for flags in [vec!["-Command"], vec!["-NoProfile", "-Command"]] {
            for newline in ["\n", "\r\n"] {
                for command in ["apply_patch", "applypatch", "APPLY_PATCH"] {
                    let patch = patch.replace('\n', newline);
                    let script = format!(" \t@' \t{newline}{patch}{newline}'@ | {command}\n");
                    let args: Vec<String> = std::iter::once(shell)
                        .chain(flags.iter().copied())
                        .chain(std::iter::once(script.as_str()))
                        .map(str::to_string)
                        .collect();
                    assert_eq!(
                        maybe_parse_apply_patch(&args, &PathUri::parse(cwd).unwrap()),
                        MaybeApplyPatch::Body(parse_patch(&patch).unwrap()),
                        "{args:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn powershell_patch_pipeline_requires_a_single_literal_invocation() {
    let patch = "*** Begin Patch\n*** Add File: example.txt\n+hello $name\n*** End Patch";
    for script in [
        format!("@\"\n{patch}\n\"@ | apply_patch"),
        format!("@'{patch}\n'@ | apply_patch"),
        format!("@'\n{patch}\n  '@ | apply_patch"),
        format!("@'\n{patch}\n'@"),
        format!("@'\n{patch}\n'@ | Write-Output"),
        format!("@'\n{patch}\n'@ || apply_patch"),
        format!("@'\n{patch}\n'@ | apply_patch extra"),
        format!("@'\n{patch}\n'@ | apply_patch > output.txt"),
        format!("@'\n{patch}\n'@ | apply_patch; Write-Output done"),
        format!("@'\n{patch}\n'@ | apply_patch\nWrite-Output done"),
        format!("@'\n{patch}\n'@ | apply_patch | Write-Output"),
        format!("Write-Output before; @'\n{patch}\n'@ | apply_patch"),
        format!("@'\n'@; Write-Output before\n{patch}\n'@ | apply_patch"),
    ] {
        let args = vec!["pwsh".to_string(), "-Command".to_string(), script];
        assert_eq!(
            maybe_parse_apply_patch(&args, &PathUri::parse("file:///workspace").unwrap()),
            MaybeApplyPatch::NotApplyPatch,
            "{args:?}",
        );
    }
}

#[test]
fn powershell_patch_pipeline_reports_invalid_patch() {
    let patch = "*** Begin Patch\n*** Update File: example.txt\n*** End Patch";
    let args = vec![
        "pwsh".to_string(),
        "-Command".to_string(),
        format!("@'\n{patch}\n'@ | apply_patch"),
    ];
    assert_eq!(
        maybe_parse_apply_patch(&args, &PathUri::parse("file:///workspace").unwrap()),
        MaybeApplyPatch::PatchParseError(parse_patch(patch).unwrap_err()),
    );
}
