use sha2::Digest;
use sha2::Sha256;

/// Returns the stable digest attached to a model-visible execution preview.
pub fn preview_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
