# jinja-lsp developer tasks.
#
# `just install-zed` builds the server + Zed extension and installs both locally.
# The extension attaches the jinja-lsp server to the "Jinja2 (HTML)" language
# declared in editors/zed/extension.toml — no extra settings.json opt-in needed.

_default:
    @just --list

install-zed:
    cargo build --release
    cp target/release/jinja-lsp ~/.cargo/bin/
    ./scripts/install-zed-extension.sh

build:
    cargo build

# Full Rust suite via nextest (the runner CI uses — REQ-STACK-05).
test:
    cargo nextest run

# Python LSP-protocol e2e suite (E29 Branch B). Needs target/debug/jinja-lsp.
test-e2e: build
    cd tests/e2e && uv run pytest -q

lint:
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt

# Build the Zed extension the way CI does. It is a separate crate with its own
# lockfile, so the root `cargo` gates never touch it: a version bump in
# editors/zed/Cargo.toml that leaves editors/zed/Cargo.lock stale breaks the
# `--locked` build in CI and nothing locally.
check-zed:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! rustup target list --installed | grep -q wasm32-wasip2; then
        echo "skipping Zed extension build: rustup target add wasm32-wasip2" >&2
        exit 0
    fi
    cd editors/zed
    cargo build --release --locked --target wasm32-wasip2

# The same gates CI runs (.github/workflows/ci.yml), in the same order.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo nextest run
    just check-zed

# Everything `check` runs, plus the Python e2e gate.
check-all: check test-e2e

# Print release notes for the commits since the last tag, grouped by
# Conventional Commit type. Read-only — safe to run any time.
notes:
    #!/usr/bin/env bash
    set -euo pipefail
    last=$(git describe --tags --abbrev=0 2>/dev/null || true)
    if [ -z "$last" ]; then
        echo "No tag found — describing full history." >&2
        range=""
    else
        echo "Changes since ${last}:" >&2
        range="${last}..HEAD"
    fi
    printf '%s\n' "$(git log --no-merges --pretty=format:'%s' ${range} | awk '
        function flush(hdr, body) { if (body != "") printf "\n### %s\n\n%s", hdr, body }
        /^feat(\(|!?:)/  { feat = feat "- " $0 "\n"; next }
        /^fix(\(|!?:)/   { fix  = fix  "- " $0 "\n"; next }
        /^perf(\(|!?:)/  { perf = perf "- " $0 "\n"; next }
        /^docs(\(|!?:)/  { docs = docs "- " $0 "\n"; next }
                         { other = other "- " $0 "\n" }
        END {
            flush("Added", feat); flush("Fixed", fix); flush("Performance", perf)
            flush("Documentation", docs); flush("Other", other)
        }')"

# Cut release VERSION (e.g. `just release 0.2.0`).
#
# Enforces the two gates release.yml enforces (F21 REQ-REL-08/09) BEFORE tagging,
# so a bad tag is never created rather than aborting a release halfway:
#   * Cargo.toml's version must equal VERSION
#   * CHANGELOG.md must carry a dated section for VERSION
# Creates the annotated tag locally with the generated notes as its message.
# Pushing is left to you — pushing the tag is what triggers the release workflow.
release version:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "$(git status --porcelain)" ]; then
        echo "error: working tree is dirty — commit or stash first." >&2
        exit 1
    fi
    cargo_ver=$(grep -m1 '^version' Cargo.toml | sed 's/.*= *"\(.*\)"/\1/')
    if [ "{{version}}" != "$cargo_ver" ]; then
        echo "error: Cargo.toml is $cargo_ver, not {{version}} (F21 REQ-REL-09)." >&2
        exit 1
    fi
    if ! grep -qE "^## \[?{{version}}\]? - [0-9]{4}-[0-9]{2}-[0-9]{2}" CHANGELOG.md; then
        echo "error: CHANGELOG.md has no dated section for {{version}} (F21 REQ-REL-08)." >&2
        exit 1
    fi
    if git rev-parse "v{{version}}" >/dev/null 2>&1; then
        echo "error: tag v{{version}} already exists." >&2
        exit 1
    fi
    just check
    git tag -a "v{{version}}" -m "$(just notes)"
    echo
    echo "Tagged v{{version}} locally. To publish (this triggers release.yml):"
    echo "    git push origin v{{version}}"
