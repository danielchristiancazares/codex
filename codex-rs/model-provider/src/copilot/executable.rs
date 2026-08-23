use std::path::PathBuf;

const COPILOT_CLI_PATH_ENV: &str = "COPILOT_CLI_PATH";

pub(super) fn resolve() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(COPILOT_CLI_PATH_ENV) {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(format!("{COPILOT_CLI_PATH_ENV} must be an absolute path"));
        }
        return Ok(path);
    }

    #[cfg(windows)]
    let executable = which::which("copilot.cmd").or_else(|_| which::which("copilot.exe"));
    #[cfg(not(windows))]
    let executable = which::which("copilot");

    executable.map_err(|_| {
        format!(
            "Copilot CLI was not found; install it and run `copilot login`, or set {COPILOT_CLI_PATH_ENV}"
        )
    })
}
