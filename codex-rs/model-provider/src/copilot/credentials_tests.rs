use pretty_assertions::assert_eq;

use super::*;

#[test]
fn credential_debug_output_redacts_tokens() {
    let credential = CopilotCredential {
        token: "github-secret".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        machine_id: Some("machine-id".to_string()),
        source: CopilotCredentialSource::StoredOAuth,
    };

    assert_eq!(
        format!("{credential:?}"),
        "CopilotCredential { token: \"[REDACTED]\", base_url: \
         \"https://api.githubcopilot.com\", machine_id: Some(\"machine-id\"), source: \
         StoredOAuth }"
    );
}

#[test]
fn direct_credentials_follow_provider_specific_precedence() {
    let api = load_with(
        |name| match name {
            COPILOT_API_TOKEN_ENV => Some("api-token".to_string()),
            COPILOT_API_URL_ENV => Some("https://api.example.test".to_string()),
            COPILOT_GITHUB_TOKEN_ENV | GH_TOKEN_ENV | GITHUB_TOKEN_ENV => {
                Some("lower-priority".to_string())
            }
            _ => None,
        },
        || panic!("API token must precede keyring access"),
    );
    let dedicated = load_with(
        |name| match name {
            COPILOT_GITHUB_TOKEN_ENV => Some("dedicated-token".to_string()),
            GH_TOKEN_ENV | GITHUB_TOKEN_ENV => Some("generic-token".to_string()),
            _ => None,
        },
        || panic!("dedicated token must precede keyring access"),
    );
    let stored = load_with(
        |name| match name {
            GH_TOKEN_ENV | GITHUB_TOKEN_ENV => Some("generic-token".to_string()),
            _ => None,
        },
        || {
            Ok(Some((
                "stored-token".to_string(),
                "stored-machine".to_string(),
            )))
        },
    );
    let generic = load_with(
        |name| match name {
            GH_TOKEN_ENV => Some("gh-token".to_string()),
            GITHUB_TOKEN_ENV => Some("github-token".to_string()),
            _ => None,
        },
        || Ok(None),
    );
    let github = load_with(
        |name| match name {
            GITHUB_TOKEN_ENV => Some("github-token".to_string()),
            _ => None,
        },
        || Ok(None),
    );

    assert_eq!(
        api,
        Ok(CopilotCredential {
            token: "api-token".to_string(),
            base_url: "https://api.example.test".to_string(),
            machine_id: None,
            source: CopilotCredentialSource::ApiTokenEnvironment,
        })
    );
    assert_eq!(
        dedicated,
        Ok(CopilotCredential {
            token: "dedicated-token".to_string(),
            base_url: COPILOT_API_URL.to_string(),
            machine_id: None,
            source: CopilotCredentialSource::DedicatedGitHubEnvironment,
        })
    );
    assert_eq!(
        stored,
        Ok(CopilotCredential {
            token: "stored-token".to_string(),
            base_url: COPILOT_API_URL.to_string(),
            machine_id: Some("stored-machine".to_string()),
            source: CopilotCredentialSource::StoredOAuth,
        })
    );
    assert_eq!(
        generic,
        Ok(CopilotCredential {
            token: "gh-token".to_string(),
            base_url: COPILOT_API_URL.to_string(),
            machine_id: None,
            source: CopilotCredentialSource::GhTokenEnvironment,
        })
    );
    assert_eq!(
        github,
        Ok(CopilotCredential {
            token: "github-token".to_string(),
            base_url: COPILOT_API_URL.to_string(),
            machine_id: None,
            source: CopilotCredentialSource::GitHubTokenEnvironment,
        })
    );
}

#[test]
fn whitespace_only_environment_values_are_absent() {
    let credential = load_with(
        |name| match name {
            COPILOT_API_TOKEN_ENV | COPILOT_GITHUB_TOKEN_ENV | GH_TOKEN_ENV => {
                Some(" \t ".to_string())
            }
            GITHUB_TOKEN_ENV => Some(" github-token ".to_string()),
            _ => None,
        },
        || Ok(None),
    );

    assert_eq!(
        credential,
        Ok(CopilotCredential {
            token: "github-token".to_string(),
            base_url: COPILOT_API_URL.to_string(),
            machine_id: None,
            source: CopilotCredentialSource::GitHubTokenEnvironment,
        })
    );
}

#[test]
fn missing_native_credential_and_store_failure_are_distinct() {
    let absent = load_with(|_| None, || Ok(None));
    let store_error = load_with(
        |_| None,
        || {
            Err(CredentialLoadError::credential_store(
                "credential store unavailable".to_string(),
            ))
        },
    );

    assert_eq!(absent, Err(CredentialLoadError::missing()));
    assert_eq!(
        store_error,
        Err(CredentialLoadError::credential_store(
            "credential store unavailable".to_string()
        ))
    );
}
