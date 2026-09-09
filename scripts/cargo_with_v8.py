#!/usr/bin/env python3
"""Run Cargo with the verified V8 archive/binding pair published by OpenAI."""

import argparse
import os
import subprocess
import sys
from collections.abc import Mapping

from codex_package.targets import TARGET_SPECS
from codex_package.v8 import resolve_codex_v8_cargo_env


def main(cargo_args: list[str], environ: Mapping[str, str]) -> int:
    cargo_env = dict(environ)
    try:
        if environ.get("V8_FROM_SOURCE") in {"true", "1", "yes"} or (
            environ.get("RUSTY_V8_ARCHIVE") and environ.get("RUSTY_V8_SRC_BINDING_PATH")
        ):
            # Preserve explicit source builds and archive/binding overrides,
            # including targets outside the published OpenAI artifact matrix.
            pass
        else:
            parser = argparse.ArgumentParser(
                description=__doc__, add_help=False, allow_abbrev=False
            )
            parser.add_argument(
                "--target", action="append", choices=TARGET_SPECS, default=[]
            )
            # Arguments after `--` belong to Clippy/rustc, not Cargo.
            build_args = (
                cargo_args[: cargo_args.index("--")]
                if "--" in cargo_args
                else cargo_args
            )
            options, _ = parser.parse_known_args(build_args)
            if len(set(options.target)) > 1:
                parser.error("Run Cargo once per target to select a matching V8 pair.")
            if options.target:
                spec = TARGET_SPECS[options.target[0]]
            elif "CARGO_BUILD_TARGET" in environ:
                spec = TARGET_SPECS[environ["CARGO_BUILD_TARGET"]]
            else:
                # Package defaults select musl on Linux; local Cargo builds use
                # the active Rust toolchain's host unless a target is explicit.
                spec = TARGET_SPECS[
                    subprocess.check_output(
                        [environ.get("RUSTC", "rustc"), "--print", "host-tuple"],
                        text=True,
                        env=environ,
                    ).strip()
                ]
            cargo_env.update(resolve_codex_v8_cargo_env(spec, environ=environ))

        return subprocess.run(
            ["cargo", *cargo_args], env=cargo_env, check=False
        ).returncode
    except KeyError as error:
        print(
            f"OpenAI V8 artifacts are unavailable for Cargo target {error.args[0]!r}. "
            "Supply RUSTY_V8_ARCHIVE and RUSTY_V8_SRC_BINDING_PATH together "
            "to use a custom artifact pair.",
            file=sys.stderr,
        )
        return 1
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(
            f"Could not prepare Cargo's OpenAI V8 artifacts: {error}", file=sys.stderr
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:], os.environ))
