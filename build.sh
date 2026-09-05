#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./build-and-install-local.sh

Build a release Codex package for the current macOS architecture and replace
the native payload in the global @openai/codex npm installation.

Supported hosts:
  Apple Silicon macOS (aarch64-apple-darwin)
  Intel macOS         (x86_64-apple-darwin)

The Intel build uses RUSTY_V8_ARCHIVE and RUSTY_V8_SRC_BINDING_PATH when set.
They otherwise default to the matching files under $HOME/rusty_v8.

Outputs:
  dist/codex-package-<target>/
  dist/codex-package-<target>.tar.gz

Replaces:
  bin/codex
  bin/codex-code-mode-host
  codex-path/rg
  codex-resources/zsh/bin/zsh
EOF
}

die() {
  echo "$1" >&2
  exit 1
}

verify_executables() {
  local root="$1"
  local description="$2"
  local relative_path
  local executable

  for relative_path in "${executable_paths[@]}"; do
    executable="$root/$relative_path"
    if [[ ! -x "$executable" ]]; then
      die "$description executable is missing: $executable"
    fi
    if ! lipo "$executable" -verify_arch "$EXPECTED_ARCH" >/dev/null 2>&1; then
      echo "$description executable does not contain $EXPECTED_ARCH: $executable" >&2
      lipo -info "$executable" >&2
      exit 1
    fi
  done
}

assert_executables_match() {
  local source_root="$1"
  local destination_root="$2"
  local relative_path

  for relative_path in "${executable_paths[@]}"; do
    if ! cmp -s \
      "$source_root/$relative_path" \
      "$destination_root/$relative_path"; then
      die "$destination_root/$relative_path does not match $source_root/$relative_path."
    fi
  done
}

run_install_command() {
  if (( use_sudo )); then
    sudo "$@"
  else
    "$@"
  fi
}

if (( $# > 0 )); then
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  die "This script requires macOS."
fi

case "$(uname -m)" in
  arm64 | aarch64)
    TARGET="aarch64-apple-darwin"
    EXPECTED_ARCH="arm64"
    PLATFORM_PACKAGE="codex-darwin-arm64"
    ;;
  x86_64 | amd64)
    TARGET="x86_64-apple-darwin"
    EXPECTED_ARCH="x86_64"
    PLATFORM_PACKAGE="codex-darwin-x64"
    ;;
  *)
    die "Unsupported macOS architecture: $(uname -m)"
    ;;
esac

PACKAGE_STEM="codex-package-$TARGET"
PACKAGE_DIR="$REPO_ROOT/dist/$PACKAGE_STEM"
ARCHIVE_PATH="$REPO_ROOT/dist/$PACKAGE_STEM.tar.gz"
executable_paths=(
  "bin/codex"
  "bin/codex-code-mode-host"
  "codex-path/rg"
  "codex-resources/zsh/bin/zsh"
)

for tool in cargo cmp ditto lipo mktemp npm python3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    die "$tool is required to build and install Codex."
  fi
done

if ! xcode-select -p >/dev/null 2>&1; then
  die "Xcode Command Line Tools are required; run: xcode-select --install"
fi

npm_root="$(npm root --global)"
if [[ ! -d "$npm_root" ]]; then
  die "npm returned a global package directory that does not exist: $npm_root"
fi
NPM_ROOT="$(cd "$npm_root" && pwd -P)"

npm_prefix="$(npm prefix --global)"
if [[ ! -d "$npm_prefix" ]]; then
  die "npm returned a global prefix that does not exist: $npm_prefix"
fi
NPM_PREFIX="$(cd "$npm_prefix" && pwd -P)"

CODEX_PACKAGE_DIR="$NPM_ROOT/@openai/codex"
if [[ ! -f "$CODEX_PACKAGE_DIR/package.json" ]]; then
  die "Could not find the global @openai/codex package. Run: npm install -g @openai/codex"
fi

nested_platform_package="$CODEX_PACKAGE_DIR/node_modules/@openai/$PLATFORM_PACKAGE"
hoisted_platform_package="$NPM_ROOT/@openai/$PLATFORM_PACKAGE"
if [[ -d "$nested_platform_package" ]]; then
  PLATFORM_PACKAGE_DIR="$(cd "$nested_platform_package" && pwd -P)"
elif [[ -d "$hoisted_platform_package" ]]; then
  PLATFORM_PACKAGE_DIR="$(cd "$hoisted_platform_package" && pwd -P)"
else
  die "Could not find the installed @openai/$PLATFORM_PACKAGE npm package."
fi

INSTALL_DIR="$PLATFORM_PACKAGE_DIR/vendor/$TARGET"
if [[ ! -d "$INSTALL_DIR" ]]; then
  die "Could not find the npm Codex payload at $INSTALL_DIR."
fi
INSTALL_DIR="$(cd "$INSTALL_DIR" && pwd -P)"
INSTALL_PARENT="$(cd "$(dirname "$INSTALL_DIR")" && pwd -P)"

