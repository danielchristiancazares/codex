//! Recognition of literal PowerShell patch pipelines before shell execution.

/// Recognize a single `@' ... '@ | apply_patch` pipeline without evaluating shell syntax.
pub(super) fn extract_apply_patch(script: &str) -> Option<&str> {
    let (opening, body) = script.trim().split_once('\n')?;
    if opening.trim_end_matches([' ', '\t', '\r']) != "@'" {
        return None;
    }

    // Stop at the first closing marker. Looking for the last one could swallow commands
    // between separate here-strings. Only literal here-strings are safe to intercept.
    let (patch, suffix) = match body.strip_prefix("'@") {
        Some(suffix) => ("", suffix),
        None => body.split_once("\n'@")?,
    };
    let command = suffix
        .trim_start_matches([' ', '\t'])
        .strip_prefix('|')?
        .trim();
    super::APPLY_PATCH_COMMANDS
        .iter()
        .any(|name| command.eq_ignore_ascii_case(name))
        .then(|| patch.strip_suffix('\r').unwrap_or(patch))
}

#[cfg(test)]
#[path = "powershell_tests.rs"]
mod tests;
