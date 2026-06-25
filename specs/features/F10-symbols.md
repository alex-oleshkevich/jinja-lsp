# F10 — Symbols (Document & Workspace)

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Two views of the same extracted symbols — a hierarchical outline of the current template, and a fuzzy search for every macro and block across the whole workspace.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F08-go-to-definition](F08-go-to-definition.md), [F09-find-references](F09-find-references.md), [F12-folding-range](F12-folding-range.md)

> Requirement tag: **SYM**

---

## 1. Purpose & Scope

A template's structure — its blocks, its macros, its top-level variables — is already in the index. This spec surfaces that structure two ways: as the *outline* of the file you're editing, and as a *workspace-wide search box* where you type `post_url` and jump straight to it, wherever it lives.

This spec covers both LSP symbol requests:

- **`textDocument/documentSymbol`** — the hierarchical outline of the current template.
- **`workspace/symbol`** — fuzzy name search over every macro and block in the workspace.
- The `SymbolKind` mapping for each Jinja construct.
- The nesting rule for blocks and macros.

## 2. Non-Goals / Out of Scope

- Jumping to one definition from a usage — [F08-go-to-definition](F08-go-to-definition.md).
- Listing usages of a symbol — [F09-find-references](F09-find-references.md).
- Collapsing regions in the gutter — [F12-folding-range](F12-folding-range.md) (structural, but a different request).
- Documenting symbols — the registry and hints ([F02](F02-builtin-registry.md)/[F04](F04-user-hints.md)).

## 3. Background & Rationale

The outline view and the symbol-search palette are the two ways editors let you navigate *by structure* instead of by scrolling. Both read the same facts: the `BlockDefinition`, `MacroDefinition`, `VariableDefinition`, and import symbols Pass 1 already extracted ([E07](../foundations/E07-data-model.md)). The document outline reads one `TemplateIndex`; the workspace search reads the whole `WorkspaceIndex` ([E30](../foundations/E30-extraction-and-indexing.md)). Both handlers are pure reads ([E01](../foundations/E01-architecture.md) REQ-ARCH-07) — no new traversal, just shaping existing symbols into the LSP response.

## 4. Concepts & Definitions

- **Document symbol** — an entry in the current file's outline, possibly nesting children.
- **Workspace symbol** — a flat, searchable entry naming a macro or block anywhere in the workspace.
- **`SymbolKind`** — the LSP enum (`Module`, `Function`, `Variable`, `Namespace`, …) that drives the outline's icon.

## 5. Detailed Specification

### 5.1 Document outline — the symbol kinds

The outline mirrors the template's structure, mapping each Jinja construct to the `SymbolKind` that reads most naturally in an editor's outline pane.

**REQ-SYM-01 — Map each construct to a `SymbolKind`.**

`textDocument/documentSymbol` returns a `DocumentSymbol[]` for the current template:

| Jinja construct | `SymbolKind` | Detail shown |
|---|---|---|
| `{% block name %}` | `Module` | — |
| `{% macro name(params) %}` | `Function` | the parameter list |
| top-level `{% set x = … %}` | `Variable` | — |
| `{% import "…" as alias %}` / `{% from "…" import … %}` | `Namespace` | the source template path |

Each symbol carries its full range (the whole construct) and a selection range (the name), so editors can both reveal and highlight it.

> **Note:** Only **top-level** `{% set %}` variables appear — loop variables, macro parameters, and `{% set %}` inside a block are local detail, not outline-worthy. This keeps the outline a map of the file's *interface*, not its internals.

### 5.2 Document outline — nesting

Blocks and macros contain other constructs, and the outline should reflect that containment.

**REQ-SYM-02 — Nested constructs nest in the tree.**

A block or macro defined inside another block or macro becomes a *child* of the enclosing symbol in the `DocumentSymbol` tree. Nesting is determined by the tree-sitter span containment from Pass 1 — a symbol whose range falls inside another's range is its child. The outline can nest arbitrarily deep.

### 5.3 Workspace symbol search

The workspace search is the document outline's sibling, scaled to every file.

**REQ-SYM-03 — `workspace/symbol` fuzzy-searches every macro and block.**

`workspace/symbol` takes a query string and returns a `WorkspaceSymbol[]` of every **macro** and **block** in the workspace whose name fuzzy-matches the query. Each result carries the symbol name, its `SymbolKind` (per §5.1), a `Location` at its definition, and a `containerName` set to the template path so duplicate names across files stay distinguishable. An empty query returns all macros and blocks.

> **Note:** Workspace search is scoped to **macros and blocks** — the named, navigable, cross-file symbols. Top-level variables and imports are file-local detail and stay in the document outline only. This mirrors what [F09](F09-find-references.md) treats as workspace-resolvable.

### 5.4 Fuzzy matching

The query rarely matches exactly; it's a few letters of the name.

**REQ-SYM-04 — Match is fuzzy and ranked.**

Matching is subsequence-based and case-insensitive: `pu` matches `post_url`. Results are ranked so that tighter matches (contiguous, prefix, exact-case) sort first. Ranking is stable, so identical scores keep workspace order and tests are deterministic.

