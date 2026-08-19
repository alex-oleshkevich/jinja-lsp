#!/usr/bin/env bash
# Build the Zed extension WASM binary and package it for a GitHub Release.
# Usage: ./scripts/package-zed-extension.sh [dist-dir]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${1:-.}"
ZED_SRC="$REPO_ROOT/editors/zed"
WASM_TARGET="wasm32-wasip2"
VERSION=$(cargo metadata --no-deps --format-version 1 --manifest-path "$REPO_ROOT/Cargo.toml" | jq -r '.packages[0].version')

mkdir -p "$DIST"
# Resolve to an absolute path before the `cd "$STAGE"` below: zip writes relative
# to its own cwd, and prefixing with $OLDPWD instead breaks the moment the caller
# passes an absolute dist-dir (it concatenates the two into a nonexistent path).
DIST="$(cd "$DIST" && pwd)"
OUTPUT="$DIST/jinja-lsp-zed-$VERSION.zip"

rustup target add "$WASM_TARGET"

(
  cd "$ZED_SRC"
  cargo build --release --target "$WASM_TARGET"
)

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

cp "$ZED_SRC/extension.toml" "$STAGE/"
cp "$REPO_ROOT/LICENSE" "$STAGE/"
cp -r "$ZED_SRC/languages" "$STAGE/"
cp "$ZED_SRC/target/$WASM_TARGET/release/jinja_lsp_zed.wasm" "$STAGE/extension.wasm"

(cd "$STAGE" && zip -qr "$OUTPUT" .)

echo "Packaged Zed extension → $OUTPUT"
