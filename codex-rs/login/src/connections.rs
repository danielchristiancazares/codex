//! Saved credential scopes. Each login keeps its provider's existing storage format.

mod files;

use std::fmt;
use std::path::Path;
use std::path::PathBuf;

use codex_config::types::AuthCredentialsStoreMode;
use serde::Deserialize;
use serde::Serialize;

use crate::AuthKeyringBackendKind;
use crate::GitHubCopilotAuth;
use crate::load_auth_dot_json;

pub(crate) use files::RefreshLease;

const MAX_CONNECTIONS: usize = 64;

/// Explicit persistent login selects the configured store. Refresh persistence
/// uses its existing backend directly and never changes this startup choice.
pub(crate) fn configured_login_saved(
    home: &Path,
    mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    match mode {
        AuthCredentialsStoreMode::Ephemeral => Ok(()),
        AuthCredentialsStoreMode::File
        | AuthCredentialsStoreMode::Keyring
        | AuthCredentialsStoreMode::Auto => {
            files::remove_registration(&home.join("connections").join("selected.json"))
        }
    }
}

/// A bounded, printable name or identity validated at the storage boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmptyString(String);

impl NonEmptyString {
    pub fn new(value: impl Into<String>) -> std::io::Result<Self> {
        Self::try_from(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = std::io::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err(std::io::Error::other(
                "Connection text must contain 1–512 printable bytes.",
            ));
        }
        Ok(Self(value))
    }
}

impl From<NonEmptyString> for String {
    fn from(value: NonEmptyString) -> Self {
        value.0
    }
}

/// The provider owns the credential format and login ceremony for a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionProvider {
    Openai,
    Copilot,
}

impl ConnectionProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Copilot => "copilot",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "camelCase")]
enum Identity {
    Openai {
        user: NonEmptyString,
        workspace: NonEmptyString,
    },
    Copilot {
        machine: NonEmptyString,
    },
}

impl Identity {
    fn provider(&self) -> ConnectionProvider {
        match self {
            Self::Openai { .. } => ConnectionProvider::Openai,
            Self::Copilot { .. } => ConnectionProvider::Copilot,
        }
    }
}

/// A validated saved login. Credential material is never part of its metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedConnection {
    id: ConnectionId,
    name: NonEmptyString,
    identity: Identity,
}

impl SavedConnection {
    pub fn id(&self) -> &str {
        self.id.0.as_str()
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn provider(&self) -> ConnectionProvider {
        self.identity.provider()
    }

    pub fn credential_home(&self, codex_home: &Path) -> PathBuf {
        match self.id() {
            "openai" | "copilot" => codex_home.to_path_buf(),
            id => codex_home.join("connections").join(id),
        }
    }
}

/// Owns registration and lookup while delegating secrets to provider storage.
#[derive(Clone)]
pub struct ConnectionStore {
    home: PathBuf,
    mode: AuthCredentialsStoreMode,
    keyring: AuthKeyringBackendKind,
    route: crate::AuthRouteConfig,
}

impl fmt::Debug for ConnectionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionStore")
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}

impl ConnectionStore {
    pub fn new(
        home: PathBuf,
        mode: AuthCredentialsStoreMode,
        keyring: AuthKeyringBackendKind,
        route: crate::AuthRouteConfig,
    ) -> Self {
        Self {
            home,
            mode,
            keyring,
            route,
        }
    }

