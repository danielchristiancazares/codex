use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub(super) struct JsonRpcClient<R, W> {
    reader: BufReader<R>,
    writer: W,
    next_id: u64,
}

impl<R, W> JsonRpcClient<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub(super) fn new(reader: R, writer: W) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            next_id: 1,
        }
    }

    pub(super) async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let payload = serde_json::to_vec(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .map_err(|error| format!("encode Copilot CLI request: {error}"))?;
        self.writer
            .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
            .await
            .map_err(|error| format!("write Copilot CLI request header: {error}"))?;
        self.writer
            .write_all(&payload)
            .await
            .map_err(|error| format!("write Copilot CLI request body: {error}"))?;
        self.writer
            .flush()
            .await
            .map_err(|error| format!("flush Copilot CLI request: {error}"))?;

        loop {
            let response = read_response(&mut self.reader).await?;
            if response.id.as_ref() != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = response.error {
                return Err(format!(
                    "Copilot CLI request `{method}` failed ({}): {}",
                    error.code, error.message
                ));
            }
            return response
                .result
                .ok_or_else(|| format!("Copilot CLI request `{method}` returned no result"));
        }
    }
}

async fn read_response<R>(reader: &mut BufReader<R>) -> Result<JsonRpcResponse, String>
where
    R: AsyncRead + Unpin,
{
    let mut content_length = None;
    let mut total_header_bytes = 0;
    loop {
        let mut line = Vec::new();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|error| format!("read Copilot CLI response header: {error}"))?;
        if bytes == 0 {
            return Err("Copilot CLI closed unexpectedly".to_string());
        }
        total_header_bytes += bytes;
        if total_header_bytes > MAX_HEADER_BYTES {
            return Err("Copilot CLI response headers are too large".to_string());
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        let line = str::from_utf8(&line)
            .map_err(|error| format!("decode Copilot CLI response header: {error}"))?;
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            if content_length.is_some() {
                return Err("Copilot CLI response has duplicate Content-Length".to_string());
            }
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("decode Copilot CLI response length: {error}"))?,
            );
        }
    }

    let content_length = content_length
        .ok_or_else(|| "Copilot CLI response is missing Content-Length".to_string())?;
    if content_length > MAX_MESSAGE_BYTES {
        return Err(format!(
            "Copilot CLI response is too large ({content_length} bytes)"
        ));
    }
    let mut payload = vec![0; content_length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| format!("read Copilot CLI response body: {error}"))?;
    serde_json::from_slice(&payload)
        .map_err(|error| format!("decode Copilot CLI response: {error}"))
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
