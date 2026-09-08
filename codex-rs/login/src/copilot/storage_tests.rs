use std::sync::Arc;

use codex_keyring_store::tests::MockKeyringStore;
use keyring::Error as KeyringError;
use pretty_assertions::assert_eq;

use super::*;

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

#[test]
fn credential_store_round_trips_token_and_machine_identity_together() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let store = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::Keyring,
        keyring,
    );
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

#[test]
fn file_credentials_survive_reopening_without_accessing_the_keyring() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let store = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        keyring.clone(),
    );
    keyring.set_error(
        &store.account,
        KeyringError::NoStorageAccess(Box::new(std::io::Error::other(
            "User interaction is not allowed",
        ))),
    );
    let expected = GitHubCopilotCredential {
        token: "file-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };

    assert_eq!(store.load(), Ok(None));
    store.save(&expected).expect("save private credential file");
    let reopened = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        keyring.clone(),
    );
    assert_eq!(reopened.load(), Ok(Some(expected)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&store.file_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600,
        );
    }
    assert_eq!(reopened.delete(), Ok(true));
    assert_eq!(reopened.load(), Ok(None));
    assert!(
        keyring.load(KEYRING_SERVICE, &store.account).is_err(),
        "the queued keyring error must remain untouched throughout the file lifecycle"
    );
}

#[test]
fn auto_fallback_remains_authoritative_when_the_keyring_recovers() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let store = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::Auto,
        keyring.clone(),
    );
    let mut expected = GitHubCopilotCredential {
        token: "old-keyring-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };
    store.save(&expected).expect("seed keyring credential");
    assert_eq!(store.load(), Ok(Some(expected.clone())));
    keyring.set_error(
        &store.account,
        KeyringError::NoStorageAccess(Box::new(std::io::Error::other("keyring unavailable"))),
    );
    expected.token = "new-file-secret".to_string();
    store.save(&expected).expect("fall back to private file");

    let reopened = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::Auto,
        keyring.clone(),
    );
    assert_eq!(reopened.load(), Ok(Some(expected.clone())));
    expected.token = "refreshed-file-secret".to_string();
    reopened.save(&expected).expect("update authoritative file");
    assert_eq!(reopened.load(), Ok(Some(expected.clone())));

    keyring.set_error(
        &store.account,
        KeyringError::NoStorageAccess(Box::new(std::io::Error::other(
            "keyring unavailable during logout",
        ))),
    );
    assert!(reopened.delete().is_err());
    assert_eq!(reopened.load(), Ok(Some(expected)));
    assert_eq!(reopened.delete(), Ok(true));
    assert_eq!(reopened.load(), Ok(None));
}

#[cfg(unix)]
#[test]
fn file_replacement_is_private_and_does_not_follow_an_existing_symlink() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let store = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        Arc::new(MockKeyringStore::default()),
    );
    let unrelated = codex_home.path().join("unrelated");
    std::fs::write(&unrelated, "leave unchanged").unwrap();
    symlink(&unrelated, &store.file_path).unwrap();
    let expected = GitHubCopilotCredential {
        token: "file-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };

    store.save(&expected).expect("replace symlink atomically");

    assert_eq!(store.load(), Ok(Some(expected)));
    assert_eq!(
        std::fs::read_to_string(unrelated).unwrap(),
        "leave unchanged"
    );
    assert_eq!(
        std::fs::metadata(&store.file_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600,
    );
}

#[test]
fn credential_files_are_bounded_and_decode_errors_do_not_expose_secrets() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let store = CopilotCredentialStore::new(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        Arc::new(MockKeyringStore::default()),
    );
    std::fs::write(&store.file_path, "sensitive-invalid-json").unwrap();
    assert_eq!(
        store.load(),
        Err(GitHubCopilotAuthError::malformed_credential(
            "decode stored GitHub Copilot credential"
        )),
    );
    std::fs::write(&store.file_path, vec![b' '; 128 * 1024 + 1]).unwrap();
    assert_eq!(
        store.load().unwrap_err().kind(),
        super::super::GitHubCopilotAuthErrorKind::CredentialStore
    );
}
