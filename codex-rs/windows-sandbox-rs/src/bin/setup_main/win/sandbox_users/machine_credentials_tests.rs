use super::MachineCredentialSource;
use super::MachineCredentialStore;
use super::MachineSandboxCredentials;
use super::ResolvedMachineCredentials;
use super::SandboxAccountCredentials;
use super::load_or_create_machine_credentials;
use anyhow::Result;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct MemoryCredentialStore {
    credentials: Option<MachineSandboxCredentials>,
    save_count: usize,
}

impl MachineCredentialStore for MemoryCredentialStore {
    fn load(&mut self) -> Result<Option<MachineSandboxCredentials>> {
        Ok(self.credentials.clone())
    }

    fn save(&mut self, credentials: &MachineSandboxCredentials) -> Result<()> {
        self.credentials = Some(credentials.clone());
        self.save_count += 1;
        Ok(())
    }
}

fn credentials(password_owner: &str) -> MachineSandboxCredentials {
    MachineSandboxCredentials::new(
        SandboxAccountCredentials {
            username: "offline".to_string(),
            password: format!("{password_owner}-offline-password"),
        },
        SandboxAccountCredentials {
            username: "online".to_string(),
            password: format!("{password_owner}-online-password"),
        },
    )
}

#[test]
fn separate_codex_homes_reuse_the_machine_owned_credentials() {
    let home_a_credentials = credentials("home-a");
    let home_b_credentials = credentials("home-b");
    let mut store = MemoryCredentialStore::default();

    let home_a = load_or_create_machine_credentials(&mut store, "offline", "online", || {
        Ok(home_a_credentials.clone())
    })
    .expect("initialize credentials from home A");
    let home_b = load_or_create_machine_credentials(&mut store, "offline", "online", || {
        Ok(home_b_credentials)
    })
    .expect("load credentials for home B");

    assert_eq!(
        (home_a, home_b, store.save_count),
        (
            ResolvedMachineCredentials {
                credentials: home_a_credentials.clone(),
                source: MachineCredentialSource::Initialized,
            },
            ResolvedMachineCredentials {
                credentials: home_a_credentials,
                source: MachineCredentialSource::Stored,
            },
            1,
        )
    );
}
