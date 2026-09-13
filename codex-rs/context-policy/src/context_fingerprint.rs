//! Fingerprints the rendered publication, preserving the rollout's hexadecimal format.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use sha1::Digest;
use sha1::Sha1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContextFingerprint([u8; 20]);

impl ContextFingerprint {
    pub(super) fn for_rendered(value: &str) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(b"codex-additional-context-v2\0");
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(value.as_bytes());
        Self(hasher.finalize().into())
    }
}

impl Serialize for ContextFingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            &self
                .0
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
    }
}

impl<'de> Deserialize<'de> for ContextFingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() != 40 || !encoded.is_ascii() {
            return Err(serde::de::Error::custom(
                "additional-context fingerprint must contain 40 hexadecimal digits",
            ));
        }
        let mut bytes = [0; 20];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Self(bytes))
    }
}
