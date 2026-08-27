use std::sync::Arc;

use codex_keyring_store::tests::MockKeyringStore;
use pretty_assertions::assert_eq;

use super::*;

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

#[test]
fn credential_store_round_trips_token_and_machine_identity_together() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let store = CopilotCredentialStore::new_with_keyring(codex_home.path(), keyring);
    let expected = GitHubCopilotCredential {
        token: "github-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };

    store.save(&expected).expect("save credential");

    assert_eq!(store.load(), Ok(Some(expected)));
    assert_eq!(store.delete(), Ok(true));
    assert_eq!(store.load(), Ok(None));
}

#[test]
fn credential_store_rejects_unpaired_or_malformed_state() {
    let missing_machine = validate_stored_credential(StoredCredential {
        version: CREDENTIAL_VERSION,
        github_token: "github-secret".to_string(),
        machine_id: String::new(),
    });
    let empty_token = validate_stored_credential(StoredCredential {
        version: CREDENTIAL_VERSION,
        github_token: String::new(),
        machine_id: "b".repeat(MACHINE_ID_HEX_BYTES),
    });

    assert_eq!(
        missing_machine,
        Err(GitHubCopilotAuthError::malformed_credential(
            "stored GitHub Copilot machine ID is invalid"
        ))
    );
    assert_eq!(
        empty_token,
        Err(GitHubCopilotAuthError::malformed_credential(
            "stored GitHub Copilot token has an invalid length"
        ))
    );
}

#[test]
fn generated_machine_identity_is_a_sha256_hex_digest() {
    let machine_id = new_machine_id();

    assert_eq!(machine_id.len(), MACHINE_ID_HEX_BYTES);
    assert!(machine_id.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
