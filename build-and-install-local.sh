#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

TARGET="x86_64-apple-darwin"
VERSION="$(
  python3 -c 'import tomllib; print(tomllib.load(open("codex-rs/Cargo.toml", "rb"))["workspace"]["package"]["version"])'
)"

PACKAGE_DIR="$REPO_ROOT/dist/codex-package-$TARGET"
INSTALL_ROOT="${CODEX_HOME:-$HOME/.codex}/packages/standalone"
RELEASE_DIR="$INSTALL_ROOT/releases/$VERSION-$TARGET"

V8_ARCHIVE="/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a"
V8_BINDINGS="/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs"

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "x86_64" ]]; then
  echo "This script is intended for an Intel macOS host." >&2
  exit 1
fi

test -s "$V8_ARCHIVE"
test -s "$V8_BINDINGS"
lipo -info "$V8_ARCHIVE" | grep -q "x86_64"

echo "==> Building Codex $VERSION for $TARGET"
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE="$V8_ARCHIVE" \
  RUSTY_V8_SRC_BINDING_PATH="$V8_BINDINGS" \
  python3 scripts/build_codex_package.py \
    --target "$TARGET" \
    --cargo-profile release \
    --package-dir "$PACKAGE_DIR" \
    --archive-output "$REPO_ROOT/dist/codex-package-$TARGET.tar.gz" \
    --force

echo "==> Installing package at $RELEASE_DIR"
mkdir -p "$INSTALL_ROOT/releases"

if [[ -e "$RELEASE_DIR" || -L "$RELEASE_DIR" ]]; then
  BACKUP_DIR="$RELEASE_DIR.backup.$(date +%Y%m%d-%H%M%S)"
  echo "==> Preserving existing release at $BACKUP_DIR"
  mv "$RELEASE_DIR" "$BACKUP_DIR"
fi

ditto "$PACKAGE_DIR" "$RELEASE_DIR"
ln -sfn "bin/codex" "$RELEASE_DIR/codex"
ln -sfn "$RELEASE_DIR" "$INSTALL_ROOT/current"

CODEX_LINK="/usr/local/bin/codex"
CODE_MODE_HOST_LINK="/usr/local/bin/codex-code-mode-host"

if [[ -w "/usr/local/bin" ]]; then
  ln -sfn "$INSTALL_ROOT/current/bin/codex" "$CODEX_LINK"
  ln -sfn \
    "$INSTALL_ROOT/current/bin/codex-code-mode-host" \
    "$CODE_MODE_HOST_LINK"
else
  echo "==> Updating /usr/local/bin links with sudo"
  sudo ln -sfn "$INSTALL_ROOT/current/bin/codex" "$CODEX_LINK"
  sudo ln -sfn \
    "$INSTALL_ROOT/current/bin/codex-code-mode-host" \
    "$CODE_MODE_HOST_LINK"
fi

hash -r

echo "==> Verifying installation"
file "$INSTALL_ROOT/current/bin/codex"
"$CODEX_LINK" --version

echo "==> Installed Codex $VERSION"
