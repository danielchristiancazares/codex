//! Resolves local provider display data without consulting local settings for remote servers.

use crate::AppServerTarget;
use crate::legacy_core::config::Config;
use codex_model_provider::create_model_provider;
use url::Url;

#[derive(Clone, Debug)]
struct NonEmptyString(String);

#[derive(Debug, thiserror::Error)]
#[error("provider display data must be nonempty")]
struct InvalidProviderDisplay;

impl NonEmptyString {
    fn new(text: &str) -> Result<Self, InvalidProviderDisplay> {
        if text.trim().is_empty() {
            return Err(InvalidProviderDisplay);
        }
        Ok(Self(text.trim().to_owned()))
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeProviderStatus(Presentation);

#[derive(Clone, Debug, Default)]
enum Presentation {
    #[default]
    UseServerIdentity,
    ShowLocalProvider {
        provider_id: NonEmptyString,
        label: NonEmptyString,
    },
}

impl RuntimeProviderStatus {
    pub(crate) async fn resolve(config: &Config, target: &AppServerTarget) -> Self {
        if target.uses_remote_workspace() {
            return Self::default();
        }
        let Ok(provider_id) = NonEmptyString::new(&config.model_provider_id) else {
            return Self::default();
        };
        let provider = &config.model_provider;
        let name = NonEmptyString::new(&provider.name).unwrap_or_else(|_| provider_id.clone());
        let label = if provider.is_copilot() {
            name
        } else {
            match create_model_provider(provider.clone(), /*auth_manager*/ None)
                .runtime_base_url()
                .await
            {
                Ok(Some(raw_url)) => match sanitize_endpoint(&raw_url) {
                    Ok(endpoint) => NonEmptyString(format!("{} - {}", name.0, endpoint.0)),
                    Err(_) => name,
                },
                Ok(None) | Err(_) => name,
            }
        };
        Self(Presentation::ShowLocalProvider { provider_id, label })
    }

    pub(crate) fn render(&self, server_provider_id: &str) -> String {
        match &self.0 {
            Presentation::ShowLocalProvider { provider_id, label }
                if provider_id.0 == server_provider_id =>
            {
                label.0.clone()
            }
            Presentation::ShowLocalProvider { .. } | Presentation::UseServerIdentity => {
                server_provider_id.to_owned()
            }
        }
    }
}

fn sanitize_endpoint(raw: &str) -> Result<NonEmptyString, InvalidProviderDisplay> {
    let mut url = Url::parse(raw.trim()).map_err(|_| InvalidProviderDisplay)?;
    // Opaque URLs cannot safely have userinfo removed; retain the provider name in that case.
    url.set_username("").map_err(|_| InvalidProviderDisplay)?;
    url.set_password(None).map_err(|_| InvalidProviderDisplay)?;
    url.set_query(None);
    url.set_fragment(None);
    NonEmptyString::new(url.as_str().trim_end_matches('/'))
}

#[cfg(test)]
#[path = "provider_endpoint_tests.rs"]
mod tests;
