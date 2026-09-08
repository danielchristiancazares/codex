#!/usr/bin/env python3
"""Build this checkout and replace the native payload of the global npm Codex."""

import argparse
from dataclasses import dataclass
import filecmp
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid


REPO_ROOT = Path(__file__).resolve().parents[1]
# The package modules need the checkout root and an import path even with PYTHONSAFEPATH.
sys.path.insert(0, str(REPO_ROOT / "scripts"))
os.environ.setdefault("CODEX_REPO_ROOT", str(REPO_ROOT))

from codex_package.layout import validate_package_dir  # noqa: E402
from codex_package.targets import (  # noqa: E402
    PACKAGE_VARIANTS,
    TARGET_SPECS,
    TargetSpec,
    default_target,
)


@dataclass(frozen=True)
class NpmInstallation:
    payload: Path
    launcher: Path


def find_npm_installation(
    npm_root: Path, npm_prefix: Path, spec: TargetSpec
) -> NpmInstallation:
    npm_root = npm_root.resolve(strict=True)
    package_root = (npm_root / "@openai" / "codex").resolve(strict=True)
    metadata = json.loads((package_root / "package.json").read_text(encoding="utf-8"))
    if metadata.get("name") != "@openai/codex":
        raise RuntimeError(f"{package_root} is not the npm Codex package.")
    system, architecture = spec.dotslash_platform.split("-")
    npm_system = {"macos": "darwin", "linux": "linux", "windows": "win32"}[system]
    npm_arch = {"aarch64": "arm64", "x86_64": "x64"}[architecture]
    platform_name = f"codex-{npm_system}-{npm_arch}"
    candidates = [
        package_root / "node_modules" / "@openai" / platform_name,
        npm_root / "@openai" / platform_name,
    ]
    platform_root = next((path for path in candidates if path.is_dir()), None)
    if platform_root is None:
        raise RuntimeError(
            f"The global npm installation is missing @openai/{platform_name}."
        )
    platform_root = platform_root.resolve(strict=True)
    payload = (platform_root / "vendor" / spec.target).resolve(strict=True)
    if (
        not package_root.is_relative_to(npm_root)
        or not platform_root.is_relative_to(npm_root)
        or not payload.is_relative_to(platform_root)
    ):
        raise RuntimeError("The npm payload resolves outside its installed package.")
    if (
        not payload.is_dir()
        or not (payload / "bin" / f"codex{spec.exe_suffix}").is_file()
    ):
        raise RuntimeError(
            f"The npm Codex payload has an unsupported layout: {payload}"
        )
    launcher = npm_prefix / ("codex.cmd" if spec.is_windows else "bin/codex")
    if not launcher.is_file():
        raise RuntimeError(f"The npm Codex launcher is missing: {launcher}")
    return NpmInstallation(payload=payload, launcher=launcher)


def codex_version(executable: Path) -> str:
    return subprocess.check_output(
        [str(executable), "--version"], text=True, timeout=30
    ).strip()


def verify_package(package: Path, spec: TargetSpec) -> None:
    validate_package_dir(
        package,
        PACKAGE_VARIANTS["codex"],
        spec,
        include_zsh=(package / "codex-resources/zsh/bin/zsh").is_file(),
    )
    if spec.dotslash_platform.startswith("macos-"):
        architecture = "arm64" if spec.target.startswith("aarch64-") else "x86_64"
        for path in package.rglob("*"):
            if path.is_file() and path.stat().st_mode & 0o111:
                subprocess.run(
                    ["lipo", str(path), "-verify_arch", architecture], check=True
                )


def install_package(
    package: Path, installation: NpmInstallation, spec: TargetSpec
) -> str:
    verify_package(package, spec)
    expected_version = codex_version(package / "bin" / f"codex{spec.exe_suffix}")
    payload = installation.payload
    lock = payload.parent / f".{payload.name}.local-install.lock"
    try:
        lock.mkdir()
    except FileExistsError:
        raise RuntimeError(f"Another local installation holds {lock}.") from None
    try:
        with tempfile.TemporaryDirectory(
            prefix=f".{payload.name}.installing-", dir=payload.parent
        ) as temporary:
            staged = Path(temporary) / "package"
            shutil.copytree(package, staged)
            verify_package(staged, spec)
            for path in package.rglob("*"):
                if path.is_file() and not filecmp.cmp(
                    path, staged / path.relative_to(package), shallow=False
                ):
                    raise RuntimeError(
                        f"The staged package differs at {path.relative_to(package)}."
                    )
            if (
                codex_version(staged / "bin" / f"codex{spec.exe_suffix}")
                != expected_version
            ):
                raise RuntimeError(
                    "The staged Codex executable reported a different version."
                )

            backup = payload.with_name(f".{payload.name}.previous-{uuid.uuid4().hex}")
            payload.rename(backup)
            try:
                staged.rename(payload)
                installed_version = codex_version(
                    payload / "bin" / f"codex{spec.exe_suffix}"
                )
                launcher_version = codex_version(installation.launcher)
                if (
                    installed_version != expected_version
                    or launcher_version != expected_version
                ):
                    raise RuntimeError(
                        "The installed Codex launcher reported a different version."
                    )
            except BaseException:
                try:
                    if payload.exists():
                        payload.rename(Path(temporary) / "failed")
                    backup.rename(payload)
                except OSError:
                    print(
                        f"Restore the previous npm payload from {backup}.",
                        file=sys.stderr,
                    )
                    raise
                raise
            try:
                shutil.rmtree(backup)
            except OSError as error:
                print(
                    f"Previous payload retained at {backup}: {error}", file=sys.stderr
                )
            return expected_version
    finally:
        lock.rmdir()


def main() -> int:
    argparse.ArgumentParser(description=__doc__).parse_args()
    spec = TARGET_SPECS[default_target()]
    npm = shutil.which("npm.cmd" if spec.is_windows else "npm")
    if npm is None:
        raise RuntimeError(
            "npm is required to locate the existing global Codex installation."
        )
    npm_root = Path(
        subprocess.check_output([npm, "root", "--global"], text=True).strip()
    )
    npm_prefix = Path(
        subprocess.check_output([npm, "prefix", "--global"], text=True).strip()
    )
    installation = find_npm_installation(npm_root, npm_prefix, spec)
    print(f"Building a release Codex package for {spec.target}", flush=True)
    with tempfile.TemporaryDirectory(prefix="codex-local-build-") as temporary:
        package = Path(temporary) / "package"
        subprocess.run(
            [
                sys.executable,
                str(REPO_ROOT / "scripts/build_codex_package.py"),
                "--target",
                spec.target,
                "--cargo-profile",
                "release",
                "--package-dir",
                str(package),
            ],
            cwd=REPO_ROOT,
            check=True,
        )
        version = install_package(package, installation, spec)
    print(f"Installed {version} at {installation.payload}")
    print(f"npm command: {installation.launcher}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Local Codex installation failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
