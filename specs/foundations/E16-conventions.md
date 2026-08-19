# E16 — Engineering Conventions

> **Status:** Approved
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** The code-level rules every module follows — partial-parse recovery, the never-panic discipline, the error-type taxonomy, and where `tracing` spans go.

> **Depends on:** [constitution](../constitution.md), [E01-architecture](E01-architecture.md)   ·   **Related:** [E03-tech-stack](E03-tech-stack.md), [E15-app-config](E15-app-config.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md)

> Requirement tag: **CONV**

---

## 1. Purpose & Scope

This spec turns the constitution's "never corrupt, never panic" and "degrade, don't fail" principles into concrete code rules. It defines how broken input is recovered, how errors are typed and handled, and where observability lives — the conventions that keep jinja-lsp robust against the half-typed templates it sees every keystroke.

This spec covers:

- Partial-parse recovery.
- The never-panic discipline.
- The error-type taxonomy (`ConfigError`, `PackError`, `SyntaxError`).
- `tracing` spans on slow paths.

## 2. Non-Goals / Out of Scope

- The architecture these conventions support — owned by [E01-architecture](E01-architecture.md).
- The crates that provide them (`tracing`, `tree-sitter`) — owned by [E03-tech-stack](E03-tech-stack.md).
- Config-specific validation rules — owned by [E15-app-config](E15-app-config.md).

## 3. Background & Rationale

An LSP analyzes code that is, by definition, usually in mid-edit. Half a tag is typed, a quote is unclosed, a `{% for %}` has no `{% endfor %}` yet. If the server panics or refuses to answer on any of that, it's useless exactly when the user needs it. So the whole codebase is built to *degrade*: tree-sitter always hands back a tree, extraction takes what it can from that tree, and anything it can't read is logged and skipped — never fatal. This is principle P3 made operational.

## 4. Concepts & Definitions

- **Partial parse** — a tree-sitter result over incomplete or invalid source, containing `ERROR`/`MISSING` nodes.
- **Recoverable error** — an error that is logged and worked around, not propagated to a panic.
- **Slow path** — an operation expensive enough to warrant a `tracing` span (workspace rebuild, large-file parse, config reload).

## 5. Detailed Specification

### 5.1 Partial-parse recovery

tree-sitter never refuses to parse — it produces a tree even for broken input. We lean on that fully.

**REQ-CONV-01 — Extraction continues past parse errors.**

tree-sitter always yields a tree, even for half-typed templates. Missing and error nodes surface as syntax errors (`JINJA-E001`), but extraction does **not** stop there: every query that *can* match a well-formed subtree still runs, so a single unclosed tag at the bottom of a file never blanks out the symbols above it. A broken file yields *partial* facts, not *no* facts.

### 5.2 The never-panic discipline

A panic in a single file's extraction must not take down the server or the whole index.

**REQ-CONV-02 — Symbol extraction is fallible and never panics.**

Every extraction step returns a `Result` and is allowed to fail for one node without aborting the file. A failure is logged at `warn` and that one node is skipped; the rest of the file extracts normally. No extraction path may `unwrap()` on untrusted parse data, index into a slice without bounds checking, or otherwise panic on input. This realizes P3.

> **Warning:** A panic anywhere in the analysis pipeline is a bug, not a degradation. Recover at the smallest scope that keeps the rest of the work alive — one node, one file — and log it.

### 5.3 The error-type taxonomy

Errors are typed by where they come from, so callers can decide what's fatal and what's a log line. **Nothing here aborts the server** — that invariant is the point of this section, not the specific type count.

**REQ-CONV-03 — Three error types, none fatal.**

| Type | Defined in | Raised by | Severity |
|---|---|---|---|
| `ConfigError` | `src/config.rs` | config parsing/validation ([E15](E15-app-config.md)) | reported to the user; previous valid config retained |
| `PackError` | `src/builtins/packs.rs` | loading a framework extension pack | logged; the pack is skipped, the rest of the registry loads |
| `SyntaxError` | `src/workspace/symbols.rs` | the parser, per unparseable region | recorded as `JINJA-E001`; extraction continues past it |

