#!/usr/bin/env bash
# Install the jinja-lsp Zed extension for local development.
# Requires: Rust, wasm32-wasip2 target, Zed, python3.11+ (uses tomllib).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXT_DIR="$REPO_ROOT/editors/zed"

echo "Building extension WASM..."
(
  cd "$EXT_DIR"
  cargo build --release --target wasm32-wasip2 2>&1
)

WASM_BIN="$EXT_DIR/target/wasm32-wasip2/release/jinja_lsp_zed.wasm"
if [[ ! -f "$WASM_BIN" ]]; then
  echo "Error: WASM binary not found at $WASM_BIN" >&2
  exit 1
fi

if [[ "$(uname)" == "Darwin" ]]; then
  ZED_EXT_BASE="$HOME/Library/Application Support/Zed/extensions"
else
  ZED_EXT_BASE="${XDG_DATA_HOME:-$HOME/.local/share}/zed/extensions"
fi

EXT_ID=$(grep -m1 '^id = ' "$EXT_DIR/extension.toml" | sed -E 's/^id = "(.*)"$/\1/')
TARGET="$ZED_EXT_BASE/installed/$EXT_ID"
INDEX="$ZED_EXT_BASE/index.json"

rm -rf "$TARGET"
mkdir -p "$TARGET"
cp "$EXT_DIR/extension.toml" "$TARGET/"
cp -r "$EXT_DIR/languages" "$TARGET/"
cp "$WASM_BIN" "$TARGET/extension.wasm"
echo "Copied extension files to $TARGET"

if [[ -f "$INDEX" ]]; then
  python3 - "$INDEX" "$EXT_DIR/extension.toml" <<'PYEOF'
import json, sys, tomllib

index_path, ext_toml_path = sys.argv[1], sys.argv[2]

with open(ext_toml_path, "rb") as f:
    ext = tomllib.load(f)

with open(index_path) as f:
    index = json.load(f)

if "extensions" not in index:
    index["extensions"] = {}

grammars = {
    name: {"repository": g["repository"], "commit": g["commit"], "rev": g["commit"]}
    for name, g in ext.get("grammars", {}).items()
}

language_servers = {
    name: {
        "language": None,
        "languages": ls.get("languages", []),
        "language_ids": {},
        "code_action_kinds": None,
    }
    for name, ls in ext.get("language_servers", {}).items()
}

# Generated from extension.toml itself, so the index entry can never contradict
# the manifest that Zed actually reads from disk (jinja-lsp-swvz).
index["extensions"][ext["id"]] = {
    "manifest": {
        "id": ext["id"],
        "name": ext["name"],
        "version": ext["version"],
        "schema_version": ext.get("schema_version", 1),
        "description": ext.get("description", ""),
        "repository": ext.get("repository", ""),
        "authors": ext.get("authors", []),
        "lib": {"kind": "Rust", "version": "0.2.0"},
        "themes": [],
        "icon_themes": [],
        "languages": ["languages/jinja2"],
        "grammars": grammars,
        "language_servers": language_servers,
        "context_servers": {},
        "slash_commands": {},
        "snippets": None,
        "capabilities": []
    },
    "dev": False
}

with open(index_path, "w") as f:
    json.dump(index, f, indent=2)

print(f"Registered {ext['id']} in {index_path}")
PYEOF
else
  echo "Warning: $INDEX not found — start Zed first, then re-run this script."
fi

echo ""
echo "Done. Restart Zed to activate the extension."
echo "Then ensure the jinja-lsp binary is on PATH (e.g. \`just install-zed\` copies it to ~/.cargo/bin)."
