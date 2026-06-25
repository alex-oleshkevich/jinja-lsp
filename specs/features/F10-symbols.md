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

Each row names the construct or condition, the fixture (or `synthetic doc` — an in-memory `didOpen` for an edge not in a fixture), and the exact expected outcome. Happy rows establish each behavior; negative/edge rows cover the §10 edges and §6 states.

**Document outline — `SymbolKind` mapping (REQ-SYM-01)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| `{% block content %}` → `Module`, no detail | integration | starlette-blog (`blog/post.html`) | symbol `content`, kind `Module`, detail empty | REQ-SYM-01 |
| `{% macro post_url(post) %}` → `Function`, param-list detail | integration | starlette-blog (`blog/macros.html`) | symbol `post_url`, kind `Function`, detail `(post)` | REQ-SYM-01 |
| `{% macro comment_card(comment, show_actions=true) %}` → `Function`, detail shows keyword default | integration | starlette-blog (`blog/macros.html`) | detail `(comment, show_actions=true)` | REQ-SYM-01 |
| top-level `{% set page_title = … %}` → `Variable` | integration | synthetic doc (top-level `set` in a template) | symbol `page_title`, kind `Variable`, no detail | REQ-SYM-01 |
| `{% from "blog/macros.html" import post_url %}` → `Namespace`, source-path detail | integration | starlette-blog (`email/digest.html`) | symbol for the import, kind `Namespace`, detail `blog/macros.html` | REQ-SYM-01 |
| `{% import "blog/macros.html" as macros %}` (alias slot) → `Namespace`, source-path detail | integration | synthetic doc (alias `import`) | symbol `macros`, kind `Namespace`, detail `blog/macros.html` | REQ-SYM-01 |
| Each symbol carries full range (whole construct) and selection range (the name) | integration | starlette-blog (`blog/macros.html`) | `range` spans the tag; `selectionRange` spans the name only | REQ-SYM-01 |
| **Negative:** loop variable (`{% for c in … %}`) is **not** an outline symbol | integration | starlette-blog (`blog/post.html`) | no symbol named `c` | REQ-SYM-01 |
| **Negative:** macro parameter is **not** a top-level symbol | integration | starlette-blog (`blog/macros.html`) | `post`/`comment` appear only as detail, not as sibling symbols | REQ-SYM-01 |
| **Negative:** `{% set %}` inside a block is **not** outline-worthy | integration | synthetic doc (`set` nested in a block) | no `Variable` symbol for the in-block `set` | REQ-SYM-01 |
| **Edge (§10):** syntactically broken template yields a partial outline, never an error (P3) | integration | syntax-errors | symbols Pass 1 extracted are returned; missing constructs simply absent; no error response | REQ-SYM-01 |
| **Edge (§10, E31):** inline-template symbols appear with ranges in host-file coordinates | integration | call-and-paths (inline/E31 case) | inline macro/block symbols present, ranges in host file | REQ-SYM-01 |

**Document outline — nesting (REQ-SYM-02)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Macro defined inside a block nests as that block's child | integration | starlette-blog (`blog/post.html`) | macro symbol is a `children` entry of the `content` `Module` | REQ-SYM-02 |
| **Edge (§10):** deeply nested block-in-macro-in-block nests to match | integration | synthetic doc (block ▸ macro ▸ block) | three-level `children` chain mirrors span containment | REQ-SYM-02 |
| **Edge (§10, W302):** duplicate macro name in one file lists both definitions in the outline | integration | duplicates | two sibling symbols with the same name; the `W302` diagnostic is separate (F01) | REQ-SYM-02 |
| **Negative:** sibling (non-contained) constructs do **not** nest | integration | starlette-blog (`blog/macros.html`) | `post_url` and `comment_card` are siblings, neither a child of the other | REQ-SYM-02 |

**Workspace symbol search (REQ-SYM-03)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Search returns every macro **and** block across files, each with kind and `containerName` | integration | starlette-blog | results include `post_url`/`comment_card` (`Function`) and `content`/`head`/`body`/`footer` (`Module`); each `containerName` is its template path; each `Location` at the definition | REQ-SYM-03 |
| **Edge (§10):** empty query returns all macros and blocks | integration | starlette-blog | every macro and block in the workspace returned | REQ-SYM-03 |
| **Edge (§10):** same name as a macro in one file and a block in another — both appear, disambiguated by `containerName` | integration | synthetic doc + starlette-blog | two results sharing the name, different `containerName`/kind | REQ-SYM-03 |
| **Edge (§10, E31):** inline-template macro/block appears in workspace search with host-file `Location` | integration | call-and-paths (inline/E31 case) | inline symbol present, `Location` in host file | REQ-SYM-03 |
| **Negative:** top-level variables are **not** in workspace results | integration | synthetic doc (top-level `set`) | the `set` variable name is absent from results | REQ-SYM-03 |
| **Negative:** imports are **not** in workspace results | integration | starlette-blog (`email/digest.html`) | the `from`-import is absent from results | REQ-SYM-03 |
| **Edge (§10):** large workspace stays within latency budget by reading the prebuilt index | integration | large-workspace | empty-query result returns within the budget (< 100 ms, P6) | REQ-SYM-03 |

