use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use super::NullableField;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Payload {
    #[serde(default, skip_serializing_if = "NullableField::is_omitted")]
    field: NullableField<String>,
}

#[test]
fn distinguishes_omitted_null_and_value_fields() {
    assert_eq!(
        [
            serde_json::from_value::<Payload>(json!({})).expect("deserialize omitted field"),
            serde_json::from_value::<Payload>(json!({"field": null}))
                .expect("deserialize null field"),
            serde_json::from_value::<Payload>(json!({"field": "value"}))
                .expect("deserialize value field"),
        ],
        [
            Payload {
                field: NullableField::Omitted,
            },
            Payload {
                field: NullableField::Null,
            },
            Payload {
                field: NullableField::Value("value".to_string()),
            },
        ]
    );
}

#[test]
fn preserves_the_three_wire_shapes_when_serializing() {
    assert_eq!(
        [
            serde_json::to_value(Payload {
                field: NullableField::Omitted,
            })
            .expect("serialize omitted field"),
            serde_json::to_value(Payload {
                field: NullableField::Null,
            })
            .expect("serialize null field"),
            serde_json::to_value(Payload {
                field: NullableField::Value("value".to_string()),
            })
            .expect("serialize value field"),
        ],
        [json!({}), json!({"field": null}), json!({"field": "value"}),]
    );
}

#[test]
fn maps_only_present_values() {
    assert_eq!(
        [
            NullableField::<String>::Omitted.map(|value| value.len()),
            NullableField::<String>::Null.map(|value| value.len()),
            NullableField::Value("value".to_string()).map(|value| value.len()),
        ],
        [
            NullableField::Omitted,
            NullableField::Null,
            NullableField::Value(5),
        ]
    );
}