    pub fn list(&self) -> std::io::Result<Vec<SavedConnection>> {
        let mut connections = Vec::new();
        if let Some(auth) = load_auth_dot_json(&self.home, self.mode, self.keyring)?
            && let Some(tokens) = auth.tokens
            && !tokens.refresh_token.is_empty()
        {
            connections.push(self.describe_openai("openai", tokens)?);
        }
        if let Some(credential) = self
            .copilot(&self.home)
            .credential()
            .map_err(std::io::Error::other)?
        {
            connections.push(SavedConnection {
                id: ConnectionId::try_from("copilot".to_owned())?,
                name: NonEmptyString::new("GitHub Copilot")?,
                identity: Identity::Copilot {
                    machine: NonEmptyString::new(credential.machine_id())?,
                },
            });
        }
        match std::fs::read_dir(self.home.join("connections")) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    if entry.file_type()?.is_dir()
                        && entry.file_name().to_string_lossy().starts_with("saved-")
                    {
                        match files::read::<SavedConnection>(&entry.path().join("connection.json"))
                        {
                            Ok(connection) => {
                                if connection.credential_home(&self.home) != entry.path() {
                                    return Err(std::io::Error::other(
                                        "Connection metadata does not match its credential scope.",
                                    ));
                                }
                                connections.push(connection);
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error),
                        }
                    }
                    if connections.len() > MAX_CONNECTIONS {
                        return Err(std::io::Error::other(
                            "Too many saved connections (maximum 64).",
                        ));
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        connections.sort_by(|a, b| a.name().cmp(b.name()).then_with(|| a.id().cmp(b.id())));
        Ok(connections)
    }

    pub fn resolve(&self, id: &str) -> std::io::Result<SavedConnection> {
        self.list()?
            .into_iter()
            .find(|connection| connection.id() == id)
            .ok_or_else(|| std::io::Error::other("The selected connection is no longer available."))
    }

    pub fn begin_login(
        &self,
        provider: ConnectionProvider,
        name: NonEmptyString,
    ) -> std::io::Result<ConnectionLogin> {
        if matches!(self.mode, AuthCredentialsStoreMode::Ephemeral) {
            return Err(std::io::Error::other(
                "Saving accounts requires a persistent credential store.",
            ));
        }
        let name = NonEmptyString::new(name.as_str().trim())?;
        if name.as_str().eq_ignore_ascii_case("add") {
            return Err(std::io::Error::other(
                "The name `add` is reserved for /switch add.",
            ));
        }
        let existing = self.list()?;
        if existing.len() >= MAX_CONNECTIONS
            || existing
                .iter()
                .any(|entry| entry.name().eq_ignore_ascii_case(name.as_str()))
        {
            return Err(std::io::Error::other(
                "Choose a unique connection name (maximum 64 connections).",
            ));
        }
        let id = ConnectionId::try_from(format!("saved-{:032x}", rand::random::<u128>()))?;
        let home = self.home.join("connections").join(id.0.as_str());
        std::fs::create_dir_all(&home)?;
        Ok(ConnectionLogin {
            store: self.clone(),
            id,
            name,
            home,
            provider,
        })
    }

    pub fn remember(&self, connection: &SavedConnection) -> std::io::Result<()> {
        crate::credential_file::write_json(
            &self.home.join("connections").join("selected.json"),
            connection,
        )
    }

    /// Returns the selected saved credential location when it belongs to `provider`.
    /// Explicit provider commands otherwise operate on their configured root destination.
    pub fn credential_home_for_provider(
        &self,
        provider: ConnectionProvider,
    ) -> std::io::Result<PathBuf> {
        Ok(match self.startup()? {
            StartupConnection::UseSaved(connection) if connection.provider() == provider => {
                connection.credential_home(&self.home)
            }
            StartupConnection::UseConfigured | StartupConnection::UseSaved(_) => self.home.clone(),
        })
    }

    /// Removes registration for a credential location after its provider logs out.
    pub fn forget_logged_out_scope(&self, scope: &Path) -> std::io::Result<()> {
        let selected_path = self.home.join("connections").join("selected.json");
        match files::read::<SavedConnection>(&selected_path) {
            Ok(selected) if selected.credential_home(&self.home) == scope => {
                files::remove_registration(&selected_path)?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if scope != self.home {
            files::remove_registration(&scope.join("connection.json"))?;
        }
        Ok(())
    }

    pub(crate) fn startup(&self) -> std::io::Result<StartupConnection> {
        if matches!(self.mode, AuthCredentialsStoreMode::Ephemeral) {
            return Ok(StartupConnection::UseConfigured);
        }
        match files::read::<SavedConnection>(&self.home.join("connections").join("selected.json")) {
            Ok(connection) => Ok(StartupConnection::UseSaved(self.resolve(connection.id())?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(StartupConnection::UseConfigured)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn validate(&self, connection: &SavedConnection) -> std::io::Result<()> {
        let home = connection.credential_home(&self.home);
        let identity = match connection.provider() {
            ConnectionProvider::Openai => {
                self.describe_openai(
                    connection.id(),
                    load_auth_dot_json(&home, self.mode, self.keyring)?
                        .ok_or_else(|| {
                            std::io::Error::other("Sign in to this OpenAI account again.")
                        })?
                        .tokens
                        .ok_or_else(|| {
                            std::io::Error::other("This connection requires a ChatGPT login.")
                        })?,
                )?
                .identity
            }
            ConnectionProvider::Copilot => Identity::Copilot {
                machine: NonEmptyString::new(
                    self.copilot(&home)
                        .credential()
                        .map_err(std::io::Error::other)?
                        .ok_or_else(|| std::io::Error::other("Sign in to GitHub Copilot again."))?
                        .machine_id(),
                )?,
            },
        };
        if identity != connection.identity {
            return Err(std::io::Error::other(
                "The saved login's identity changed. Add the account again.",
            ));
        }
        Ok(())
    }

    fn describe_openai(
        &self,
        id: &str,
        tokens: crate::TokenData,
    ) -> std::io::Result<SavedConnection> {
        if tokens.access_token.trim().is_empty() || tokens.refresh_token.trim().is_empty() {
            return Err(std::io::Error::other(
                "A saved ChatGPT login requires access and refresh tokens.",
            ));
        }
        let user = NonEmptyString::new(tokens.id_token.chatgpt_user_id.ok_or_else(|| {
            std::io::Error::other("ChatGPT login is missing its user identity.")
        })?)?;
        let workspace = NonEmptyString::new(
            tokens
                .account_id
                .or(tokens.id_token.chatgpt_account_id)
                .ok_or_else(|| {
                    std::io::Error::other("ChatGPT login is missing its workspace identity.")
                })?,
        )?;
        let name = NonEmptyString::new(
            tokens
                .id_token
                .email
                .unwrap_or_else(|| user.as_str().to_owned()),
        )?;
        Ok(SavedConnection {
            id: ConnectionId::try_from(id.to_owned())?,
            name,
            identity: Identity::Openai { user, workspace },
        })
    }

    fn copilot(&self, home: &Path) -> GitHubCopilotAuth {
        GitHubCopilotAuth::new_in(home, self.route.http_client_factory().clone(), self.mode)
    }
}

pub(crate) enum StartupConnection {
    UseConfigured,
    UseSaved(SavedConnection),
}

/// An isolated login destination, consumed only after provider authentication completes.
#[derive(Debug)]
pub struct ConnectionLogin {
    store: ConnectionStore,
    id: ConnectionId,
    name: NonEmptyString,
    home: PathBuf,
    provider: ConnectionProvider,
}

impl ConnectionLogin {
    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn finish(self) -> std::io::Result<SavedConnection> {
        let registration = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.store.home.join("connections").join(".registry.lock"))?;
        registration.lock()?;
        let existing = self.store.list()?;
        if existing.len() >= MAX_CONNECTIONS
            || existing
                .iter()
                .any(|entry| entry.name().eq_ignore_ascii_case(self.name.as_str()))
        {
            return Err(std::io::Error::other(
                "This account name was registered by another login. Choose a unique name.",
            ));
        }
        let mut connection = match self.provider {
            ConnectionProvider::Openai => self.store.describe_openai(
                self.id.0.as_str(),
                load_auth_dot_json(&self.home, self.store.mode, self.store.keyring)?
                    .ok_or_else(|| std::io::Error::other("Login did not save credentials."))?
                    .tokens
                    .ok_or_else(|| std::io::Error::other("Login did not save ChatGPT tokens."))?,
            )?,
            ConnectionProvider::Copilot => SavedConnection {
                id: self.id.clone(),
                name: self.name.clone(),
                identity: Identity::Copilot {
                    machine: NonEmptyString::new(
                        self.store
                            .copilot(&self.home)
                            .credential()
                            .map_err(std::io::Error::other)?
                            .ok_or_else(|| {
                                std::io::Error::other("Login did not save GitHub credentials.")
                            })?
                            .machine_id(),
                    )?,
                },
            },
        };
        connection.name = self.name;
        crate::credential_file::write_json(&self.home.join("connection.json"), &connection)?;
        Ok(connection)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct ConnectionId(NonEmptyString);

impl TryFrom<String> for ConnectionId {
    type Error = std::io::Error;
    fn try_from(id: String) -> Result<Self, Self::Error> {
        if id == "openai"
            || id == "copilot"
            || (id.len() == 38
                && id.starts_with("saved-")
                && id[6..].bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            Ok(Self(NonEmptyString::new(id)?))
        } else {
            Err(std::io::Error::other(
                "Invalid saved connection identifier.",
            ))
        }
    }
}

impl From<ConnectionId> for String {
    fn from(id: ConnectionId) -> Self {
        id.0.into()
    }
}

#[cfg(test)]
#[path = "connections_tests.rs"]
mod tests;