`ConfigError` is the most visible (it surfaces as a workspace diagnostic), but even it degrades — the session keeps its last good config ([E15](E15-app-config.md#56-live-config-reload)).

**Why not the four types this spec used to name** (jinja-lsp-pvq3). `ParseError`, `ExtractionError`, `ConfigError` and `DiagnosticError` were once written as `src/error.rs`, then deleted as dead code: nothing outside that module ever referenced them, they implemented neither `Display` nor `std::error::Error`, and `ConfigError` duplicated the real one in `config.rs`. Two of the four could not exist as written — tree-sitter always returns a tree, so there is no "no tree at all" failure for `ParseError` to describe, and the checks are pure functions over an index that return `Vec<Diagnostic>` and have no failure mode for `DiagnosticError` to carry. The recovery *behaviour* those types were meant to guarantee is still required and still tested: a bad node is skipped rather than propagated (REQ-CONV-02), and a syntax error suppresses only the other checks for that file (F01 §10), by construction rather than by error type.

### 5.4 Observability

Slow operations get a span so a `tracing` consumer can see where time goes, without paying for instrumentation on the hot, cheap paths.

**REQ-CONV-04 — `tracing` spans wrap slow paths.**

The expensive operations carry a `tracing` span: the workspace index rebuild (Pass 2), large-file parsing, and config reload. Per-keystroke single-file extraction (Pass 1) is cheap and is not spanned, to keep the common path quiet. All `tracing` output goes to stderr or a file — **never stdout**, which carries JSON-RPC ([E01](E01-architecture.md#51-process-model--front-ends)). There is no metrics backend (constitution §4.6).

## 8. Data Shapes

There is deliberately **no** unifying `JinjaError` enum and no `src/error.rs`. Each error type lives beside the code that raises it, because nothing ever needs to handle two of them at one call site:

```rust
crate::config::ConfigError            // bad config     -> workspace diagnostic, retain prior
crate::builtins::packs::PackError     // bad pack       -> log, skip that pack
crate::workspace::symbols::SyntaxError // unparseable region -> record JINJA-E001, keep extracting
```

A wrapper enum would exist only to be immediately matched back apart. Recovery for everything else is structural rather than typed — an extraction step that cannot make sense of a node skips it and returns the symbols it did find.

## 10. Edge Cases & Failure Modes

- **Unclosed tag mid-file** → `JINJA-E001` recorded; symbols before it still extract (REQ-CONV-01).
- **A query matches an unexpected node shape** → the node is skipped and extraction continues; no error value propagates (REQ-CONV-02).
- **A pack fails to load** → `PackError`, logged; that pack's builtins are absent, every other registry entry still loads (REQ-CONV-03).
- **`tracing` misconfigured to stdout** → forbidden; it would corrupt the JSON-RPC stream (REQ-CONV-04).

## 11. Testing

This foundation is verified by fault-injection unit tests over deliberately broken fixtures.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior is covered.** Every `REQ-CONV-NN` maps to at least one test. See the policy in [E17-testing](E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Broken file still yields the symbols before the error | unit | [syntax-errors](E17-testing.md#syntax-errors) | REQ-CONV-01 |
| Malformed node is skipped, never panics | unit | [syntax-errors](E17-testing.md#syntax-errors) | REQ-CONV-02 |
| A failing check doesn't suppress the others | unit | [syntax-errors](E17-testing.md#syntax-errors) | REQ-CONV-03 |
| Pass 2 emits a `tracing` span; stdout stays clean | integration | [large-workspace](E17-testing.md#large-workspace) | REQ-CONV-04 |

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-CONV-01 | partial-extraction test |
| REQ-CONV-02 | fault-injection no-panic test |
| REQ-CONV-03 | isolated-check-failure test |
| REQ-CONV-04 | span-emission + stdout-cleanliness test |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — conventions only; no trust boundary introduced. Recovery never executes recovered content (P1).
- **Input & validation** — all parse input is untrusted; the never-panic discipline is the defense against malformed input.
- **Data sensitivity** — `tracing` output may include template source spans; it goes to stderr/file under the user's control, never the network.

### 13.5 Observability

- **Logs / traces** — `tracing` spans on Pass 2, large-file parse, and config reload; recoverable errors logged at `warn`. No metrics backend.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P3, "degrade, don't fail"; [E01-architecture](E01-architecture.md) — the passes these conventions guard.
- **Related:** [E03-tech-stack](E03-tech-stack.md) — `tracing` and `tree-sitter`; [E15-app-config](E15-app-config.md) — `ConfigError` handling; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — where extraction recovery happens.

## 17. Changelog
- **2026-06-26** — Status: Draft → Approved.

- **2026-06-24** — Initial draft: partial-parse recovery, the never-panic discipline, the four-type error taxonomy, and `tracing` spans on slow paths.
