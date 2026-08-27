use super::Config;
use super::ConfigLayer;
use super::ConfigLayerSource;
use super::ConfigReadResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn nullable_config_read_response_fields_serialize_as_null() {
    let config: Config =
        serde_json::from_value(json!({})).expect("empty config should deserialize");
    let serialized_config =
        serde_json::to_value(&config).expect("config should serialize for expected response");

    assert_eq!(
        serde_json::to_value(ConfigReadResponse {
            config,
            origins: HashMap::new(),
            layers: None,
        })
        .expect("config/read response should serialize"),
        json!({
            "config": serialized_config,
            "origins": {},
            "layers": null,
        }),
    );

    assert_eq!(
        serde_json::to_value(ConfigLayer {
            name: ConfigLayerSource::SessionFlags,
            version: "test-version".to_string(),
            config: json!({}),
            disabled_reason: None,
        })
        .expect("config layer should serialize"),
        json!({
            "name": {
                "type": "sessionFlags",
            },
            "version": "test-version",
            "config": {},
            "disabledReason": null,
        }),
    );
}
