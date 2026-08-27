use clap::Args;
use codex_utils_cli::CliConfigOverrides;
use std::ffi::OsString;

/// Placeholder for Codex Cloud commands in builds that omit the Cloud client.
#[derive(Debug, Args)]
pub(crate) struct CloudCommand {
    #[clap(skip)]
    pub(crate) config_overrides: CliConfigOverrides,

    /// Ignored arguments accepted for compatibility with existing invocations.
    #[arg(
        value_name = "ARGS",
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    args: Vec<OsString>,
}

pub(crate) fn run(command: CloudCommand) -> anyhow::Result<()> {
    let _ = command.args;
    anyhow::bail!("Codex Cloud is disabled in this build")
}