case "$INSTALL_DIR/" in
  "$PLATFORM_PACKAGE_DIR"/*/) ;;
  *)
    die "Refusing to modify $INSTALL_DIR because it is outside $PLATFORM_PACKAGE_DIR."
    ;;
esac

CODEX_SHIM="$NPM_PREFIX/bin/codex"
if [[ ! -x "$CODEX_SHIM" ]]; then
  die "Could not find the global npm Codex command at $CODEX_SHIM."
fi

verify_executables "$INSTALL_DIR" "Installed"

build_env=(
  "CODEX_REPO_ROOT=$REPO_ROOT"
  "MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-12.0}"
  "V8_FORCE_DEBUG=0"
  "V8_FROM_SOURCE=0"
)

v8_archive="${RUSTY_V8_ARCHIVE:-}"
v8_bindings="${RUSTY_V8_SRC_BINDING_PATH:-}"
if [[ "$TARGET" == "x86_64-apple-darwin" && -z "$v8_archive" && -z "$v8_bindings" ]]; then
  v8_archive="$HOME/rusty_v8/target/release/gn_out/obj/librusty_v8.a"
  v8_bindings="$HOME/rusty_v8/target/release/gn_out/src_binding.rs"
fi

if [[ -n "$v8_archive" || -n "$v8_bindings" ]]; then
  if [[ -z "$v8_archive" || -z "$v8_bindings" ]]; then
    die "RUSTY_V8_ARCHIVE and RUSTY_V8_SRC_BINDING_PATH must be set together."
  fi
  if [[ ! -s "$v8_archive" ]]; then
    die "The rusty_v8 archive is missing or empty: $v8_archive"
  fi
  if [[ ! -s "$v8_bindings" ]]; then
    die "The rusty_v8 binding is missing or empty: $v8_bindings"
  fi
  if ! lipo "$v8_archive" -verify_arch "$EXPECTED_ARCH" >/dev/null 2>&1; then
    echo "The rusty_v8 archive does not contain $EXPECTED_ARCH: $v8_archive" >&2
    lipo -info "$v8_archive" >&2
    exit 1
  fi
  build_env+=(
    "RUSTY_V8_ARCHIVE=$v8_archive"
    "RUSTY_V8_SRC_BINDING_PATH=$v8_bindings"
  )
fi

echo "==> Building Codex for $TARGET"
env "${build_env[@]}" \
  python3 "$REPO_ROOT/scripts/build_codex_package.py" \
    --target "$TARGET" \
    --cargo-profile release \
    --package-dir "$PACKAGE_DIR" \
    --archive-output "$ARCHIVE_PATH" \
    --force

echo "==> Verifying package"
verify_executables "$PACKAGE_DIR" "Package"
version="$("$PACKAGE_DIR/bin/codex" --version)"

use_sudo=0
if [[ ! -w "$INSTALL_PARENT" ]]; then
  if ! command -v sudo >/dev/null 2>&1; then
    die "$INSTALL_PARENT is not writable and sudo is unavailable."
  fi
  echo "==> Requesting permission to update the global npm package"
  sudo -v
  use_sudo=1
fi

staging_dir="$(run_install_command mktemp -d "$INSTALL_PARENT/.${TARGET}.installing.XXXXXX")"
backup_dir="$INSTALL_PARENT/.${TARGET}.previous.$$"
failed_dir="$staging_dir.failed"
swap_started=0

cleanup() {
  local status=$?
  trap - EXIT INT TERM

  if (( swap_started )) && [[ -e "$backup_dir" || -L "$backup_dir" ]]; then
    echo "==> Restoring the previous npm payload" >&2
    if [[ -e "$INSTALL_DIR" || -L "$INSTALL_DIR" ]]; then
      run_install_command mv "$INSTALL_DIR" "$failed_dir" || true
    fi
    if run_install_command mv "$backup_dir" "$INSTALL_DIR"; then
      if [[ -e "$failed_dir" || -L "$failed_dir" ]]; then
        run_install_command rm -rf -- "$failed_dir" || true
      fi
    else
      echo "Could not restore the previous npm payload from $backup_dir." >&2
    fi
  fi

  if [[ -n "${staging_dir:-}" && ( -e "$staging_dir" || -L "$staging_dir" ) ]]; then
    run_install_command rm -rf -- "$staging_dir" || true
  fi

  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

echo "==> Staging $version beside the npm payload"
run_install_command ditto "$PACKAGE_DIR" "$staging_dir"
run_install_command chmod 0755 "$staging_dir"
verify_executables "$staging_dir" "Staged"
assert_executables_match "$PACKAGE_DIR" "$staging_dir"

staged_version="$("$staging_dir/bin/codex" --version)"
if [[ "$staged_version" != "$version" ]]; then
  die "$staging_dir/bin/codex reported '$staged_version'; expected '$version'."
fi

if [[ -e "$backup_dir" || -L "$backup_dir" ]]; then
  die "Temporary backup path already exists: $backup_dir"
fi

echo "==> Replacing npm payload at $INSTALL_DIR"
run_install_command mv "$INSTALL_DIR" "$backup_dir"
swap_started=1
run_install_command mv "$staging_dir" "$INSTALL_DIR"

verify_executables "$INSTALL_DIR" "Installed"
assert_executables_match "$PACKAGE_DIR" "$INSTALL_DIR"
reported_version="$("$INSTALL_DIR/bin/codex" --version)"
if [[ "$reported_version" != "$version" ]]; then
  die "$INSTALL_DIR/bin/codex reported '$reported_version'; expected '$version'."
fi

shim_version="$("$CODEX_SHIM" --version)"
if [[ "$shim_version" != "$version" ]]; then
  die "$CODEX_SHIM reported '$shim_version'; expected '$version'."
fi

swap_started=0
if ! run_install_command rm -rf -- "$backup_dir"; then
  echo "WARNING: Previous npm payload remains at $backup_dir." >&2
fi
trap - EXIT INT TERM
hash -r

echo "$reported_version"
echo "==> npm command:       $CODEX_SHIM"
echo "==> Replaced payload:  $INSTALL_DIR"
echo "==> Package directory: $PACKAGE_DIR"
echo "==> Package archive:   $ARCHIVE_PATH"
