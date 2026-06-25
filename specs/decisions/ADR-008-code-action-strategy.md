# ADR-008 — Code actions derived from the diagnostic catalog

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

Code actions are a designed feature of jinja-lsp ([F17](../features/F17-code-actions.md)). The risk with a blank slate is inventing actions ad hoc, ending up with an inconsistent grab-bag where some diagnostics have fixes and others don't for no principled reason. We already have a complete, well-structured diagnostic catalog ([F01](../features/F01-diagnostics.md)) — every finding the server can produce, with its code, slug, and range. That catalog is the natural spine for quick-fixes: each diagnostic class suggests its own remedy.

## Decision

We derive quick-fixes mechanically from the F01 diagnostic catalog — each fix is tied to a diagnostic code (remove an unused import for `W203`, "did you mean…?" for `E102`/`E104`, insert a stub for `E403`, create the file for `E601`, and so on) — and add a small set of cursor/selection-triggered refactors (extract-to-macro, wrap-in-block/if/for) that aren't diagnostic-bound. All edits are applied via `WorkspaceEdit`; refactors needing follow-up input use `workspace/executeCommand`.

## Consequences

Coverage is principled and discoverable: if a diagnostic exists, you can ask what fixes it, and the answer is in one place ([F17](../features/F17-code-actions.md) maps the catalog). This re-enables the `workspace/applyEdit` and `executeCommand` capabilities the server declares ([E01](../foundations/E01-architecture.md)). Because fixes are catalog-driven, adding a new diagnostic naturally prompts the question "what's its quick-fix?", keeping the two in step over time. The refactors are the deliberate exception — they're triggered by cursor/selection, not a finding, so they don't fit the catalog spine and are specified separately. The cost is that some diagnostics have no sensible automatic fix (a `W402 unreachable-content` is a design issue, not a mechanical edit), so coverage is "every diagnostic that *can* have a fix," not literally all of them — and edits must respect P3's round-trip safety, sharing the same `edit/` builders as the formatter ([ADR-007](../decisions/ADR-007-formatting-strategy.md)).

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| No code actions | Leaves obvious, mechanical fixes (remove unused import, create missing template) on the table. |
| Ad-hoc actions invented per feature | Inconsistent coverage with no principle for which diagnostics get fixes; hard to keep in step. |
| Refactors only, no diagnostic quick-fixes | Skips the highest-value, lowest-effort wins — the one-click fixes for findings the user is already looking at. |

## Changelog

- **2026-06-24** — Created.
