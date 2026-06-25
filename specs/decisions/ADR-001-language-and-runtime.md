# ADR-001 — Language and runtime: Rust

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The server has to ship as something a user can install once and forget, work identically in every editor, and start instantly on every keystroke-driven request. A Python LSP drags a runtime and a dependency tree behind it — version skew, virtualenv friction, slow cold starts — none of which a latency-sensitive editor companion can afford (P6). The sibling projects in the same ecosystem (sqlalchemy-lsp) settled this question in Rust.

## Decision

We build jinja-lsp in Rust. It compiles to a single self-contained binary with no runtime dependency, distributed as a cross-compiled executable per platform.

## Consequences

A user installs one file — `cargo install jinja-lsp` or a download from a GitHub release ([F21](../features/F21-release-ci.md)) — with no interpreter, no virtualenv, and no transitive packages to resolve. Startup is immediate, which is what P6's latency budgets require. The whole engine (extraction, indexing, diagnostics) is one codebase shared by the `lsp`, `check`, and `format` front-ends, so they cannot drift. The cost is that contributors need Rust fluency, compile times are longer than an interpreted language's, and the one place we genuinely need Python — the LSP-protocol E2E harness ([E29](../foundations/E29-e2e-testing.md) Branch B) — is confined to `tests/e2e/` rather than being the implementation language.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Python (pygls) | Runtime dependency, virtualenv friction, slower cold start; fails P6's latency posture for a per-keystroke tool. |
| Go | Single binary too, but the ecosystem (sqlalchemy-lsp) is Rust, and the mature tree-sitter bindings the parsing core needs are first-class in Rust. |
| TypeScript / Node | Bundles a runtime and is weaker for the CPU-bound parsing/indexing core. |

## Changelog

- **2026-06-24** — Created.
