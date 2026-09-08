use super::*;
use pretty_assertions::assert_eq;

#[test]
fn expands_inspection_batches_without_hiding_other_commands() {
    let inspections = "rg -n needle src\nsed -n '1,20p' src/lib.rs\ngit diff HEAD -- src/lib.rs";
    let parsed = vec![ParsedCommand::Unknown {
        cmd: inspections.to_string(),
    }];
    assert_eq!(
        expand_command_summaries(parsed),
        parse_shell_script_for_display(inspections)
    );

    for script in [
        "rg needle src\ncargo test",
        "cat src/lib.rs\ngit reset --hard",
        "rg needle src\ngit diff --output patch.txt",
        "rg needle src\ngit diff --output=patch.txt",
        "rg needle src\ngit diff > patch.txt",
        "git diff $(touch marker)",
        "git diff; rm marker",
    ] {
        let parsed = vec![ParsedCommand::Unknown {
            cmd: script.to_string(),
        }];
        let summaries = expand_command_summaries(parsed);
        assert!(
            summaries
                .iter()
                .any(|command| !is_exploration_command(command)),
            "{script}"
        );
    }
}

#[test]
fn groups_plain_git_inspections_with_checkout_and_pager_options() {
    for command in [
        "git diff HEAD -- src/lib.rs",
        "git --no-pager -C 'working tree' status --short",
        "/usr/bin/git -C repo log -5 --oneline",
        "git.exe show HEAD:src/lib.rs",
    ] {
        assert!(is_exploration_command(&ParsedCommand::Unknown {
            cmd: command.to_string(),
        }));
    }
}
