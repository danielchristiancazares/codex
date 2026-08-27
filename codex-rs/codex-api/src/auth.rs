use codex_client::Request;
use codex_client::TransportError;
use http::HeaderMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Error returned while applying authentication to an outbound request.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("request auth build error: {0}")]
    Build(String),
    #[error("transient auth error: {0}")]
    Transient(String),
}

impl From<AuthError> for TransportError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Build(message) => TransportError::Build(message),
            AuthError::Transient(message) => TransportError::Network(message),
        }
    }
}

/// Applies authentication to API requests.
///
/// Header-only providers can implement `add_auth_headers`; providers that sign
/// complete requests can override `apply_auth`.
pub trait AuthProvider: Send + Sync {
    /// Adds any auth headers that are available without request body access.
    ///
    /// Implementations should be cheap and non-blocking. This method is also
    /// used by telemetry and non-HTTP request paths.
    fn add_auth_headers(&self, headers: &mut HeaderMap);

    /// Applies provider-specific shaping to a serialized Responses WebSocket request.
    ///
    /// Providers that override [`AuthProvider::apply_auth`] to adapt HTTP Responses request
    /// bodies should override this hook as well so both transports send equivalent payloads.
    fn prepare_responses_websocket_request(&self, request: String) -> Result<String, AuthError> {
        Ok(request)
    }

    /// Observes a 401/403 rejection from a Responses WebSocket upgrade or error frame.
    ///
    /// Credential providers can use this to reject the exact credential generation attached to
    /// the connection before the caller reconnects.
    fn on_responses_websocket_auth_rejected(&self) {}

    /// Identifies the credential snapshot used by a Responses WebSocket connection.
    ///
    /// When this value changes, callers must establish a new connection before sending another
    /// request. Providers with connection-bound or model-bound credentials should return a stable,
    /// non-secret identifier for the lifetime of that credential snapshot.
    fn responses_websocket_connection_key(&self) -> Option<String> {
        None
    }

    /// Returns any auth headers that are available without request body access.
    fn to_auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        self.add_auth_headers(&mut headers);
        headers
    }

    /// Resolves auth headers for an outbound request.
    ///
    /// Unlike [`Self::to_auth_headers`], implementations may perform asynchronous work to refresh
    /// credentials before returning. Header-only providers with static credentials can rely on the
    /// default implementation.
    fn resolve_auth_headers(&self) -> AuthHeadersFuture<'_> {
        Box::pin(async { Ok(self.to_auth_headers()) })
    }

    /// Applies auth to a complete outbound request and returns the request to send.
    ///
    /// The input `request` is moved into this method. Implementations may mutate
    /// the owned request, or replace it entirely, before returning.
    ///
    /// Header-only auth providers can rely on the default implementation.
    /// Request-signing providers can override this to inspect the final URL,
    /// headers, and body bytes before the transport sends the request.
    ///
    /// Callers must always use the returned request as authoritative.
    /// If this returns [`AuthError`], the request should not be sent.
    fn apply_auth(&self, request: Request) -> AuthProviderFuture<'_> {
        Box::pin(async move {
            let mut request = request;
            request.headers.extend(self.resolve_auth_headers().await?);
            Ok(request)
        })
    }
}

pub type AuthProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Request, AuthError>> + Send + 'a>>;

pub type AuthHeadersFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HeaderMap, AuthError>> + Send + 'a>>;

/// Shared auth handle passed through API clients.
pub type SharedAuthProvider = Arc<dyn AuthProvider>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentIdentityTelemetry {
    pub agent_id: String,
    pub task_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthHeaderTelemetry {
    pub attached: bool,
    pub name: Option<&'static str>,
}

pub fn auth_header_telemetry(auth: &dyn AuthProvider) -> AuthHeaderTelemetry {
    let mut headers = HeaderMap::new();
    auth.add_auth_headers(&mut headers);
    let name = headers
        .contains_key(http::header::AUTHORIZATION)
        .then_some("authorization");
    AuthHeaderTelemetry {
        attached: name.is_some(),
        name,
    }
}
