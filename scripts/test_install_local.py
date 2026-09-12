"""Exercise npm payload selection and installation rollback with real directories."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import install_local
from codex_package.layout import build_package_dir
from codex_package.targets import PACKAGE_VARIANTS, TARGET_SPECS, PackageInputs
from install_local import NpmInstallation, find_npm_installation, install_package


class LocalInstallTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.spec = TARGET_SPECS["x86_64-pc-windows-msvc"]
        self.npm_root = self.root / "npm"
        self.npm_prefix = self.root / "prefix"
        self.package_root = self.npm_root / "@openai/codex"
        self.package_root.mkdir(parents=True)
        (self.package_root / "package.json").write_text(
            json.dumps({"name": "@openai/codex"}), encoding="utf-8"
        )
        self.npm_prefix.mkdir()
        self.launcher = self.npm_prefix / "codex.cmd"
        self.launcher.write_text("unchanged npm launcher", encoding="utf-8")
        self.platform_root = self.package_root / "node_modules/@openai/codex-win32-x64"
        self.payload = self.platform_root / "vendor" / self.spec.target
        self.payload.mkdir(parents=True)
        self.build_package(self.payload, "old")
        (self.payload / "obsolete-resource").write_text("old", encoding="utf-8")
        self.package = self.root / "new-package"
        self.package.mkdir()
        self.build_package(self.package, "new")
        self.installation = NpmInstallation(self.payload, self.launcher)
        self.before = self.contents(self.payload)

    def build_package(self, destination: Path, version: str, *, spec=None):
        spec = self.spec if spec is None else spec
        inputs = self.root / f"inputs-{version}"
        inputs.mkdir(exist_ok=True)
        binaries = []
        for name in ["codex", "host", "rg", "runner", "setup", "bwrap"]:
            binary = inputs / name
            binary.write_bytes(f"{version}-{name}".encode())
            binaries.append(binary)
        build_package_dir(
            destination,
            version,
            PACKAGE_VARIANTS["codex"],
            spec,
            PackageInputs(
                entrypoint_bin=binaries[0],
                code_mode_host_bin=binaries[1],
                rg_bin=binaries[2],
                zsh_bin=None,
                bwrap_bin=binaries[5] if spec.is_linux else None,
                codex_command_runner_bin=binaries[3] if spec.is_windows else None,
                codex_windows_sandbox_setup_bin=binaries[4]
                if spec.is_windows
                else None,
            ),
        )

    @staticmethod
    def contents(root: Path):
        return {
            str(path.relative_to(root)): path.read_bytes()
            for path in root.rglob("*")
            if path.is_file()
        }

    def assert_previous_installation(self):
        self.assertEqual(self.contents(self.payload), self.before)
        self.assertEqual(list(self.payload.parent.iterdir()), [self.payload])
        self.assertEqual(self.launcher.read_text(), "unchanged npm launcher")

    def test_builds_native_linux_package_in_npm_release_directory(self):
        for target in [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
        ]:
            with self.subTest(target=target):
                npm_target = target.replace("-gnu", "-musl")
                npm_arch = "arm64" if target.startswith("aarch64-") else "x64"
                npm_root = self.root / target / "npm"
                npm_prefix = self.root / target / "prefix"
                package_root = npm_root / "@openai/codex"
                payload = (
                    package_root
                    / f"node_modules/@openai/codex-linux-{npm_arch}/vendor"
                    / npm_target
                )
                (payload / "bin").mkdir(parents=True)
                (payload / "bin/codex").write_text("old", encoding="utf-8")
                (payload / "obsolete-resource").write_text("old", encoding="utf-8")
                (package_root / "package.json").write_text(
                    json.dumps({"name": "@openai/codex"}), encoding="utf-8"
                )
                launcher = npm_prefix / "bin/codex"
                launcher.parent.mkdir(parents=True)
                launcher.write_text("unchanged npm launcher", encoding="utf-8")
                expected_contents = {}

                def build(command, **kwargs):
                    package = Path(command[command.index("--package-dir") + 1])
                    package.mkdir()
                    build_target = command[command.index("--target") + 1]
                    self.build_package(package, "new", spec=TARGET_SPECS[build_target])
                    expected_contents.update(self.contents(package))

                with (
                    mock.patch("install_local.sys.argv", ["install_local.py"]),
                    mock.patch("install_local.default_target", return_value=npm_target),
                    mock.patch("install_local.shutil.which", return_value="npm"),
                    mock.patch(
                        "install_local.subprocess.check_output",
                        side_effect=[f"{target}\n", str(npm_root), str(npm_prefix)],
                    ) as check_output,
                    mock.patch(
                        "install_local.subprocess.run", side_effect=build
                    ) as run,
                    mock.patch("install_local.codex_version", return_value="new"),
                    mock.patch("builtins.print"),
                ):
                    self.assertEqual(install_local.main(), 0)

                self.assertEqual(
                    check_output.call_args_list,
                    [
                        mock.call(
                            [os.environ.get("RUSTC", "rustc"), "--print", "host-tuple"],
                            cwd=install_local.REPO_ROOT / "codex-rs",
                            text=True,
                        ),
                        mock.call(["npm", "root", "--global"], text=True),
                        mock.call(["npm", "prefix", "--global"], text=True),
                    ],
                )
                run.assert_called_once_with(
                    [
                        sys.executable,
                        str(install_local.REPO_ROOT / "scripts/build_codex_package.py"),
                        "--target",
                        target,
                        "--cargo-profile",
                        "release",
                        "--package-dir",
                        mock.ANY,
                    ],
                    cwd=install_local.REPO_ROOT,
                    check=True,
                )
                self.assertEqual(self.contents(payload), expected_contents)
                self.assertEqual(list(payload.parent.iterdir()), [payload])
                self.assertEqual(launcher.read_text(), "unchanged npm launcher")

    def test_incompatible_rust_host_stops_before_build_or_install(self):
        for target, error in [
            ("unsupported-target", "Unsupported Rust host target"),
            ("aarch64-unknown-linux-gnu", "does not match the npm platform"),
            ("x86_64-pc-windows-msvc", "does not match the npm platform"),
        ]:
            with (
                self.subTest(target=target),
                mock.patch("install_local.sys.argv", ["install_local.py"]),
                mock.patch(
                    "install_local.default_target",
                    return_value="x86_64-unknown-linux-musl",
                ),
                mock.patch(
                    "install_local.subprocess.check_output", return_value=target
                ) as check_output,
                mock.patch("install_local.subprocess.run") as run,
            ):
                with self.assertRaisesRegex(RuntimeError, error):
                    install_local.main()
                check_output.assert_called_once()
                run.assert_not_called()
                self.assert_previous_installation()

    def test_selects_nested_npm_payload(self):
        hoisted = self.npm_root / "@openai/codex-win32-x64/vendor" / self.spec.target
        hoisted.mkdir(parents=True)
        self.build_package(hoisted, "hoisted")
        self.assertEqual(
            find_npm_installation(self.npm_root, self.npm_prefix, self.spec),
            self.installation,
        )

    def test_selects_hoisted_npm_payload(self):
        hoisted = self.npm_root / "@openai/codex-win32-x64"
        self.platform_root.rename(hoisted)
        self.assertEqual(
            find_npm_installation(self.npm_root, self.npm_prefix, self.spec),
            NpmInstallation(hoisted / "vendor" / self.spec.target, self.launcher),
        )

    @unittest.skipIf(
        os.name == "nt", "Creating symlinks may require Windows privileges"
    )
    def test_rejects_payload_symlink_outside_platform_package(self):
        outside = self.root / "outside"
        self.payload.rename(outside)
        self.payload.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "outside"):
            find_npm_installation(self.npm_root, self.npm_prefix, self.spec)
        self.assertEqual(self.contents(outside), self.before)

    @mock.patch("install_local.codex_version", return_value="codex-cli new")
    def test_replaces_complete_package_without_changing_launcher(self, version):
        self.assertEqual(
            install_package(self.package, self.installation, self.spec),
            "codex-cli new",
        )
        self.assertEqual(self.contents(self.payload), self.contents(self.package))
        self.assertEqual(list(self.payload.parent.iterdir()), [self.payload])
        self.assertEqual(self.launcher.read_text(), "unchanged npm launcher")
        self.assertEqual(version.call_args, mock.call(self.launcher))

    @mock.patch("install_local.codex_version", side_effect=["new", "wrong"])
    def test_staging_failure_preserves_previous_package(self, _version):
        with self.assertRaisesRegex(RuntimeError, "staged Codex executable"):
            install_package(self.package, self.installation, self.spec)
        self.assert_previous_installation()

    @mock.patch(
        "install_local.codex_version",
        side_effect=[
            "new",
            "new",
            "new",
            subprocess.CalledProcessError(1, ["codex.cmd"]),
        ],
    )
    def test_launcher_failure_restores_previous_package(self, _version):
        with self.assertRaises(subprocess.CalledProcessError):
            install_package(self.package, self.installation, self.spec)
        self.assert_previous_installation()

    @mock.patch("install_local.codex_version", return_value="new")
    def test_failed_replacement_restores_previous_package(self, _version):
        rename = Path.rename

        def deny_replacement(source, destination):
            if source.name == "package" and destination == self.payload:
                raise PermissionError("replacement denied")
            return rename(source, destination)

        with mock.patch.object(Path, "rename", deny_replacement):
            with self.assertRaisesRegex(PermissionError, "replacement denied"):
                install_package(self.package, self.installation, self.spec)
        self.assert_previous_installation()

    @mock.patch(
        "install_local.codex_version",
        side_effect=[
            "new",
            "new",
            "new",
            subprocess.CalledProcessError(1, ["codex.cmd"]),
        ],
    )
    def test_failed_rollback_preserves_recovery_copy(self, _version):
        rename = Path.rename

        def deny_rollback(source, destination):
            if ".previous-" in source.name:
                raise PermissionError("rollback denied")
            return rename(source, destination)

        with mock.patch.object(Path, "rename", deny_rollback):
            with mock.patch("install_local.sys.stderr") as stderr:
                with self.assertRaisesRegex(PermissionError, "rollback denied"):
                    install_package(self.package, self.installation, self.spec)
        backups = list(self.payload.parent.glob(f".{self.payload.name}.previous-*"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(self.contents(backups[0]), self.before)
        self.assertIn(
            str(backups[0]),
            "".join(call.args[0] for call in stderr.write.call_args_list),
        )

    def test_incomplete_package_preserves_previous_installation(self):
        (self.package / "bin/codex-code-mode-host.exe").unlink()
        with self.assertRaisesRegex(RuntimeError, "Missing package file"):
            install_package(self.package, self.installation, self.spec)
        self.assert_previous_installation()

    def test_wrong_target_preserves_previous_installation(self):
        metadata = self.package / "codex-package.json"
        contents = json.loads(metadata.read_text())
        contents["target"] = "aarch64-pc-windows-msvc"
        metadata.write_text(json.dumps(contents), encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "target"):
            install_package(self.package, self.installation, self.spec)
        self.assert_previous_installation()


if __name__ == "__main__":
    unittest.main()
