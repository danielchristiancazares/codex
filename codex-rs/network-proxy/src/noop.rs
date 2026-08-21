//! Disabled implementation of the managed sandbox network proxy.
//!
//! The public contract remains available to the rest of the workspace, while attempts to start an
//! enabled proxy fail closed. This keeps direct networking, Remote, and MCP behavior independent
//! from the optional managed proxy policy engine.

use crate::NetworkProxyConfig;
use crate::RemoteNetworkProxyConfig;
use crate::RemoteNetworkProxyLaunchConfig;
use anyhow::Result;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

const DISABLED_MESSAGE: &str = "managed network proxy is disabled in this personal build";

pub const PROXY_ATTRIBUTION_TOKEN_ENV_KEY: &str = "CODEX_NETWORK_PROXY_ATTRIBUTION_TOKEN";
pub const CREDENTIAL_BROKER_ACTIVE_ENV_KEY: &str = "CODEX_NETWORK_CREDENTIAL_BROKER_ACTIVE";
pub const CUSTOM_CA_ENV_KEYS: [&str; 5] = [
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
];
pub const PROXY_URL_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "WS_PROXY",
    "WSS_PROXY",
    "ALL_PROXY",
    "FTP_PROXY",
];
pub const ALL_PROXY_ENV_KEYS: &[&str] = &["ALL_PROXY", "all_proxy"];
pub const PROXY_ACTIVE_ENV_KEY: &str = "CODEX_NETWORK_PROXY_ACTIVE";
pub const ALLOW_LOCAL_BINDING_ENV_KEY: &str = "CODEX_NETWORK_ALLOW_LOCAL_BINDING";
pub const NO_PROXY_ENV_KEYS: &[&str] = &["NO_PROXY", "no_proxy"];
pub const DEFAULT_NO_PROXY_VALUE: &str = "localhost,127.0.0.1,::1";
pub const PROXY_ENV_KEYS: &[&str] = &[
    PROXY_ACTIVE_ENV_KEY,
    CREDENTIAL_BROKER_ACTIVE_ENV_KEY,
    ALLOW_LOCAL_BINDING_ENV_KEY,
    PROXY_ATTRIBUTION_TOKEN_ENV_KEY,
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "http_proxy",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];

#[cfg(target_os = "macos")]
pub const PROXY_GIT_SSH_COMMAND_ENV_KEY: &str = "GIT_SSH_COMMAND";
#[cfg(target_os = "macos")]
pub const CODEX_PROXY_GIT_SSH_COMMAND_MARKER: &str = "# codex-managed-network-proxy";

