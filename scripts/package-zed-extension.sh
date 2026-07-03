#!/usr/bin/env bash
# Build the Zed extension WASM binary and package it for a GitHub Release.
# Usage: ./scripts/package-zed-extension.sh [dist-dir]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${1:-.}"
ZED_SRC="$REPO_ROOT/editors/zed"
WASM_TARGET="wasm32-wasip2"
VERSION=$(cargo metadata --no-deps --format-version 1 --manifest-path "$REPO_ROOT/Cargo.toml" | jq -r '.packages[0].version')
OUTPUT="$DIST/jinja-lsp-zed-$VERSION.zip"

mkdir -p "$DIST"

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

(cd "$STAGE" && zip -r "$OLDPWD/$OUTPUT" .)

echo "Packaged Zed extension → $OUTPUT"
