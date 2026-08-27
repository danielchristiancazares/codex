use std::collections::HashMap;
use std::io::Cursor;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::CODEX_WINDOWS_SANDBOX_ARG1;
use super::CONTROL_FRAME_MAGIC;
use super::MAX_CONTROL_PAYLOAD_LEN;
use super::WindowsSandboxWrapperReadRoots;
use super::WindowsSandboxWrapperRequest;
use super::create_windows_sandbox_command_for_permission_profile;
use super::read_windows_sandbox_wrapper_request;

#[test]
fn windows_wrapper_control_frame_round_trips_large_request_and_preserves_following_input() {
    let command_cwd = AbsolutePathBuf::from_absolute_path(Path::new(r"C:\workspace"))
        .expect("absolute command cwd");
    let workspace_roots = vec![
        command_cwd.clone(),
        AbsolutePathBuf::from_absolute_path(Path::new(r"D:\other-workspace"))
            .expect("absolute workspace root"),
    ];
    let env = HashMap::from([
        ("Path".to_string(), r"C:\Windows\System32".to_string()),
        ("LARGE_VALUE".to_string(), "x".repeat(40_000)),
    ]);
    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let read_roots_override = vec![PathBuf::from(r"C:\read")];
    let write_roots_override = vec![PathBuf::from(r"C:\write")];
    let deny_read_paths_override = vec![
        AbsolutePathBuf::from_absolute_path(Path::new(r"C:\blocked-read"))
            .expect("absolute deny-read"),
    ];
    let deny_write_paths_override = vec![
        AbsolutePathBuf::from_absolute_path(Path::new(r"C:\blocked-write"))
            .expect("absolute deny-write"),
    ];
    let inner_command = vec![
        "codex.exe".to_string(),
        "--codex-run-as-fs-helper".to_string(),
    ];
    let codex_home = Path::new(r"C:\Users\me\.codex");
    let expected = WindowsSandboxWrapperRequest {
        codex_home: codex_home.to_path_buf(),
        command_cwd: command_cwd.clone(),
        workspace_roots: workspace_roots.clone(),
        env_map: env.clone(),
        permission_profile: permission_profile.clone(),
        windows_sandbox_level: WindowsSandboxLevel::Elevated,
        windows_sandbox_private_desktop: true,
        managed_network: crate::WindowsSandboxManagedNetwork::Enforced {
            network_proxy_restricting_sid: "S-1-5-21-100-200-300-400".to_string(),
            proxy_settings_mode: crate::WindowsSandboxProxySettingsMode::Preserve,
        },
        read_roots: WindowsSandboxWrapperReadRoots::Explicit {
            roots: read_roots_override.clone(),
            include_platform_defaults: true,
        },
        write_roots_override: Some(write_roots_override.clone()),
        deny_read_paths_override: deny_read_paths_override.clone(),
        deny_write_paths_override: deny_write_paths_override.clone(),
        command: inner_command.clone(),
    };

    let wrapper_command = create_windows_sandbox_command_for_permission_profile(
        inner_command,
        &command_cwd,
        workspace_roots.as_slice(),
        &env,
        &permission_profile,
        WindowsSandboxLevel::Elevated,
        /*windows_sandbox_private_desktop*/ true,
        crate::WindowsSandboxManagedNetwork::Enforced {
            network_proxy_restricting_sid: "S-1-5-21-100-200-300-400".to_string(),
            proxy_settings_mode: crate::WindowsSandboxProxySettingsMode::Preserve,
        },
        crate::WindowsSandboxReadRoots::Explicit {
            roots: read_roots_override.as_slice(),
            include_platform_defaults: true,
        },
        Some(write_roots_override.as_slice()),
        deny_read_paths_override.as_slice(),
        deny_write_paths_override.as_slice(),
        codex_home,
    )
    .expect("create wrapper command");

    assert_eq!(
        wrapper_command.args,
        vec![CODEX_WINDOWS_SANDBOX_ARG1.to_string()]
    );
    assert!(wrapper_command.stdin_prelude.len() > 32_767);

    let trailing_input = b"filesystem helper request";
    let mut framed_input = wrapper_command.stdin_prelude;
    framed_input.extend_from_slice(trailing_input);
    let mut reader = Cursor::new(framed_input);
    let parsed =
        read_windows_sandbox_wrapper_request(Vec::new(), &mut reader).expect("read request");
    assert_eq!(parsed, expected);

    let mut remaining = Vec::new();
    reader.read_to_end(&mut remaining).expect("read input");
    assert_eq!(remaining, trailing_input);
}

