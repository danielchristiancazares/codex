use pretty_assertions::assert_eq;

use super::*;

#[test]
fn credential_debug_output_redacts_tokens() {
    let credentials = [
        CopilotCredential::ApiToken {
            token: "direct-secret".to_string(),
            api_url: "https://api.githubcopilot.com".to_string(),
        },
        CopilotCredential::GitHubToken {
            token: "github-secret".to_string(),
            token_url: "https://api.github.com/copilot_internal/v2/token".to_string(),
        },
    ];

    assert_eq!(
        credentials.map(|credential| format!("{credential:?}")),
        [
            "ApiToken { token: \"[REDACTED]\", api_url: \
             \"https://api.githubcopilot.com\" }"
                .to_string(),
            "GitHubToken { token: \"[REDACTED]\", token_url: \
             \"https://api.github.com/copilot_internal/v2/token\" }"
                .to_string(),
        ]
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
    let keyring = load_with(
        |name| match name {
            GH_TOKEN_ENV | GITHUB_TOKEN_ENV => Some("generic-token".to_string()),
            _ => None,
        },
        || Ok(Some("keyring-token".to_string())),
    );
    let generic = load_with(
        |name| match name {
            GH_TOKEN_ENV => Some("gh-token".to_string()),
            GITHUB_TOKEN_ENV => Some("github-token".to_string()),
            _ => None,
        },
        || Ok(None),
    );

    assert_eq!(
        api,
        Ok(Some(CopilotCredential::ApiToken {
            token: "api-token".to_string(),
            api_url: "https://api.example.test".to_string(),
        }))
    );
    assert_eq!(
        dedicated,
        Ok(Some(CopilotCredential::GitHubToken {
            token: "dedicated-token".to_string(),
            token_url: COPILOT_TOKEN_URL.to_string(),
        }))
    );
    assert_eq!(
        keyring,
        Ok(Some(CopilotCredential::GitHubToken {
            token: "keyring-token".to_string(),
            token_url: COPILOT_TOKEN_URL.to_string(),
        }))
    );
    assert_eq!(
        generic,
        Ok(Some(CopilotCredential::GitHubToken {
            token: "gh-token".to_string(),
            token_url: COPILOT_TOKEN_URL.to_string(),
        }))
    );
}

#[test]
fn missing_credential_and_store_failure_are_distinct() {
    let absent = load_with(|_| None, || Ok(None));
    let store_error = load_with(
        |_| None,
        || {
            Err(CredentialLoadError::credential_store(
                "credential store unavailable".to_string(),
            ))
        },
    );

    assert_eq!(absent, Ok(None));
    assert_eq!(
        store_error,
        Err(CredentialLoadError::credential_store(
            "credential store unavailable".to_string()
        ))
    );
}
