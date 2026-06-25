# ADR-010 — PyPI distribution via maturin wheels

> **Status:** Accepted
>
> **Date:** 2026-06-25
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The product is a single self-contained Rust binary ([ADR-001](ADR-001-language-and-runtime.md)). It already reaches users three ways: `cargo install` (Rust developers), direct binary downloads from a GitHub release, and the VS Code marketplace ([F21](../features/F21-release-ci.md)). But the people who write Jinja templates mostly live in Python projects — Flask, Starlette, FastAPI — and on those machines `pip` and `uv` are always present while a Rust toolchain usually is not. For that audience, `cargo install jinja-lsp` means "first install Rust," and a raw binary download means "pick the right target, put it on PATH, keep it updated by hand." Both are friction the rest of their tooling (ruff, for one) has already removed by shipping on PyPI.

The question is how to put `jinja-lsp` one `pip install` away without turning a Rust binary into a Python program.

## Decision

We publish jinja-lsp to PyPI as **platform wheels built with maturin**. A root `pyproject.toml` declares `build-backend = "maturin"` and `[tool.maturin] binaries = ["jinja-lsp"]`; maturin packages the prebuilt binary for each target into a wheel whose entry point *is* the binary. `pip install jinja-lsp` or `uv tool install jinja-lsp` then drops `jinja-lsp` on PATH with no Rust toolchain and no compile step. The wheel carries no Python code and adds no runtime Python dependency — pip/uv is purely the delivery vehicle for the same self-contained binary every other channel ships.

PyPI is one more channel published from the same release tag as crates.io, the marketplace, and GitHub releases ([F21](../features/F21-release-ci.md)).

## Consequences

`maturin` becomes a release-time build dependency, and a `pyproject.toml` sits at the repo root beside `Cargo.toml`. CI gains a `build-wheels` matrix over the same target triples as the binary build (`PyO3/maturin-action`, manylinux 2_28) and a `publish-pypi` job that uploads via OIDC trusted publishing (`uv publish`), so no long-lived PyPI token is stored. maturin reads the package version dynamically from `Cargo.toml`, so the wheel version cannot drift from the crate version — the existing tag↔`Cargo.toml` gate ([F21](../features/F21-release-ci.md) REQ-REL-09) covers PyPI for free, and crates.io and PyPI can never disagree on a number. The binary stays exactly what it was ([ADR-001](ADR-001-language-and-runtime.md)): static analysis only, no network, no runtime Python. The cost is one more registry identity to manage and PyPI's immutability — a botched upload bumps PATCH rather than republishing, the same discipline crates.io already imposes.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| A pure-Python wrapper that downloads the binary at install time | Needs network at install, checksum handling, and breaks offline/locked-down installs. maturin wheels ship the binary *inside* the wheel — nothing to fetch. |
| `cargo install` + GitHub downloads only | Leaves the Python-native audience — who already have pip/uv — either installing Rust or hand-placing a binary. |
| conda-forge or a Homebrew tap | Narrower reach than PyPI for this audience, and more recipe maintenance. A Homebrew tap remains an open question in [F21](../features/F21-release-ci.md) and can be added later without affecting this decision. |
| Build wheels from source on the user's machine | Reintroduces the Rust-toolchain requirement at install time — the exact friction this decision removes. |

## Changelog

- **2026-06-25** — Created.
