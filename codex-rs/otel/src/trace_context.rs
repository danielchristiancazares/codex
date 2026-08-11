use codex_protocol::protocol::W3cTraceContext;
use std::collections::BTreeMap;
use std::env;
use std::sync::OnceLock;
use tracing::Span;

const TRACEPARENT_ENV_VAR: &str = "TRACEPARENT";
const TRACESTATE_ENV_VAR: &str = "TRACESTATE";
static TRACEPARENT_CONTEXT: OnceLock<Option<TraceContext>> = OnceLock::new();

/// Parsed trace metadata retained for API compatibility while tracing export is disabled.
#[derive(Clone, Debug)]
pub struct TraceContext {
    trace: W3cTraceContext,
}

pub fn current_span_w3c_trace_context() -> Option<W3cTraceContext> {
    None
}

pub fn span_w3c_trace_context(_span: &Span) -> Option<W3cTraceContext> {
    None
}

pub fn inject_span_w3c_trace_headers(_span: &Span, _headers: &mut http::HeaderMap) -> bool {
    false
}

pub fn current_span_trace_id() -> Option<String> {
    None
}

pub fn context_from_w3c_trace_context(trace: &W3cTraceContext) -> Option<TraceContext> {
    valid_traceparent(trace.traceparent.as_deref()?).then(|| TraceContext {
        trace: trace.clone(),
    })
}

pub fn set_parent_from_w3c_trace_context(_span: &Span, trace: &W3cTraceContext) -> bool {
    context_from_w3c_trace_context(trace).is_some()
}

pub fn set_parent_from_context(_span: &Span, context: TraceContext) {
    let _ = context.trace;
}

pub fn traceparent_context_from_env() -> Option<TraceContext> {
    TRACEPARENT_CONTEXT
        .get_or_init(|| {
            let traceparent = env::var(TRACEPARENT_ENV_VAR).ok()?;
            let tracestate = env::var(TRACESTATE_ENV_VAR).ok();
            context_from_w3c_trace_context(&W3cTraceContext {
                traceparent: Some(traceparent),
                tracestate,
            })
        })
        .clone()
}

pub fn validate_tracestate_entries(
    entries: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for (member_key, fields) in entries {
        validate_tracestate_member(member_key, fields)?;
    }
    Ok(())
}

pub fn validate_tracestate_member(
    member_key: &str,
    fields: &BTreeMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !valid_member_key(member_key) {
        return Err(invalid_tracestate_config(format!(
            "invalid configured tracestate member key {member_key}"
        )));
    }
    let mut encoded_len = 0;
    for (field_key, value) in fields {
        if !valid_field_component(field_key) {
            return Err(invalid_tracestate_config(format!(
                "invalid configured tracestate field key {member_key}.{field_key}"
            )));
        }
        if !valid_field_component(value) {
            return Err(invalid_tracestate_config(format!(
                "invalid configured tracestate value for {member_key}.{field_key}"
            )));
        }
        encoded_len += field_key.len() + value.len() + 2;
    }
    if encoded_len > 256 {
        return Err(invalid_tracestate_config(format!(
            "configured tracestate member {member_key} is too long"
        )));
    }
    Ok(())
}

fn valid_traceparent(value: &str) -> bool {
    let mut parts = value.split('-');
    let Some(version) = parts.next() else {
        return false;
    };
    let Some(trace_id) = parts.next() else {
        return false;
    };
    let Some(parent_id) = parts.next() else {
        return false;
    };
    let Some(flags) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && version.len() == 2
        && version != "ff"
        && valid_hex(trace_id, 32)
        && trace_id.bytes().any(|byte| byte != b'0')
        && valid_hex(parent_id, 16)
        && parent_id.bytes().any(|byte| byte != b'0')
        && valid_hex(flags, 2)
}

fn valid_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_member_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'*' | b'/' | b'@')
        })
}

fn valid_field_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'!'..=b'~') && !matches!(byte, b':' | b';' | b',' | b'='))
}

fn invalid_tracestate_config(message: String) -> Box<dyn std::error::Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into()
}
