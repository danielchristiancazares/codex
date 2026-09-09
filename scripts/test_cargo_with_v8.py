import contextlib
import io
import subprocess
import unittest
from unittest.mock import patch

import cargo_with_v8
from codex_package import v8
from codex_package.targets import TARGET_SPECS


class CargoWithV8Test(unittest.TestCase):
    def test_host_pair_reaches_cargo_with_arguments_and_exit_status_intact(self):
        args = [
            "clippy",
            "--fix",
            "--tests",
            "--allow-dirty",
            "-p",
            "codex-code-mode-runtime",
            "--",
            "--cfg",
            'label="with spaces"',
        ]
        environ = {"RUSTUP_TOOLCHAIN": "stable"}
        overrides = {
            "RUSTY_V8_ARCHIVE": "/cache with spaces/v8.a.gz",
            "RUSTY_V8_SRC_BINDING_PATH": "/cache with spaces/binding.rs",
        }
        with (
            patch.object(
                cargo_with_v8.subprocess,
                "check_output",
                return_value="x86_64-unknown-linux-gnu\n",
            ) as host,
            patch.object(
                cargo_with_v8, "resolve_codex_v8_cargo_env", return_value=overrides
            ) as resolve,
            patch.object(
                cargo_with_v8.subprocess,
                "run",
                return_value=subprocess.CompletedProcess(["cargo", *args], 37),
            ) as run,
        ):
            self.assertEqual(cargo_with_v8.main(args, environ), 37)

        host.assert_called_once_with(
            ["rustc", "--print", "host-tuple"], text=True, env=environ
        )
        resolve.assert_called_once_with(
            TARGET_SPECS["x86_64-unknown-linux-gnu"], environ=environ
        )
        run.assert_called_once_with(
            ["cargo", *args], env={**environ, **overrides}, check=False
        )
        self.assertEqual(environ, {"RUSTUP_TOOLCHAIN": "stable"})

    def test_explicit_target_precedence_and_both_argument_forms(self):
        environ = {"CARGO_BUILD_TARGET": "aarch64-apple-darwin"}
        target = "x86_64-pc-windows-msvc"
        for flags, expected in [
            ([], "aarch64-apple-darwin"),
            (["--target", target], target),
            ([f"--target={target}"], target),
        ]:
            with (
                self.subTest(flags=flags),
                patch.object(cargo_with_v8.subprocess, "check_output") as host,
                patch.object(
                    cargo_with_v8, "resolve_codex_v8_cargo_env", return_value={}
                ) as resolve,
                patch.object(
                    cargo_with_v8.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess(["cargo"], 0),
                ),
            ):
                self.assertEqual(cargo_with_v8.main(["clippy", *flags], environ), 0)
                resolve.assert_called_once_with(TARGET_SPECS[expected], environ=environ)
                host.assert_not_called()

    def test_custom_pair_and_source_build_keep_the_callers_environment(self):
        for environ in [
            {"V8_FROM_SOURCE": "1"},
            {
                "RUSTY_V8_ARCHIVE": "/custom/v8.a.gz",
                "RUSTY_V8_SRC_BINDING_PATH": "/custom/binding.rs",
            },
        ]:
            args = ["clippy", "--target", "custom-target.json"]
            with (
                self.subTest(environ=environ),
                patch.object(cargo_with_v8.subprocess, "check_output") as host,
                patch.object(cargo_with_v8, "resolve_codex_v8_cargo_env") as resolve,
                patch.object(
                    cargo_with_v8.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess(["cargo"], 0),
                ) as run,
            ):
                self.assertEqual(cargo_with_v8.main(args, environ), 0)
                run.assert_called_once_with(["cargo", *args], env=environ, check=False)
                resolve.assert_not_called()
                host.assert_not_called()

    def test_incomplete_override_stops_before_download_or_cargo(self):
        environ = {
            "CARGO_BUILD_TARGET": "x86_64-pc-windows-msvc",
            "RUSTY_V8_ARCHIVE": "/custom/v8.lib.gz",
        }
        with (
            patch.object(v8, "fetch_codex_v8_artifacts") as fetch,
            patch.object(cargo_with_v8.subprocess, "run") as run,
            contextlib.redirect_stderr(io.StringIO()) as stderr,
        ):
            self.assertEqual(cargo_with_v8.main(["clippy"], environ), 1)
            self.assertIn("set together", stderr.getvalue())
            fetch.assert_not_called()
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