**Fuzzy matching & ranking (REQ-SYM-04)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Subsequence match: `pu` matches `post_url` | integration | starlette-blog | `post_url` is among the results for `pu` | REQ-SYM-04 |
| Case-insensitive: `PU` / `Post` match `post_url` | integration | starlette-blog | `post_url` present for the differing-case query | REQ-SYM-04 |
| Ranking: tighter matches (contiguous / prefix / exact-case) sort before looser ones | integration | starlette-blog | for `pu`, a prefix/contiguous match ranks above a scattered subsequence match | REQ-SYM-04 |
| Ranking is stable: equal scores keep workspace order (deterministic) | integration | starlette-blog | repeated runs return identical order for equal-scored results | REQ-SYM-04 |
| **Negative:** a query that is not a subsequence of any name returns no matches | integration | starlette-blog | query `zzz` returns an empty result set | REQ-SYM-04 |

### 11.3 Fixtures

- `starlette-blog` for the outline, mapping, workspace search, and fuzzy ranking; `syntax-errors` for the partial-outline edge; `duplicates` for the same-name-in-one-file outline edge; `call-and-paths` for the inline/E31 cases; `large-workspace` for the empty-query latency budget. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry). Edge constructs with no fixture (top-level/in-block `set`, alias `import`, deep block▸macro▸block nesting, cross-file same-name) use synthetic in-memory `didOpen` documents.

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-SYM-01 | §11.2 *Document outline — `SymbolKind` mapping* rows (block/macro/macro-with-default/set/from-import/alias-import kinds, range + selectionRange, the three negative excludes, and the broken-template and inline/E31 edges) |
| REQ-SYM-02 | §11.2 *Document outline — nesting* rows (child nesting, deep block▸macro▸block, duplicate-name-in-file, non-contained-sibling negative) |
| REQ-SYM-03 | §11.2 *Workspace symbol search* rows (all macros + blocks with `containerName`, empty query, cross-file same-name, inline/E31, variable & import negatives, large-workspace latency) |
| REQ-SYM-04 | §11.2 *Fuzzy matching & ranking* rows (subsequence, case-insensitive, ranking order, stable order, no-match negative) |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of both symbol requests**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `documentSymbol` on `blog/post.html` | happy | the `content` `Module` with its nested macro child and correct kinds/details |
| E2E-02 | `documentSymbol` on `blog/macros.html` — macro kinds & details | happy | `post_url` and `comment_card` as sibling `Function`s with param-list details, neither nested under the other |
| E2E-03 | `documentSymbol` on `email/digest.html` — import as `Namespace` | happy | the `from "blog/macros.html"` import appears as a `Namespace` with the source-path detail |
| E2E-04 | `documentSymbol` outline excludes locals | happy | no symbol for the `{% for c … %}` loop variable or macro parameters |
| E2E-05 | `workspace/symbol` with `"pu"` | happy | `post_url` among the fuzzy matches, tagged `Function` with its `containerName` |
| E2E-06 | `workspace/symbol` with an empty query | happy | every macro and block (`post_url`, `comment_card`, `content`, `head`, `body`, `footer`) returned, none of the variables or imports |
| E2E-07 | `workspace/symbol` ranking — `"pu"` orders tighter matches first | happy | results come back in stable, score-ranked order |
| E2E-08 | `documentSymbol` on a broken file | error path | a partial outline from what Pass 1 extracted, no crash |
| E2E-09 | `workspace/symbol` with a non-matching query (`"zzz"`) | error path | an empty result set, no error |

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
- **2026-06-25** — Expanded §11.2 test plan to cover every REQ sub-case, §10 edge, and §6 state in both happy and negative polarities (per-construct `SymbolKind` rows incl. keyword-default detail and alias-import slot, range/selectionRange, the three local-exclusion negatives, deep block▸macro▸block nesting, duplicate-name-in-file, cross-file same-name, inline/E31 in both views, variable/import workspace negatives, fuzzy ranking + stability + no-match); rewrote §11.4 to map each REQ to its grouped rows; expanded §12.2 E2E scenarios to 9 covering both requests, all symbol kinds, ranking, empties, and the broken-file and no-match paths.