#[derive(Debug, Clone, clap::Parser)]
pub struct Args {}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookConfig {
    pub host: String,
    #[serde(rename = "match", default)]
    pub matcher: MitmHookMatchConfig,
    #[serde(default)]
    pub actions: MitmHookActionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookMatchConfig {
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
    pub query: BTreeMap<String, Vec<String>>,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: Option<MitmHookBodyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct MitmHookActionsConfig {
    pub strip_request_headers: Vec<String>,
    pub inject_request_headers: Vec<InjectedHeaderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct InjectedHeaderConfig {
    pub name: String,
    pub secret_env_var: Option<String>,
    pub secret_file: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct MitmHookBodyConfig(pub serde_json::Value);

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProxyAuditMetadata {
    pub conversation_id: Option<String>,
    pub app_version: Option<String>,
    pub user_account_id: Option<String>,
    pub auth_mode: Option<String>,
    pub originator: Option<String>,
    pub user_email: Option<String>,
    pub terminal_type: Option<String>,
    pub model: Option<String>,
    pub slug: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockedRequest {
    pub host: String,
    pub reason: String,
    pub client: Option<String>,
    pub method: Option<String>,
    pub mode: Option<crate::NetworkMode>,
    pub protocol: String,
    #[serde(skip)]
    pub execution_id: Option<String>,
    pub decision: Option<String>,
    pub source: Option<String>,
    pub port: Option<u16>,
    pub timestamp: i64,
}

pub struct BlockedRequestArgs {
    pub host: String,
    pub reason: String,
    pub client: Option<String>,
    pub method: Option<String>,
    pub mode: Option<crate::NetworkMode>,
    pub protocol: String,
    pub decision: Option<String>,
    pub source: Option<String>,
    pub port: Option<u16>,
}

impl BlockedRequest {
    pub fn new(args: BlockedRequestArgs) -> Self {
        Self {
            host: args.host,
            reason: args.reason,
            client: args.client,
            method: args.method,
            mode: args.mode,
            protocol: args.protocol,
            execution_id: None,
            decision: args.decision,
            source: args.source,
            port: args.port,
            timestamp: 0,
        }
    }
}

pub trait BlockedRequestObserver: Send + Sync + 'static {
    fn on_blocked_request(&self, request: BlockedRequest) -> BlockedRequestObserverFuture<'_>;
}

pub type BlockedRequestObserverFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

impl<F, Fut> BlockedRequestObserver for F
where
    F: Fn(BlockedRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    fn on_blocked_request(&self, request: BlockedRequest) -> BlockedRequestObserverFuture<'_> {
        Box::pin((self)(request))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkProtocol {
    Http,
    HttpsConnect,
    Socks5Tcp,
    Socks5Udp,
}

/// A completed network-policy audit decision without tenant or session identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkPolicyAuditEvent {
    pub timestamp: String,
    pub scope: String,
    pub decision: String,
    pub source: String,
    pub reason: String,
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub method: Option<String>,
    pub client: Option<String>,
    pub policy_override: bool,
}

/// Observes final network-policy decisions without delaying or altering enforcement.
///
/// Implementations must return immediately and treat notification delivery as best effort.
pub type NetworkPolicyAuditObserver = Arc<dyn Fn(NetworkPolicyAuditEvent) + Send + Sync + 'static>;

impl NetworkProtocol {
    pub const fn as_policy_protocol(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::HttpsConnect => "https_connect",
            Self::Socks5Tcp => "socks5_tcp",
            Self::Socks5Udp => "socks5_udp",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPolicyDecision {
    Deny,
    Ask,
}

impl NetworkPolicyDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDecisionSource {
    BaselinePolicy,
    ModeGuard,
    ProxyState,
    Decider,
}

impl NetworkDecisionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselinePolicy => "baseline_policy",
            Self::ModeGuard => "mode_guard",
            Self::ProxyState => "proxy_state",
            Self::Decider => "decider",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkDecision {
    Allow,
    Deny {
        reason: String,
        source: NetworkDecisionSource,
        decision: NetworkPolicyDecision,
    },
}

impl NetworkDecision {
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::deny_with_source(reason, NetworkDecisionSource::BaselinePolicy)
    }

    pub fn ask(reason: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
            source: NetworkDecisionSource::BaselinePolicy,
            decision: NetworkPolicyDecision::Ask,
        }
    }

    pub fn deny_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        Self::Deny {
            reason: reason.into(),
            source,
            decision: NetworkPolicyDecision::Deny,
        }
    }

    pub fn ask_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        Self::Deny {
            reason: reason.into(),
            source,
            decision: NetworkPolicyDecision::Ask,
        }
    }
}

/// Inert transport metadata for builds without the managed network proxy.
#[derive(Clone, Debug, Default)]
pub struct NetworkRequestDisconnect;

impl NetworkRequestDisconnect {
    pub fn elapsed(&self) -> Option<Duration> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct NetworkPolicyRequest {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub environment_id: Option<String>,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
    pub execution_id: Option<String>,
    /// Present only when the local HTTP transport can identify an abandoned request.
    pub disconnect: Option<NetworkRequestDisconnect>,
}

pub struct NetworkPolicyRequestArgs {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub environment_id: Option<String>,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
}

impl NetworkPolicyRequest {
    pub fn new(args: NetworkPolicyRequestArgs) -> Self {
        Self {
            protocol: args.protocol,
            host: args.host,
            port: args.port,
            environment_id: args.environment_id,
            client_addr: args.client_addr,
            method: args.method,
            command: args.command,
            exec_policy_hint: args.exec_policy_hint,
            execution_id: None,
            disconnect: None,
        }
    }
}

pub trait NetworkPolicyDecider: Send + Sync + 'static {
    fn decide(&self, req: NetworkPolicyRequest) -> NetworkPolicyDeciderFuture<'_>;
}

pub type NetworkPolicyDeciderFuture<'a> =
    Pin<Box<dyn Future<Output = NetworkDecision> + Send + 'a>>;

impl<F, Fut> NetworkPolicyDecider for F
where
    F: Fn(NetworkPolicyRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = NetworkDecision> + Send + 'static,
{
    fn decide(&self, req: NetworkPolicyRequest) -> NetworkPolicyDeciderFuture<'_> {
        Box::pin((self)(req))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkProxyConstraints {
    pub enabled: Option<bool>,
    pub mode: Option<crate::NetworkMode>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub allowed_domains: Option<Vec<String>>,
    pub allowlist_expansion_enabled: Option<bool>,
    pub denied_domains: Option<Vec<String>>,
    pub denylist_expansion_enabled: Option<bool>,
    pub allow_unix_sockets: Option<Vec<String>>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartialNetworkProxyConfig {
    pub enabled: Option<bool>,
    pub mode: Option<crate::NetworkMode>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub domains: Option<crate::NetworkDomainPermissions>,
    pub unix_sockets: Option<crate::NetworkUnixSocketPermissions>,
    pub allow_local_binding: Option<bool>,
    pub mitm: Option<bool>,
    pub credential_broker: Option<bool>,
    pub dangerously_allow_plaintext_credential_injection: Option<bool>,
    pub mitm_hooks: Option<Vec<MitmHookConfig>>,
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkProxyConstraintError {
    #[error("invalid network proxy value for {field_name}: {candidate}; allowed: {allowed}")]
    InvalidValue {
        field_name: &'static str,
        candidate: String,
        allowed: String,
    },
    #[error("invalid network proxy configuration: {0}")]
    InvalidConfiguration(String),
}

impl NetworkProxyConstraintError {
    pub fn into_anyhow(self) -> anyhow::Error {
        self.into()
    }
}

#[derive(Clone, Debug)]
pub struct ConfigState {
    pub config: NetworkProxyConfig,
    pub constraints: NetworkProxyConstraints,
}

pub fn build_config_state(
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
) -> Result<ConfigState> {
    validate_policy_against_constraints(&config, &constraints)?;
    Ok(ConfigState {
        config,
        constraints,
    })
}

pub fn validate_policy_against_constraints(
    config: &NetworkProxyConfig,
    constraints: &NetworkProxyConstraints,
) -> std::result::Result<(), NetworkProxyConstraintError> {
    if constraints.enabled == Some(false) && config.enabled {
        return Err(NetworkProxyConstraintError::InvalidValue {
            field_name: "network.enabled",
            candidate: "true".to_string(),
            allowed: "false".to_string(),
        });
    }
    Ok(())
}

pub trait ConfigReloader: Send + Sync {
    fn source_label(&self) -> String;
    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>>;
    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState>;
}

pub type ConfigReloaderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[path = "noop_runtime.rs"]
mod runtime;
pub use runtime::*;
