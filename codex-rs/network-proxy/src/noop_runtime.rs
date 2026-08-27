use super::*;

#[derive(Clone)]
pub struct NetworkProxyState {
    state: Arc<RwLock<ConfigState>>,
    audit_metadata: NetworkProxyAuditMetadata,
    environment_id: Option<String>,
    execution_id: Option<String>,
}

impl std::fmt::Debug for NetworkProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkProxyState").finish_non_exhaustive()
    }
}

impl NetworkProxyState {
    pub fn from_remote_launch_config(launch: RemoteNetworkProxyLaunchConfig) -> Result<Self> {
        let RemoteNetworkProxyLaunchConfig {
            proxy,
            audit_metadata,
            environment_id,
            execution_id,
            policy_decision_timeout_ms: _,
        } = launch;
        Ok(Self {
            state: Arc::new(RwLock::new(build_config_state(
                proxy.into_network_proxy_config(),
                NetworkProxyConstraints::default(),
            )?)),
            audit_metadata,
            environment_id,
            execution_id,
        })
    }

    pub fn with_reloader(state: ConfigState, reloader: Arc<dyn ConfigReloader>) -> Self {
        Self::with_reloader_and_audit_metadata(
            state,
            reloader,
            NetworkProxyAuditMetadata::default(),
        )
    }

    pub fn with_reloader_and_audit_metadata(
        state: ConfigState,
        _reloader: Arc<dyn ConfigReloader>,
        audit_metadata: NetworkProxyAuditMetadata,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
            audit_metadata,
            environment_id: None,
            execution_id: None,
        }
    }

    pub fn with_reloader_and_blocked_observer(
        state: ConfigState,
        reloader: Arc<dyn ConfigReloader>,
        _blocked_request_observer: Option<Arc<dyn BlockedRequestObserver>>,
    ) -> Self {
        Self::with_reloader(state, reloader)
    }

    pub async fn set_blocked_request_observer(
        &self,
        _blocked_request_observer: Option<Arc<dyn BlockedRequestObserver>>,
    ) {
    }

    pub fn set_policy_audit_observer(&mut self, _observer: NetworkPolicyAuditObserver) {}

    pub fn audit_metadata(&self) -> &NetworkProxyAuditMetadata {
        &self.audit_metadata
    }

    pub fn environment_id(&self) -> Option<&str> {
        self.environment_id.as_deref()
    }

    pub fn execution_id(&self) -> Option<String> {
        self.execution_id.clone()
    }

    pub async fn current_cfg(&self) -> Result<NetworkProxyConfig> {
        Ok(self
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .config
            .clone())
    }

    pub async fn replace_config_state(&self, state: ConfigState) -> Result<()> {
        *self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = state;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedNetworkSandboxContext {
    pub loopback_ports: Vec<u16>,
    pub allow_local_binding: bool,
}

pub struct PreparedManagedNetwork {
    pub env: HashMap<String, String>,
    pub sandbox_context: ManagedNetworkSandboxContext,
}

#[derive(Clone)]
pub struct NetworkProxy {
    state: Arc<NetworkProxyState>,
    http_addr: SocketAddr,
    socks_addr: SocketAddr,
}

impl std::fmt::Debug for NetworkProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkProxy").finish_non_exhaustive()
    }
}

impl PartialEq for NetworkProxy {
    fn eq(&self, other: &Self) -> bool {
        self.http_addr == other.http_addr && self.socks_addr == other.socks_addr
    }
}

impl Eq for NetworkProxy {}

#[derive(Clone, Default)]
pub struct NetworkProxyBuilder {
    state: Option<Arc<NetworkProxyState>>,
    http_addr: Option<SocketAddr>,
    socks_addr: Option<SocketAddr>,
}

impl NetworkProxyBuilder {
    pub fn state(mut self, state: Arc<NetworkProxyState>) -> Self {
        self.state = Some(state);
        self
    }

    pub fn http_addr(mut self, addr: SocketAddr) -> Self {
        self.http_addr = Some(addr);
        self
    }

    pub fn socks_addr(mut self, addr: SocketAddr) -> Self {
        self.socks_addr = Some(addr);
        self
    }

    pub fn managed_by_codex(self, _managed_by_codex: bool) -> Self {
        self
    }

    pub fn policy_decider<D: NetworkPolicyDecider>(self, _decider: D) -> Self {
        self
    }

    pub fn policy_decider_arc(self, _decider: Arc<dyn NetworkPolicyDecider>) -> Self {
        self
    }

    pub fn blocked_request_observer<O: BlockedRequestObserver>(self, _observer: O) -> Self {
        self
    }

    pub fn blocked_request_observer_arc(self, _observer: Arc<dyn BlockedRequestObserver>) -> Self {
        self
    }

    pub async fn build(self) -> Result<NetworkProxy> {
        let state = self
            .state
            .ok_or_else(|| anyhow::anyhow!("NetworkProxyBuilder requires a state"))?;
        if state.current_cfg().await?.enabled {
            anyhow::bail!(DISABLED_MESSAGE);
        }
        Ok(NetworkProxy {
            state,
            http_addr: self
                .http_addr
                .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0))),
            socks_addr: self
                .socks_addr
                .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0))),
        })
    }
}

