use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

/// Bounded printable connection metadata, validated when decoding a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(try_from = "String", into = "String")]
#[ts(export_to = "v2/")]
pub struct ConnectionText(String);

impl ConnectionText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConnectionText {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err("Expected bounded, nonempty connection text.");
        }
        Ok(Self(value))
    }
}

impl From<ConnectionText> for String {
    fn from(value: ConnectionText) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum SavedConnectionProvider {
    Openai,
    Copilot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ConnectionSelection {
    Retain,
    Switch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SavedConnectionInfo {
    pub id: ConnectionText,
    pub name: ConnectionText,
    pub provider: SavedConnectionProvider,
    pub selection: ConnectionSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    export_to = "v2/"
)]
pub enum SavedConnectionParams {
    List {
        #[schemars(rename = "activeProvider")]
        active_provider: ConnectionText,
    },
    Add {
        provider: SavedConnectionProvider,
        name: ConnectionText,
    },
    Prepare {
        id: ConnectionText,
    },
    Select {
        id: ConnectionText,
        #[schemars(rename = "switchId")]
        switch_id: ConnectionText,
    },
    Commit {
        #[schemars(rename = "switchId")]
        switch_id: ConnectionText,
    },
    Restore {
        #[schemars(rename = "switchId")]
        switch_id: ConnectionText,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    export_to = "v2/"
)]
pub enum ConnectionLoginChallenge {
    Browser {
        url: ConnectionText,
    },
    DeviceCode {
        url: ConnectionText,
        code: ConnectionText,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    export_to = "v2/"
)]
pub enum SavedConnectionResponse {
    Connections {
        data: Vec<SavedConnectionInfo>,
    },
    Prepared {
        data: Vec<super::Model>,
    },
    Selected {
        #[schemars(rename = "switchId")]
        switch_id: ConnectionText,
    },
    LoginStarted {
        #[schemars(rename = "loginId")]
        login_id: ConnectionText,
        challenge: ConnectionLoginChallenge,
    },
    Settled,
}
