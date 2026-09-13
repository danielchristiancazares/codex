use codex_core::config::Config;
use codex_login::ConnectionProvider;
use codex_login::ConnectionStore;
use codex_login::GitHubCopilotAuth;
use codex_utils_cli::CliConfigOverrides;

use crate::login::init_login_file_logging;
use crate::login::load_config_or_exit;

fn scoped_copilot_auth(
    config: &Config,
) -> std::io::Result<(ConnectionStore, std::path::PathBuf, GitHubCopilotAuth)> {
    let store = ConnectionStore::new(
        config.codex_home.to_path_buf(),
        config.cli_auth_credentials_store_mode,
        config.auth_keyring_backend_kind(),
        config.auth_route_config(),
    );
    let credential_home = store.credential_home_for_provider(ConnectionProvider::Copilot)?;
    let auth = GitHubCopilotAuth::new_in(
        &credential_home,
        config.http_client_factory(),
        config.cli_auth_credentials_store_mode,
    );
    Ok((store, credential_home, auth))
}

pub async fn run_login_with_copilot(cli_config_overrides: CliConfigOverrides, force: bool) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let _login_log_guard = init_login_file_logging(&config);
    tracing::info!(force, "starting native GitHub Copilot login flow");
    let (_, _, auth) = match scoped_copilot_auth(&config) {
        Ok(scope) => scope,
        Err(error) => {
            eprintln!("Error resolving GitHub Copilot login: {error}");
            std::process::exit(1);
        }
    };
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
    let (_, _, auth) = match scoped_copilot_auth(&config) {
        Ok(scope) => scope,
        Err(error) => {
            eprintln!("Error resolving GitHub Copilot login: {error}");
            std::process::exit(1);
        }
    };
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
    let (store, credential_home, auth) = match scoped_copilot_auth(&config) {
        Ok(scope) => scope,
        Err(error) => {
            eprintln!("Error resolving GitHub Copilot login: {error}");
            std::process::exit(1);
        }
    };
    match auth.logout() {
        Ok(true) => {
            if let Err(error) = store.forget_logged_out_scope(&credential_home) {
                eprintln!("Error updating saved GitHub Copilot accounts: {error}");
                std::process::exit(1);
            }
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