impl NetworkProxy {
    pub fn builder() -> NetworkProxyBuilder {
        NetworkProxyBuilder::default()
    }

    pub fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    pub fn socks_addr(&self) -> SocketAddr {
        self.socks_addr
    }

    #[cfg(target_os = "windows")]
    pub fn network_proxy_restricting_sid(&self, _environment_id: Option<&str>) -> Option<String> {
        None
    }

    pub async fn current_cfg(&self) -> Result<NetworkProxyConfig> {
        self.state.current_cfg().await
    }

    pub async fn remote_launch_config(&self) -> Result<RemoteNetworkProxyLaunchConfig> {
        Ok(RemoteNetworkProxyLaunchConfig::new(
            RemoteNetworkProxyConfig::from_effective_config(&self.current_cfg().await?)?,
        ))
    }

    pub fn for_execution(
        &self,
        _environment_id: &str,
        _execution_id: &str,
        _attribution_token: String,
    ) -> Result<Self> {
        Ok(self.clone())
    }

    pub fn remote_policy_decider(&self) -> Option<Arc<dyn NetworkPolicyDecider>> {
        None
    }

    pub async fn add_allowed_domain(&self, _host: &str) -> Result<()> {
        Ok(())
    }

    pub async fn add_denied_domain(&self, _host: &str) -> Result<()> {
        Ok(())
    }

    pub fn allow_local_binding(&self) -> bool {
        false
    }

    pub fn allow_unix_sockets(&self) -> Arc<[String]> {
        Arc::from([])
    }

    pub fn dangerously_allow_all_unix_sockets(&self) -> bool {
        false
    }

    pub fn managed_mitm_ca_trust_bundle_path(&self) -> Option<AbsolutePathBuf> {
        None
    }

    pub fn apply_to_env(&self, _env: &mut HashMap<String, String>) {}

    pub fn apply_to_env_for_environment(
        &self,
        _env: &mut HashMap<String, String>,
        _environment_id: &str,
    ) -> Result<()> {
        Ok(())
    }

    pub fn apply_to_env_for_optional_environment(
        &self,
        _env: &mut HashMap<String, String>,
        _environment_id: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn prepare_for_optional_environment(
        &self,
        env: HashMap<String, String>,
        _environment_id: Option<&str>,
    ) -> Result<PreparedManagedNetwork> {
        Ok(PreparedManagedNetwork {
            env,
            sandbox_context: ManagedNetworkSandboxContext {
                loopback_ports: Vec::new(),
                allow_local_binding: false,
            },
        })
    }

    pub fn prepare_for_remote_environment(
        &self,
        env: HashMap<String, String>,
        _environment_id: &str,
    ) -> Result<PreparedManagedNetwork> {
        self.prepare_for_optional_environment(env, None)
    }

    pub async fn replace_config_state(&self, state: ConfigState) -> Result<()> {
        if state.config.enabled {
            anyhow::bail!(DISABLED_MESSAGE);
        }
        self.state.replace_config_state(state).await
    }

    pub async fn run(&self) -> Result<NetworkProxyHandle> {
        Ok(NetworkProxyHandle {})
    }
}

pub struct NetworkProxyHandle {}

impl NetworkProxyHandle {
    pub async fn wait(self) -> Result<()> {
        Ok(())
    }

    pub async fn shutdown(self) -> Result<()> {
        Ok(())
    }
}

pub fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

pub fn is_managed_proxy_env_var(key: &str, _value: &str) -> bool {
    PROXY_ENV_KEYS.contains(&key) || CUSTOM_CA_ENV_KEYS.contains(&key)
}

pub fn strip_managed_proxy_env(env: &mut HashMap<String, String>) {
    env.retain(|key, value| !is_managed_proxy_env_var(key, value));
}

pub fn proxy_url_env_value<'a>(
    env: &'a HashMap<String, String>,
    canonical_key: &str,
) -> Option<&'a str> {
    env.get(canonical_key).map(String::as_str).or_else(|| {
        let lower_key = canonical_key.to_ascii_lowercase();
        env.get(&lower_key).map(String::as_str)
    })
}

pub fn has_proxy_url_env_vars(env: &HashMap<String, String>) -> bool {
    PROXY_URL_ENV_KEYS
        .iter()
        .any(|key| proxy_url_env_value(env, key).is_some_and(|value| !value.trim().is_empty()))
}

pub fn brokered_credential_dummy_env_keys(_env: &HashMap<String, String>) -> Vec<String> {
    Vec::new()
}

pub fn brokered_credential_env_keys(_env: &HashMap<String, String>) -> Vec<String> {
    Vec::new()
}

pub fn is_managed_mitm_ca_trust_bundle_path(_path: &str) -> bool {
    false
}

pub fn write_attribution_frame(
    _writer: &mut impl std::io::Write,
    _token: &str,
) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "noop_tests.rs"]
mod tests;
