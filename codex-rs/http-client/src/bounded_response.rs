use bytes::Bytes;
use bytes::BytesMut;
use thiserror::Error;

use crate::HttpResponse;

/// Error returned while reading an HTTP response under a byte limit.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BoundedResponseError {
    #[error("response body exceeded the {limit}-byte limit")]
    LimitExceeded { limit: usize },
    #[error("response body could not be read")]
    Read,
}

/// Reads an HTTP response incrementally and stops before retaining more than `limit` bytes.
pub async fn read_response_body_bounded(
    mut response: HttpResponse,
    limit: usize,
) -> Result<Bytes, BoundedResponseError> {
    let mut body = BytesMut::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| BoundedResponseError::Read)?
    {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(BoundedResponseError::LimitExceeded { limit });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}
