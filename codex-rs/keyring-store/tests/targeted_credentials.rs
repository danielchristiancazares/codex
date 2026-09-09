use std::any::Any;
use std::sync::Arc;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use codex_keyring_store::tests::MockKeyringStore;
use keyring::credential::Credential;
use keyring::credential::CredentialApi;
use keyring::credential::CredentialBuilderApi;
use keyring::credential::CredentialPersistence;
use keyring::mock::MockCredential;
use pretty_assertions::assert_eq;

struct SharedCredential(Arc<MockCredential>);

impl CredentialApi for SharedCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        self.0.set_secret(secret)
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.0.get_secret()
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        self.0.delete_credential()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TestCredentialBuilder {
    credentials: MockKeyringStore,
}

impl CredentialBuilderApi for TestCredentialBuilder {
    fn build(
        &self,
        target: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        // Model Windows target identity without accessing the OS credential store.
        let key = target.map_or_else(|| format!("{user}.{service}"), str::to_owned);
        Ok(Box::new(SharedCredential(
            self.credentials.credential(&key),
        )))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

#[test]
fn default_store_preserves_explicit_targets_and_storage_errors() {
    let credentials = MockKeyringStore::default();
    // This is the only test in this binary that uses the global keyring builder.
    keyring::set_default_credential_builder(Box::new(TestCredentialBuilder {
        credentials: credentials.clone(),
    }));
    let store: Arc<dyn KeyringStore> = Arc::new(DefaultKeyringStore);
    let service = "test-service";
    let account = "test-account";
    let first_target = "codex/test/first";
    let second_target = "codex/test/second";

    assert_eq!(
        store
            .load_with_target(first_target, service, account)
            .unwrap(),
        None
    );
    assert!(
        !store
            .delete_with_target(first_target, service, account)
            .unwrap()
    );
    store.save(service, account, "ordinary-secret").unwrap();
    store
        .save_with_target(first_target, service, account, "first-secret")
        .unwrap();
    store
        .save_with_target(second_target, service, account, "second-secret")
        .unwrap();
    assert_eq!(
        credentials.saved_value(first_target),
        Some("first-secret".to_string())
    );
    assert_eq!(
        store
            .load_with_target(first_target, service, account)
            .unwrap(),
        Some("first-secret".to_string())
    );
    assert_eq!(
        store
            .load_with_target(second_target, service, account)
            .unwrap(),
        Some("second-secret".to_string())
    );

    credentials.set_error(
        first_target,
        keyring::Error::Invalid("credential".to_string(), "load failure".to_string()),
    );
    assert!(matches!(
        store
            .load_with_target(first_target, service, account)
            .unwrap_err()
            .into_error(),
        keyring::Error::Invalid(_, _)
    ));
    credentials.set_error(
        first_target,
        keyring::Error::Invalid("credential".to_string(), "save failure".to_string()),
    );
    assert!(matches!(
        store
            .save_with_target(first_target, service, account, "replacement-secret")
            .unwrap_err()
            .into_error(),
        keyring::Error::Invalid(_, _)
    ));
    credentials.set_error(
        first_target,
        keyring::Error::Invalid("credential".to_string(), "delete failure".to_string()),
    );
    assert!(matches!(
        store
            .delete_with_target(first_target, service, account)
            .unwrap_err()
            .into_error(),
        keyring::Error::Invalid(_, _)
    ));
    assert_eq!(
        store
            .load_with_target(first_target, service, account)
            .unwrap(),
        Some("first-secret".to_string())
    );

    assert!(
        store
            .delete_with_target(first_target, service, account)
            .unwrap()
    );
    assert_eq!(
        store
            .load_with_target(first_target, service, account)
            .unwrap(),
        None
    );
    assert!(
        !store
            .delete_with_target(first_target, service, account)
            .unwrap()
    );
    assert_eq!(
        store
            .load_with_target(second_target, service, account)
            .unwrap(),
        Some("second-secret".to_string())
    );
    assert_eq!(
        store.load(service, account).unwrap(),
        Some("ordinary-secret".to_string())
    );
    assert!(store.delete(service, account).unwrap());
    assert_eq!(store.load(service, account).unwrap(), None);
    assert!(!store.delete(service, account).unwrap());
}

#[test]
fn targeted_methods_default_to_service_and_account_for_existing_backends() {
    let store: Arc<dyn KeyringStore> = Arc::new(MockKeyringStore::default());
    let service = "test-service";
    let account = "test-account";
    let target = "codex/test/fallback";

    store
        .save_with_target(target, service, account, "targeted-secret")
        .unwrap();
    assert_eq!(
        store.load(service, account).unwrap(),
        Some("targeted-secret".to_string())
    );
    store.save(service, account, "ordinary-secret").unwrap();
    assert_eq!(
        store.load_with_target(target, service, account).unwrap(),
        Some("ordinary-secret".to_string())
    );
    assert!(store.delete_with_target(target, service, account).unwrap());
    assert_eq!(store.load(service, account).unwrap(), None);
    assert!(!store.delete_with_target(target, service, account).unwrap());
}
