use codex_login::GitHubCopilotAuth;
use codex_utils_cli::CliConfigOverrides;

use crate::login::init_login_file_logging;
use crate::login::load_config_or_exit;

pub async fn run_login_with_copilot(cli_config_overrides: CliConfigOverrides, force: bool) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let _login_log_guard = init_login_file_logging(&config);
    tracing::info!(force, "starting native GitHub Copilot login flow");
    let auth = GitHubCopilotAuth::new_in(&config.codex_home, config.http_client_factory());
    match auth.login(force).await {
        Ok(account) => {
            eprintln!("Successfully logged in to GitHub Copilot as {account}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("Error logging in to GitHub Copilot: {error}");
            std::process::exit(1);
        }
    }
}

pub async fn run_copilot_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let auth = GitHubCopilotAuth::new_in(&config.codex_home, config.http_client_factory());
    match auth.account().await {
        Ok(Some(account)) => {
            eprintln!("Logged in to GitHub Copilot as {account}");
            std::process::exit(0);
        }
        Ok(None) => {
            eprintln!("Not logged in to GitHub Copilot");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("Error checking GitHub Copilot login: {error}");
            std::process::exit(1);
        }
    }
}

pub async fn run_copilot_logout(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let auth = GitHubCopilotAuth::new_in(&config.codex_home, config.http_client_factory());
    match auth.logout() {
        Ok(true) => {
            eprintln!("Successfully logged out of GitHub Copilot");
            std::process::exit(0);
        }
        Ok(false) => {
            eprintln!("Not logged in to GitHub Copilot");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("Error logging out of GitHub Copilot: {error}");
            std::process::exit(1);
        }
    }
}
