use std::borrow::Cow;
use std::ops::Deref;

use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::Schema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use ts_rs::TS;
use ts_rs::TypeVisitor;

/// State of a wire field that may be omitted, set to `null`, or assigned a value.
///
/// Containers should use [`NullableField::is_omitted`] with
/// `serde(skip_serializing_if)` so [`NullableField::Omitted`] stays distinct from
/// [`NullableField::Null`] on the wire.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NullableField<T> {
    /// Leave the field absent from the serialized container.
    #[default]
    Omitted,
    /// Serialize the field as `null`.
    Null,
    /// Serialize the supplied field value.
    Value(T),
}

impl<T> NullableField<T> {
    /// Returns whether the containing wire field should be omitted.
    pub const fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }

    /// Returns whether the containing wire field is explicitly present.
    pub const fn is_present(&self) -> bool {
        !self.is_omitted()
    }

    /// Borrows a nullable field without changing its state.
    pub const fn as_ref(&self) -> NullableField<&T> {
        match self {
            Self::Omitted => NullableField::Omitted,
            Self::Null => NullableField::Null,
            Self::Value(value) => NullableField::Value(value),
        }
    }

    /// Maps a present value while preserving omitted and null states.
    pub fn map<U>(self, map_value: impl FnOnce(T) -> U) -> NullableField<U> {
        match self {
            Self::Omitted => NullableField::Omitted,
            Self::Null => NullableField::Null,
            Self::Value(value) => NullableField::Value(map_value(value)),
        }
    }
}

impl<T> NullableField<T>
where
    T: Deref,
{
    /// Borrows through a dereferenceable value without changing field state.
    pub fn as_deref(&self) -> NullableField<&T::Target> {
        self.as_ref().map(Deref::deref)
    }
}

impl<T> Serialize for NullableField<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Omitted | Self::Null => serializer.serialize_none(),
            Self::Value(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T> Deserialize<'de> for NullableField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

impl<T> JsonSchema for NullableField<T>
where
    T: JsonSchema,
{
    fn is_referenceable() -> bool {
        <Option<T> as JsonSchema>::is_referenceable()
    }

    fn schema_name() -> String {
        <Option<T> as JsonSchema>::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        <Option<T> as JsonSchema>::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        <Option<T> as JsonSchema>::json_schema(generator)
    }

    fn _schemars_private_non_optional_json_schema(generator: &mut SchemaGenerator) -> Schema {
        <Option<T> as JsonSchema>::_schemars_private_non_optional_json_schema(generator)
    }

    fn _schemars_private_is_option() -> bool {
        true
    }
}

impl<T> TS for NullableField<T>
where
    T: TS,
{
    type WithoutGenerics = NullableField<ts_rs::Dummy>;
    type OptionInnerType = T;

    const IS_OPTION: bool = true;

    fn name() -> String {
        <Option<T> as TS>::name()
    }

    fn inline() -> String {
        <Option<T> as TS>::inline()
    }

    fn visit_dependencies(visitor: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <Option<T> as TS>::visit_dependencies(visitor);
    }

    fn visit_generics(visitor: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <Option<T> as TS>::visit_generics(visitor);
    }

    fn decl() -> String {
        <Option<T> as TS>::decl()
    }

    fn decl_concrete() -> String {
        <Option<T> as TS>::decl_concrete()
    }

    fn inline_flattened() -> String {
        <Option<T> as TS>::inline_flattened()
    }
}

impl<T> ts_rs::IsOption for NullableField<T> {}

#[cfg(test)]
#[path = "nullable_field_tests.rs"]
mod tests;