#[test]
fn windows_wrapper_control_frame_round_trips_tagged_authority_variants() {
    let command_cwd = AbsolutePathBuf::from_absolute_path(Path::new(r"C:\workspace"))
        .expect("absolute command cwd");
    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let explicit_empty_roots = Vec::<PathBuf>::new();
    let cases = [
        (
            crate::WindowsSandboxManagedNetwork::Disabled {
                proxy_settings_mode: crate::WindowsSandboxProxySettingsMode::Reconcile,
            },
            crate::WindowsSandboxReadRoots::ProfileDefaults,
            WindowsSandboxWrapperReadRoots::ProfileDefaults,
            "disabled",
            "profile_defaults",
        ),
        (
            crate::WindowsSandboxManagedNetwork::Enforced {
                network_proxy_restricting_sid: "S-1-5-21-100-200-300-400".to_string(),
                proxy_settings_mode: crate::WindowsSandboxProxySettingsMode::Preserve,
            },
            crate::WindowsSandboxReadRoots::Explicit {
                roots: explicit_empty_roots.as_slice(),
                include_platform_defaults: true,
            },
            WindowsSandboxWrapperReadRoots::Explicit {
                roots: explicit_empty_roots.clone(),
                include_platform_defaults: true,
            },
            "enforced",
            "explicit",
        ),
    ];

    for (
        managed_network,
        read_roots,
        expected_read_roots,
        expected_managed_network_type,
        expected_read_roots_type,
    ) in cases
    {
        let expected = WindowsSandboxWrapperRequest {
            codex_home: PathBuf::from(r"C:\Users\me\.codex"),
            command_cwd: command_cwd.clone(),
            workspace_roots: vec![command_cwd.clone()],
            env_map: HashMap::new(),
            permission_profile: permission_profile.clone(),
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            windows_sandbox_private_desktop: false,
            managed_network: managed_network.clone(),
            read_roots: expected_read_roots,
            write_roots_override: None,
            deny_read_paths_override: Vec::new(),
            deny_write_paths_override: Vec::new(),
            command: vec!["cmd.exe".to_string()],
        };
        let wrapper_command = create_windows_sandbox_command_for_permission_profile(
            vec!["cmd.exe".to_string()],
            &command_cwd,
            &[],
            &HashMap::new(),
            &permission_profile,
            WindowsSandboxLevel::Elevated,
            /*windows_sandbox_private_desktop*/ false,
            managed_network,
            read_roots,
            /*write_roots_override*/ None,
            &[],
            &[],
            Path::new(r"C:\Users\me\.codex"),
        )
        .expect("create wrapper command");

        let encoded: serde_json::Value =
            serde_json::from_slice(&wrapper_command.stdin_prelude[8..])
                .expect("decode wrapper request JSON");
        assert_eq!(
            encoded["managed_network"]["type"],
            expected_managed_network_type
        );
        assert_eq!(encoded["read_roots"]["type"], expected_read_roots_type);
        let parsed = read_windows_sandbox_wrapper_request(
            Vec::new(),
            wrapper_command.stdin_prelude.as_slice(),
        )
        .expect("read request");
        assert_eq!(parsed, expected);
    }
}

#[test]
fn windows_wrapper_control_frame_rejects_oversized_payload_before_allocation() {
    let mut frame = Vec::from(CONTROL_FRAME_MAGIC.as_slice());
    frame.extend_from_slice(&((MAX_CONTROL_PAYLOAD_LEN + 1) as u32).to_le_bytes());

    let err = read_windows_sandbox_wrapper_request(Vec::new(), frame.as_slice())
        .expect_err("oversized frame should fail");

    assert!(
        err.to_string()
            .contains("windows sandbox request is too large")
    );
}
