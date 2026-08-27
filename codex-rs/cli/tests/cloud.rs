use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

#[test]
fn cloud_command_is_stubbed() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["cloud", "list", "--json"])
        .assert()
        .failure()
        .stderr(contains("Codex Cloud is disabled in this build"));

    Ok(())
}

#[test]
fn cloud_tasks_alias_is_stubbed() -> Result<()> {
    let codex_home = TempDir::new()?;

    codex_command(codex_home.path())?
        .args(["cloud-tasks", "status", "task-123"])
        .assert()
        .failure()
        .stderr(contains("Codex Cloud is disabled in this build"));

    Ok(())
}
