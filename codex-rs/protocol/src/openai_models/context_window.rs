//! Validated catalog capacities; provider-managed windows serialize as JSON null.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::num::NonZeroI64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextWindowCapacity(NonZeroI64);

#[derive(Debug, thiserror::Error)]
#[error("model context windows must be positive signed 64-bit integers")]
pub struct InvalidContextWindow;

impl TryFrom<i64> for ContextWindowCapacity {
    type Error = InvalidContextWindow;
    fn try_from(tokens: i64) -> Result<Self, Self::Error> {
        if tokens < 0 {
            return Err(InvalidContextWindow);
        }
        NonZeroI64::new(tokens)
            .map(Self)
            .ok_or(InvalidContextWindow)
    }
}

impl ContextWindowCapacity {
    pub fn tokens(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelContextWindow(WindowPolicy);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum WindowPolicy {
    #[default]
    ProviderManaged,
    Explicit(ContextWindowCapacity),
}

impl TryFrom<i64> for ModelContextWindow {
    type Error = InvalidContextWindow;
    fn try_from(tokens: i64) -> Result<Self, Self::Error> {
        Ok(Self(WindowPolicy::Explicit(
            ContextWindowCapacity::try_from(tokens)?,
        )))
    }
}

impl ModelContextWindow {
    /// Dispatches only a strictly increasing normal/maximum catalog pair to the picker.
    pub fn select<R>(
        self,
        maximum: Self,
        keep_configured: impl FnOnce() -> R,
        choose: impl FnOnce(ContextWindowCapacity, ContextWindowCapacity) -> R,
    ) -> R {
        match (self.0, maximum.0) {
            (WindowPolicy::Explicit(normal), WindowPolicy::Explicit(maximum)) => {
                if maximum.tokens() > normal.tokens() {
                    choose(normal, maximum)
                } else {
                    keep_configured()
                }
            }
            (
                WindowPolicy::ProviderManaged,
                WindowPolicy::ProviderManaged | WindowPolicy::Explicit(_),
            )
            | (WindowPolicy::Explicit(_), WindowPolicy::ProviderManaged) => keep_configured(),
        }
    }
}

impl Serialize for ModelContextWindow {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            WindowPolicy::ProviderManaged => serializer.serialize_none(),
            WindowPolicy::Explicit(capacity) => serializer.serialize_i64(capacity.tokens()),
        }
    }
}

impl<'de> Deserialize<'de> for ModelContextWindow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Number(number) => Self::try_from(
                number
                    .as_i64()
                    .ok_or_else(|| serde::de::Error::custom(InvalidContextWindow))?,
            )
            .map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom(InvalidContextWindow)),
        }
    }
}

impl JsonSchema for ModelContextWindow {
    fn schema_name() -> String {
        "ModelContextWindow".to_string()
    }
    fn is_referenceable() -> bool {
        false
    }
    fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::InstanceType;
        use schemars::schema::NumberValidation;
        use schemars::schema::Schema;
        use schemars::schema::SchemaObject;
        use schemars::schema::SingleOrVec;
        Schema::Object(SchemaObject {
            instance_type: Some(SingleOrVec::Vec(vec![
                InstanceType::Integer,
                InstanceType::Null,
            ])),
            format: Some("int64".to_string()),
            number: Some(Box::new(NumberValidation {
                minimum: Some(1.0),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}
