use super::SandboxUserRecord;
use super::SandboxUsersFile;
use super::load_home_credentials;
use super::machine_credentials::MachineSandboxCredentials;
use super::machine_credentials::SandboxAccountCredentials;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use codex_windows_sandbox::SETUP_VERSION;
use codex_windows_sandbox::dpapi_protect;
use codex_windows_sandbox::sandbox_secrets_dir;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

#[test]
fn previous_home_credentials_can_seed_the_machine_store() {
    let codex_home = TempDir::new().expect("create CODEX_HOME");
    let secrets_dir = sandbox_secrets_dir(codex_home.path());
    fs::create_dir_all(&secrets_dir).expect("create sandbox secrets directory");
    let users = SandboxUsersFile {
        version: SETUP_VERSION - 1,
        offline: SandboxUserRecord {
            username: "offline".to_string(),
            password: protected_password("offline-password"),
        },
        online: SandboxUserRecord {
            username: "online".to_string(),
            password: protected_password("online-password"),
        },
    };
    fs::write(
        secrets_dir.join("sandbox_users.json"),
        serde_json::to_vec(&users).expect("serialize sandbox users"),
    )
    .expect("write sandbox users");

    let credentials = load_home_credentials(codex_home.path(), "offline", "online")
        .expect("load home credentials");

    assert_eq!(
        credentials,
        Some(MachineSandboxCredentials::new(
            SandboxAccountCredentials {
                username: "offline".to_string(),
                password: "offline-password".to_string(),
            },
            SandboxAccountCredentials {
                username: "online".to_string(),
                password: "online-password".to_string(),
            },
        ))
    );
}

fn protected_password(password: &str) -> String {
    BASE64.encode(dpapi_protect(password.as_bytes()).expect("protect password"))
}
