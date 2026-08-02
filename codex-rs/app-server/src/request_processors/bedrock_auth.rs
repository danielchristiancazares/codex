use super::config_processor::map_error as map_config_error;
use crate::config_manager::ConfigManager;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_model_provider::AMAZON_BEDROCK_PROVIDER_ID;

pub(super) async fn clear_user_model_provider_if_bedrock(
    config_manager: &ConfigManager,
) -> Result<(), JSONRPCErrorError> {
    let result = config_manager
        .clear_user_value_if_matches(
            "model_provider",
            serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
        )
        .await;
    if let Err(err) = &result
        && err.write_error_code() == Some(ConfigWriteErrorCode::ConfigVersionConflict)
    {
        tracing::warn!(
            "configuration changed while clearing the managed Amazon Bedrock model provider; retrying once"
        );
        return config_manager
            .clear_user_value_if_matches(
                "model_provider",
                serde_json::json!(AMAZON_BEDROCK_PROVIDER_ID),
            )
            .await
            .map_err(map_config_error);
    }
    result.map_err(map_config_error)
}
