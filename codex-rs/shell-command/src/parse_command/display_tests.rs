use super::*;
use pretty_assertions::assert_eq;

#[test]
fn summaries_preserve_commands_with_globs_and_unclassified_neighbors() {
    let script = "sed -n '1,20p' src/first.rs\n\
                  rg -n 'fn on_task_complete' src/*.rs\n\
                  command -v cargo-dylint dylint-link\n\
                  sed -n '20,40p' src/second.rs\n\
                  rustup toolchain list";
    assert_eq!(
        parse_shell_script_for_display(script),
        vec![
            ParsedCommand::Read {
                cmd: "sed -n '1,20p' src/first.rs".to_string(),
                name: "first.rs".to_string(),
                path: PathBuf::from("src/first.rs"),
            },
            ParsedCommand::Search {
                cmd: "rg -n 'fn on_task_complete' src/*.rs".to_string(),
                query: Some("fn on_task_complete".to_string()),
                path: Some("*.rs".to_string()),
            },
            ParsedCommand::Unknown {
                cmd: "command -v cargo-dylint dylint-link".to_string(),
            },
            ParsedCommand::Read {
                cmd: "sed -n '20,40p' src/second.rs".to_string(),
                name: "second.rs".to_string(),
                path: PathBuf::from("src/second.rs"),
            },
            ParsedCommand::Unknown {
                cmd: "rustup toolchain list".to_string(),
            },
        ]
    );
    assert_eq!(crate::bash::parse_shell_script_into_commands(script), None);
}

#[test]
fn summaries_retain_shell_structure_and_every_independent_command() {
    for statement in [
        "git diff > patch.txt",
        "cat $(generate_path)",
        "if test -f ready; then cargo test; fi",
        "for file in *.rs; do cat \"$file\"; done",
    ] {
        let script = format!("rg needle src\n{statement}\nsed -n '1,20p' src/lib.rs");
        assert_eq!(
            parse_shell_script_for_display(&script),
            vec![
                ParsedCommand::Search {
                    cmd: "rg needle src".to_string(),
                    query: Some("needle".to_string()),
                    path: Some("src".to_string()),
                },
                ParsedCommand::Unknown {
                    cmd: statement.to_string(),
                },
                ParsedCommand::Read {
                    cmd: "sed -n '1,20p' src/lib.rs".to_string(),
                    name: "lib.rs".to_string(),
                    path: PathBuf::from("src/lib.rs"),
                },
            ],
            "{statement}"
        );
    }
}

#[test]
fn summaries_keep_working_directories_and_standalone_output_commands() {
    assert_eq!(
        parse_shell_script_for_display(
            "cd project && cat src/lib.rs\nprintf checkpoint\ncargo test"
        ),
        vec![
            ParsedCommand::Read {
                cmd: "cat src/lib.rs".to_string(),
                name: "lib.rs".to_string(),
                path: PathBuf::from("project/src/lib.rs"),
            },
            ParsedCommand::Unknown {
                cmd: "printf checkpoint".to_string(),
            },
            ParsedCommand::Unknown {
                cmd: "cargo test".to_string(),
            },
        ]
    );
}
