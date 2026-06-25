# E03 — Tech Stack

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** The pinned languages, crates, and tools jinja-lsp is built on — what we depend on, which version, and why — including the upstream tree-sitter grammar and the one place Python is allowed.

> **Depends on:** [constitution](../constitution.md)   ·   **Related:** [E01-architecture](E01-architecture.md), [E02-folder-structure](E02-folder-structure.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), [E29-e2e-testing](E29-e2e-testing.md)

> Requirement tag: **STACK**

---

## 1. Purpose & Scope

This spec is the dependency contract. It pins the language edition, the core crates, the test toolchain, and the license, so every other spec can name a tool without re-justifying it and a build stays reproducible.

This spec covers:

- The language: Rust edition and MSRV.
- The core runtime crates and their versions.
- The upstream tree-sitter grammar (block + inline).
- The Rust test toolchain and the confined Python e2e toolchain.
- The license.

## 2. Non-Goals / Out of Scope

- How the code is organized into modules — owned by [E02-folder-structure](E02-folder-structure.md).
- The runtime architecture the crates serve — owned by [E01-architecture](E01-architecture.md).
- How the test tools are used — owned by [E17-testing](E17-testing.md) and [E29-e2e-testing](E29-e2e-testing.md).

## 3. Background & Rationale

The sibling project sqlalchemy-lsp settled this stack already, and reusing it buys us a known-good combination of `tower-lsp`, `tokio`, and `tree-sitter` plus the same release tooling. The one project-specific choice is the grammar: jinja-lsp depends on `alex-oleshkevich/tree-sitter-jinja` upstream directly, pinned by revision, and authors its own `.scm` queries against it (ADR-002).

## 4. Concepts & Definitions

- **MSRV** — Minimum Supported Rust Version; the oldest toolchain the crate compiles on.
- **Block grammar / inline grammar** — the two tree-sitter grammars the upstream package ships; see [E31](E31-inline-templates.md).
- **`.scm` query** — a tree-sitter S-expression query that captures named nodes; see [E30](E30-extraction-and-indexing.md).

## 5. Detailed Specification

### 5.1 Language

jinja-lsp is written in modern Rust, pinned to a recent edition with a stated floor.

**REQ-STACK-01 — Rust edition 2024, MSRV 1.85.**

The crate targets Rust **edition 2024** with a **minimum supported Rust version of 1.85**. CI builds against the MSRV and against stable to catch accidental use of newer features.

### 5.2 Core runtime crates

These crates are the runtime spine — the LSP framework, the async runtime, the parser, and the serialization and CLI plumbing.

**REQ-STACK-02 — Core dependencies are pinned.**

| Crate | Version | Role |
|---|---|---|
| `tower-lsp` | `0.20` | LSP server framework; stdio transport ([E01](E01-architecture.md)) |
| `tree-sitter` | `0.26` | Incremental parser runtime ([E30](E30-extraction-and-indexing.md)) |
| `tokio` | `1` | Async runtime for the server and `spawn_blocking` parse jobs |
| `serde` + `serde_json` | latest `1` | Serialization; JSON for LSP payloads and `check --format json` |
| `serde_yaml` | latest | **Only** for parsing YAML frontmatter in built-in doc files ([F02](../features/F02-builtin-registry.md)) — **not** for config |
| `toml` | latest | Config parsing ([E15](E15-app-config.md)); config is TOML-only |
| `tracing` | latest | Structured logging / spans on slow paths ([E16](E16-conventions.md)) |
| `clap` | latest `4` | CLI argument parsing for the three subcommands ([E01](E01-architecture.md)) |

> **Note:** `serde_yaml` exists in the tree for exactly one reason — the YAML frontmatter inside the embedded `.md` built-in docs. Configuration is TOML only ([E15](E15-app-config.md)); do not reach for YAML there.

### 5.3 The tree-sitter grammar

The parser is the heart of a static-analysis LSP. jinja-lsp depends on an upstream grammar that ships two cooperating grammars.

**REQ-STACK-03 — Upstream grammar, block + inline.**

jinja-lsp depends on **`alex-oleshkevich/tree-sitter-jinja`** upstream, pinned by revision (ADR-002). It ships **two grammars**:

- The **block grammar** parses template structure — tags, blocks, comments, and the `{{ }}`/`{% %}` delimiters.
- The **inline (expression) grammar** parses the sub-language *inside* the delimiters — attribute access, filters, tests, and function calls. It is what makes `{{ post.title | upper }}` analyzable down to the attribute and filter. It is also the grammar used for standalone inline templates ([E31](E31-inline-templates.md)).

**REQ-STACK-04 — Queries are authored against the upstream grammar.**

jinja-lsp authors its own 17 `.scm` query files against the upstream grammar. The node-type names in those queries must stay in sync with the pinned grammar revision, which extraction depends on ([E30](E30-extraction-and-indexing.md)). Treat a mismatch as a bug in the queries, not in the grammar.

### 5.4 Test toolchain

Tests split along a language line: everything except the LSP-protocol e2e is Rust; that one branch is Python, and it is fenced off.

