# ADR-002 — Use the upstream tree-sitter Jinja grammar

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The server's analysis stands entirely on its parser (P1 — static analysis only). We build on `alex-oleshkevich/tree-sitter-jinja`, a two-grammar design: a block grammar for `{% %}`/`{{ }}` structure and an inline grammar for the expression sub-language inside the delimiters ([E31](../foundations/E31-inline-templates.md)). jinja-lsp authors its own `.scm` query files against that grammar, and those queries reference its node-type names. We depend on the upstream grammar directly: it already ships both cooperating grammars, and pinning it by revision keeps builds reproducible. The one risk that buys us — an upstream release renaming a node type and silently breaking a query — is mitigated not by maintaining our own copy of the grammar but by per-query fixture tests that fail the moment a capture stops matching.

## Decision

We depend on `alex-oleshkevich/tree-sitter-jinja` upstream directly, pinned by revision (git dependency). Both the block grammar and the inline/expression grammar come from upstream. We author the `.scm` queries against its node types.

## Consequences

There is no grammar to maintain. The grammar is pinned by revision, so a build is reproducible and a grammar bump is a deliberate, reviewable change ([E03](../foundations/E03-tech-stack.md)). Node-type drift on an upstream bump is caught by the per-query fixture tests ([E30](../foundations/E30-extraction-and-indexing.md), REQ-EXTR-02): a query that stops capturing fails its fixture, so a breaking rename surfaces at bump time rather than silently at runtime. When we genuinely need a grammar change, the path is a PR upstream or a pinned-rev bump — not a private divergence to keep building. The prerequisite work — verifying the 17 query files against the upstream node types — happens before any server code.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Maintain our own copy of the grammar | Extra maintenance burden of keeping a private copy building and pulling upstream changes, for control over node types we don't need — the per-query fixture tests already catch drift on a pinned-rev bump. |
| Write a grammar from scratch | Months of work to re-derive what the upstream grammar already provides; the two-grammar split is exactly what we need. |
| Hand-rolled / regex parser | Can't satisfy P3 (never corrupt, graceful partial-parse recovery) or P1's tree-based analysis; tree-sitter's error recovery is the whole point. |

## Changelog

- **2026-06-24** — Created.
