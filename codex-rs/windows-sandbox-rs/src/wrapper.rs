//! Internal `codex.exe --run-as-windows-sandbox` wrapper.
//!
//! This gives direct-spawn callers a Windows sandbox launcher analogous to the
//! macOS seatbelt and Linux sandbox wrapper paths. The wrapper reads a framed
//! sandbox request from stdin, launches the requested inner command in a
//! Windows sandbox session, and forwards the remaining stdio to that command.

use std::collections::HashMap;
use std::io::Read;
use std::mem::size_of;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

pub const CODEX_WINDOWS_SANDBOX_ARG1: &str = "--run-as-windows-sandbox";

const CONTROL_FRAME_MAGIC: &[u8; 4] = b"CWS1";
const CONTROL_FRAME_HEADER_LEN: usize = CONTROL_FRAME_MAGIC.len() + size_of::<u32>();
const MAX_CONTROL_PAYLOAD_LEN: usize = 64 * 1024 * 1024;

/// Fixed-size wrapper argv and the framed control request to write to stdin.
#[derive(Debug, PartialEq, Eq)]
pub struct WindowsSandboxCommand {
    /// Arguments appended to the Codex executable used as the wrapper.
    pub args: Vec<String>,
    /// Bytes that must precede input intended for the sandboxed command.
    pub stdin_prelude: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
pub fn create_windows_sandbox_command_for_permission_profile(
    command: Vec<String>,
    command_cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
    env_map: &HashMap<String, String>,
    permission_profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    windows_sandbox_private_desktop: bool,
    managed_network: crate::WindowsSandboxManagedNetwork,
    read_roots: crate::WindowsSandboxReadRoots<'_>,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[AbsolutePathBuf],
    deny_write_paths_override: &[AbsolutePathBuf],
    codex_home: &Path,
) -> Result<WindowsSandboxCommand> {
    let workspace_roots = if workspace_roots.is_empty() {
        vec![command_cwd.clone()]
    } else {
        workspace_roots.to_vec()
    };
    let request = WindowsSandboxWrapperRequest {
        codex_home: codex_home.to_path_buf(),
        command_cwd: command_cwd.clone(),
        workspace_roots,
        env_map: env_map.clone(),
        permission_profile: permission_profile.clone(),
        windows_sandbox_level,
        windows_sandbox_private_desktop,
        managed_network,
        read_roots: match read_roots {
            crate::WindowsSandboxReadRoots::ProfileDefaults => {
                WindowsSandboxWrapperReadRoots::ProfileDefaults
            }
            crate::WindowsSandboxReadRoots::Explicit {
                roots,
                include_platform_defaults,
            } => WindowsSandboxWrapperReadRoots::Explicit {
                roots: roots.to_vec(),
                include_platform_defaults,
            },
        },
        write_roots_override: write_roots_override.map(<[PathBuf]>::to_vec),
        deny_read_paths_override: deny_read_paths_override.to_vec(),
        deny_write_paths_override: deny_write_paths_override.to_vec(),
        command,
    };
    Ok(WindowsSandboxCommand {
        args: vec![CODEX_WINDOWS_SANDBOX_ARG1.to_string()],
        stdin_prelude: encode_windows_sandbox_wrapper_request(&request)?,
    })
}

pub fn run_windows_sandbox_wrapper_main() -> ! {
    let args = std::env::args().skip(2).collect::<Vec<_>>();
    let mut stdin = std::io::stdin();
    let request = match read_windows_sandbox_wrapper_request(args, &mut stdin) {
        Ok(request) => request,
        Err(err) => {
            eprintln!("windows sandbox failed: {err:#}");
            std::process::exit(1);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("windows sandbox failed to build runtime: {err}");
            std::process::exit(1);
        }
    };
    let exit_code = match runtime.block_on(run_windows_sandbox_wrapper_request(request, stdin)) {
        Ok(exit_code) => exit_code,
        Err(err) => {
            eprintln!("windows sandbox failed: {err:#}");
            1
        }
    };
    std::process::exit(exit_code);
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WindowsSandboxWrapperReadRoots {
    ProfileDefaults,
    Explicit {
        roots: Vec<PathBuf>,
        include_platform_defaults: bool,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct WindowsSandboxWrapperRequest {
    codex_home: PathBuf,
    command_cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
    env_map: HashMap<String, String>,
    permission_profile: PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    windows_sandbox_private_desktop: bool,
    managed_network: crate::WindowsSandboxManagedNetwork,
    read_roots: WindowsSandboxWrapperReadRoots,
    write_roots_override: Option<Vec<PathBuf>>,
    deny_read_paths_override: Vec<AbsolutePathBuf>,
    deny_write_paths_override: Vec<AbsolutePathBuf>,
    command: Vec<String>,
}

async fn run_windows_sandbox_wrapper_request(
    request: WindowsSandboxWrapperRequest,
    stdin: std::io::Stdin,
) -> Result<i32> {
    if request.command.is_empty() {
        bail!("missing sandboxed command in windows sandbox wrapper request");
    }
    let read_roots = match &request.read_roots {
        WindowsSandboxWrapperReadRoots::ProfileDefaults => {
            crate::WindowsSandboxReadRoots::ProfileDefaults
        }
        WindowsSandboxWrapperReadRoots::Explicit {
            roots,
            include_platform_defaults,
        } => crate::WindowsSandboxReadRoots::Explicit {
            roots,
            include_platform_defaults: *include_platform_defaults,
        },
    };
    let spawned =
        crate::spawn_windows_sandbox_session_for_level(crate::WindowsSandboxSessionRequest {
            permission_profile: &request.permission_profile,
            workspace_roots: request.workspace_roots.as_slice(),
            codex_home: request.codex_home.as_path(),
            command: request.command,
            cwd: request.command_cwd.as_path(),
            env_map: request.env_map,
            windows_sandbox_level: request.windows_sandbox_level,
            managed_network: request.managed_network,
            timeout_ms: None,
            read_roots,
            write_roots_override: request.write_roots_override.as_deref(),
            deny_read_paths_override: request.deny_read_paths_override.as_slice(),
            deny_write_paths_override: request.deny_write_paths_override.as_slice(),
            tty: false,
            stdin_open: true,
            use_private_desktop: request.windows_sandbox_private_desktop,
        })
        .await?;

    Ok(crate::stdio_bridge::forward_sandbox_session_stdio_with_input(spawned, stdin).await)
}

fn encode_windows_sandbox_wrapper_request(
    request: &WindowsSandboxWrapperRequest,
) -> Result<Vec<u8>> {
    let payload =
        serde_json::to_vec(request).context("failed to serialize windows sandbox request")?;
    if payload.len() > MAX_CONTROL_PAYLOAD_LEN {
        bail!(
            "windows sandbox request is too large: {} bytes (maximum {MAX_CONTROL_PAYLOAD_LEN})",
            payload.len()
        );
    }
    let payload_len = payload.len() as u32;
    let mut frame = Vec::with_capacity(CONTROL_FRAME_HEADER_LEN + payload.len());
    frame.extend_from_slice(CONTROL_FRAME_MAGIC);
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn read_windows_sandbox_wrapper_request(
    args: Vec<String>,
    mut reader: impl Read,
) -> Result<WindowsSandboxWrapperRequest> {
    if let Some(arg) = args.first() {
        bail!("unexpected windows sandbox wrapper argument: {arg}");
    }

    let mut magic = [0_u8; CONTROL_FRAME_MAGIC.len()];
    reader
        .read_exact(&mut magic)
        .context("failed to read windows sandbox control frame magic")?;
    if &magic != CONTROL_FRAME_MAGIC {
        bail!("invalid windows sandbox control frame magic");
    }

    let mut payload_len = [0_u8; size_of::<u32>()];
    reader
        .read_exact(&mut payload_len)
        .context("failed to read windows sandbox control frame length")?;
    let payload_len = u32::from_le_bytes(payload_len) as usize;
    if payload_len > MAX_CONTROL_PAYLOAD_LEN {
        bail!(
            "windows sandbox request is too large: {payload_len} bytes \
             (maximum {MAX_CONTROL_PAYLOAD_LEN})"
        );
    }

    let mut payload = vec![0_u8; payload_len];
    reader
        .read_exact(&mut payload)
        .context("failed to read windows sandbox control frame payload")?;
    let mut request: WindowsSandboxWrapperRequest = serde_json::from_slice(&payload)
        .context("failed to deserialize windows sandbox request")?;
    if !request.codex_home.is_absolute() {
        bail!(
            "windows sandbox codex home must be absolute: {}",
            request.codex_home.display()
        );
    }
    if request.workspace_roots.is_empty() {
        request.workspace_roots.push(request.command_cwd.clone());
    }
    Ok(request)
}

#[cfg(test)]
#[path = "wrapper_tests.rs"]
mod tests;