## 6. UI Mockups

### 6.1 Document outline — the `post.html` tree

The outline of `templates/blog/post.html`: a block at the top, with a nested macro and a top-level `set` underneath.

```
┌─ OUTLINE — templates/blog/post.html ─────────────────────────────────────┐
│                                                                            │
│  ⬚ content                              {% block content %}    [Module]   │
│     ƒ excerpt(post, words)              {% macro … %}          [Function]  │
│     𝑥 page_title                        {% set page_title %}   [Variable]  │
│  ◇ macros                               from "blog/macros.html" [Namespace]│
│                                                                            │
└───────────────────────────────────────────────────────────────────────────┘
  ⬚ Module   ƒ Function   𝑥 Variable   ◇ Namespace
```

### 6.2 Workspace symbol search — typing "pu"

The command palette filters every macro and block to fuzzy matches, each tagged with its template.

```
┌─ Go to Symbol in Workspace ──────────────────────────────────────────────┐
│  > pu                                                                      │
├───────────────────────────────────────────────────────────────────────────┤
│  ƒ  post_url            templates/blog/macros.html:1     [Function]        │
│  ƒ  pubdate             templates/blog/macros.html:14    [Function]        │
│  ⬚  pub_notice          templates/email/digest.html:3    [Module]          │
│                                                                            │
│  ⏎ open    ⇧⏎ open to the side    matches: name subsequence, ranked        │
└───────────────────────────────────────────────────────────────────────────┘
```

## 9. Examples & Use Cases

Opening `templates/blog/post.html` in `starlette-blog`, the outline shows the `content` block, any macros it nests, top-level `{% set %}` variables, and the `blog/macros.html` import as a namespace (§5.1, §5.2). Typing `comment` into the workspace symbol search surfaces `comment_card` from `blog/macros.html` even though that file isn't open (§5.3). Typing `pu` surfaces `post_url` via subsequence matching (§5.4).

## 10. Edge Cases & Failure Modes

- **A macro and a block with the same name in different files** → both appear in workspace search, disambiguated by `containerName` (§5.3).
- **A syntactically broken template** → the outline shows whatever Pass 1 could extract; missing constructs are simply absent, never an error (P3).
- **A deeply nested block-in-macro-in-block** → the tree nests to match (§5.2).
- **Empty workspace query** → returns all macros and blocks (§5.3); large workspaces stay within the latency budget by reading the prebuilt index.
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → their symbols appear in both views, with ranges in host-file coordinates.
- **A duplicate macro name in one file** ([F01](F01-diagnostics.md) `JINJA-W302`) → both definitions list in the outline; the diagnostic flags the duplication separately.

## 11. Testing

Symbols are verified by integration tests over fixture templates plus a `pytest-lsp` protocol journey for both requests.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-SYM-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Each construct maps to the right `SymbolKind` with detail | integration | starlette-blog | REQ-SYM-01 |
| Nested block/macro nests under its enclosing symbol | integration | starlette-blog | REQ-SYM-02 |
| Workspace search returns every macro & block with `containerName` | integration | starlette-blog | REQ-SYM-03 |
| Subsequence query (`pu` → `post_url`); ranking is stable | integration | starlette-blog | REQ-SYM-04 |
| Broken template yields a partial outline, no error | integration | syntax-errors | REQ-SYM-01 |

### 11.3 Fixtures

- `starlette-blog` for the outline and workspace search; `syntax-errors` for partial extraction. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-SYM-01 | symbol-kind mapping test |
| REQ-SYM-02 | nesting test |
| REQ-SYM-03 | workspace-search test |
| REQ-SYM-04 | fuzzy-match + ranking test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of both symbol requests**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `documentSymbol` on `post.html` | happy | the nested outline with correct kinds |
| E2E-02 | `workspace/symbol` with `"pu"` | happy | `post_url` among the fuzzy matches |
| E2E-03 | `workspace/symbol` with an empty query | happy | all macros and blocks returned |
| E2E-04 | `documentSymbol` on a broken file | error path | a partial outline, no crash |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; symbols are read from the index only, never by executing templates (P1).
- **Data sensitivity** — symbol names and locations point only into the user's own workspace; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the outline pane and the symbol palette.

### 13.4 Performance & Scale

- **Latency** — the document outline reads one `TemplateIndex` and workspace search reads the prebuilt `WorkspaceIndex`; both return in < 100 ms (P6). The index itself is built within the 2 s budget ([E30](../foundations/E30-extraction-and-indexing.md)).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P3, P6; [E07-data-model](../foundations/E07-data-model.md) — the symbol types shaped into responses; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — the workspace index; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F08-go-to-definition](F08-go-to-definition.md) — jumping to a symbol; [F09-find-references](F09-find-references.md) — the same workspace-resolvable symbols, as usages; [F12-folding-range](F12-folding-range.md) — the file's structure as collapsible regions.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-24** — Outline example names the `blog/macros.html` import (a `from`-import shown as a `Namespace`, §5.1), not an alias namespace, matching how `post.html` imports.
