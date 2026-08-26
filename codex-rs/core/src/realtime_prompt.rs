use codex_prompts::BACKEND_PROMPT;
use codex_protocol::NullableField;
const DEFAULT_USER_FIRST_NAME: &str = "there";
const USER_FIRST_NAME_PLACEHOLDER: &str = "{{ user_first_name }}";

pub(crate) fn prepare_realtime_backend_prompt(
    prompt: NullableField<String>,
    config_prompt: Option<String>,
) -> String {
    if let Some(config_prompt) = config_prompt
        && !config_prompt.trim().is_empty()
    {
        return config_prompt;
    }

    match prompt {
        NullableField::Value(prompt) => return prompt,
        NullableField::Null => return String::new(),
        NullableField::Omitted => {}
    }

    BACKEND_PROMPT
        .trim_end()
        .replace(USER_FIRST_NAME_PLACEHOLDER, &current_user_first_name())
}

fn current_user_first_name() -> String {
    [whoami::realname(), whoami::username()]
        .into_iter()
        .filter_map(|name| name.split_whitespace().next().map(str::to_string))
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_USER_FIRST_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use codex_protocol::NullableField;

    use super::prepare_realtime_backend_prompt;

    #[test]
    fn prepare_realtime_backend_prompt_prefers_config_override() {
        assert_eq!(
            prepare_realtime_backend_prompt(
                NullableField::Value("prompt from request".to_string()),
                Some("prompt from config".to_string()),
            ),
            "prompt from config"
        );
    }

    #[test]
    fn prepare_realtime_backend_prompt_uses_request_prompt() {
        assert_eq!(
            prepare_realtime_backend_prompt(
                NullableField::Value("prompt from request".to_string()),
                /*config_prompt*/ None,
            ),
            "prompt from request"
        );
    }

    #[test]
    fn prepare_realtime_backend_prompt_preserves_empty_request_prompt() {
        assert_eq!(
            prepare_realtime_backend_prompt(
                NullableField::Value(String::new()),
                /*config_prompt*/ None,
            ),
            ""
        );
        assert_eq!(
            prepare_realtime_backend_prompt(NullableField::Null, /*config_prompt*/ None),
            ""
        );
    }

    #[test]
    fn prepare_realtime_backend_prompt_renders_default() {
        let prompt = prepare_realtime_backend_prompt(
            /*prompt*/ NullableField::Omitted,
            /*config_prompt*/ None,
        );

        assert!(prompt.starts_with("## Identity, tone, and role"));
        assert!(prompt.contains("You are Codex, an OpenAI general-purpose agentic assistant"));
        assert!(prompt.contains("The user's name is "));
        assert!(!prompt.contains("{{ user_first_name }}"));
    }
}
