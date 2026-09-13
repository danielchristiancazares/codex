use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn catalog_reuses_its_binding_and_rebuilds_from_settings_when_the_binding_changes() {
    let first_home = tempfile::tempdir().expect("first credential home");
    let second_home = tempfile::tempdir().expect("second credential home");
    let first_auth = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("first-account"),
        first_home.path().to_path_buf(),
    );
    let second_auth = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("second-account"),
        second_home.path().to_path_buf(),
    );
    let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
    let mut expected =
        codex_models_manager::bundled_models_response().expect("bundled model catalog");
    expected.models.truncate(1);
    let settings = ModelCatalogSettings {
        model_catalog: Some(expected.clone()),
        api_key_model_discovery_enabled: true,
    };
    let initial = build_models_manager(provider.clone(), first_auth.clone(), settings.clone());
    let scoped = ScopedModelCatalog::new(provider.clone(), &first_auth, initial.clone());
    assert!(Arc::ptr_eq(
        &initial,
        &scoped.for_provider(&provider, &first_auth, settings.clone())
    ));

    let other_provider = ModelProviderInfo {
        base_url: Some("https://provider.example".to_string()),
        ..provider.clone()
    };
    let mut previous = initial;
    for (provider, auth) in [(&other_provider, &first_auth), (&other_provider, &second_auth)] {
        expected.models[0].slug.push_str("-selected");
        let manager = scoped.for_provider(
            provider,
            auth,
            ModelCatalogSettings {
                model_catalog: Some(expected.clone()),
                ..settings.clone()
            },
        );
        assert!(!Arc::ptr_eq(&previous, &manager));
        assert!(Arc::ptr_eq(&scoped.current(), &manager));
        assert_eq!(
            manager
                .raw_model_catalog(
                    RefreshStrategy::Offline,
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                )
                .await,
            expected
        );
        previous = manager;
    }
}