**REQ-STACK-05 — Rust test tooling.**

The Rust test toolchain is `cargo nextest` as the runner and `insta` for snapshot tests (the formatter golden tests and the rich/compact report snapshots). See [E17-testing](E17-testing.md).

**REQ-STACK-06 — Python e2e tooling is confined to `tests/e2e/`.**

The LSP-protocol e2e suite ([E29](E29-e2e-testing.md) Branch B) is the **only** Python in the repository. It uses `pytest`, `pytest-lsp`, and `lsprotocol` to drive the real `jinja-lsp lsp` stdio binary. These dependencies live exclusively under `tests/e2e/` (with their own `pyproject.toml` / `requirements`), never in the crate's `Cargo.toml`. We do not hand-roll an LSP client harness — `pytest-lsp` handles capability negotiation and JSON-RPC plumbing.

### 5.5 License

**REQ-STACK-07 — MIT license.**

jinja-lsp is MIT licensed.

## 8. Data Shapes

The dependency floor, as it appears in `Cargo.toml`. This is the contract a fresh checkout builds against:

```toml
# Cargo.toml
[package]
name = "jinja-lsp"
edition = "2024"
rust-version = "1.85"
license = "MIT"

[dependencies]
tower-lsp = "0.20"
tree-sitter = "0.26"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"          # built-in doc frontmatter ONLY (F02)
toml = "0.8"               # config (E15)
tracing = "0.1"
clap = { version = "4", features = ["derive"] }
tree-sitter-jinja = { git = "https://github.com/alex-oleshkevich/tree-sitter-jinja", rev = "<pinned-rev>" }  # upstream: block + inline (ADR-002)

[dev-dependencies]
insta = "1"                # snapshot tests (E17)
# cargo nextest is a runner, installed separately, not a dependency
```

A second manifest at the repo root, the maturin `pyproject.toml`, packages the built binary into PyPI wheels at release time — `maturin` is a release-time build tool, not a crate dependency, and the wheel carries no Python runtime ([F21](../features/F21-release-ci.md), [ADR-010](../decisions/ADR-010-pypi-distribution.md)).

The Python e2e deps live apart from the crate, under `tests/e2e/`:

```toml
# tests/e2e/pyproject.toml
[project]
name = "jinja-lsp-e2e"
dependencies = ["pytest", "pytest-lsp", "lsprotocol"]
```

## 10. Edge Cases & Failure Modes

- **Grammar node-type drift** → an `.scm` query captures nothing; extraction for that construct silently returns empty. Caught by the per-query extraction tests in [E30](E30-extraction-and-indexing.md), not at runtime (P3).
- **MSRV breakage** → a dependency bumps its own MSRV above 1.85; pin that dependency or raise our MSRV deliberately with a changelog entry.
- **Python deps leaking into the crate** → forbidden; the e2e `pyproject.toml` is the only manifest that may name them.

## 11. Testing

This foundation is verified by the build itself plus a small MSRV/license check in CI.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior is covered.** Every `REQ-STACK-NN` maps to at least one check. See the policy in [E17-testing](E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Crate compiles on MSRV 1.85 and on stable | CI build | — | REQ-STACK-01 |
| Each `.scm` query captures against the upstream grammar | unit | [starlette-blog](E17-testing.md#starlette-blog) | REQ-STACK-03, REQ-STACK-04 |
| No Python dependency appears in the crate `Cargo.toml` | CI lint | — | REQ-STACK-06 |
| `LICENSE` is MIT | CI lint | — | REQ-STACK-07 |

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-STACK-01 | MSRV + stable CI build |
| REQ-STACK-02 | build resolves the pinned versions |
| REQ-STACK-03 | per-query extraction test on the upstream grammar |
| REQ-STACK-04 | per-query extraction test (drift detection) |
| REQ-STACK-05 | nextest runs the suite |
| REQ-STACK-06 | crate-manifest Python-leak lint |
| REQ-STACK-07 | license lint |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — dependencies come from crates.io and the pinned upstream grammar (git rev); no network access at runtime (P1, stdio only).
- **Input & validation** — `tree-sitter` parsing is memory-safe and never executes template content.
- **Data sensitivity** — none; the supply chain is the only surface, mitigated by pinned versions and `cargo audit` in CI ([F21](../features/F21-release-ci.md)).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1 (static analysis), P6 (performance) the stack must serve.
- **Related:** [E01-architecture](E01-architecture.md) — how `tower-lsp`/`tokio` are used; [E02-folder-structure](E02-folder-structure.md) — where each crate is used; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — the `.scm` queries and grammar; [E31-inline-templates](E31-inline-templates.md) — the inline grammar; [E29-e2e-testing](E29-e2e-testing.md) — the Python e2e deps; [F02-builtin-registry](../features/F02-builtin-registry.md) — the `serde_yaml` frontmatter use.

## 17. Changelog

- **2026-06-24** — Initial draft: Rust 2024 / MSRV 1.85, core crates, the upstream block+inline grammar, the Rust and confined Python test toolchains, MIT license.
