# F10 — Symbols (Document & Workspace)

> **Status:** Draft
>
> **Version:** 0.3   ·   **Last updated:** 2026-06-26
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
| `{% block name %}` | `Class` | — |
| `{% macro name(params) %}` | `Function` | the parameter list |
| top-level `{% set x = … %}` | `Variable` | — |
| `{% import "…" as alias %}` / `{% from "…" import … %}` | `Namespace` | the source template path |
| `{% extends "…" %}` | `Module` | the parent template path |
| `{% include "…" %}` | `Module` | the included template path |

Each symbol carries its full range (the whole construct) and a selection range (the name), so editors can both reveal and highlight it. The selection-range shapes for the two import forms and for `extends`/`include` are pinned in §5.5.

> **Kind rationale (see §15):** `block → Class` (a nestable, named container with a distinct outline icon — `Module` collided with imports and reads oddly for a template block); `macro → Function`; top-level `set → Variable`; imports → `Namespace` (a named bag of imported symbols); `extends`/`include` → `Module` (a whole-template reference, distinct from the `Namespace` imports). The choice of `Namespace` (not `Module`) for imports is deliberate and consistent across both import shapes.

> **Note — what earns an outline slot:** **Top-level** `{% set %}` variables appear; loop variables, macro parameters, and `{% set %}` *inside* a block or macro do not — those are local detail, not outline-worthy. The cross-template landmarks `{% extends %}` and `{% include %}` **do** appear: they're navigable structure authors expect to jump to (and are already `TemplateReference`s in [E07](../foundations/E07-data-model.md) REQ-DATA-05). **Decision (§15):** top-level `{% set %}` keeps its slot — it is part of the file's declared interface, unlike in-block locals — but is excluded from workspace search (§5.3). This keeps the outline a map of the file's *interface*, not its internals.

### 5.2 Document outline — nesting

Blocks and macros contain other constructs, and the outline should reflect that containment.

**REQ-SYM-02 — Nested constructs nest in the tree.**

A block or macro defined inside another block or macro becomes a *child* of the enclosing symbol in the `DocumentSymbol` tree. The outline can nest arbitrarily deep.

Nesting is **not** a stored field. The `TemplateIndex` holds macros and blocks in two *flat* vectors (`macros`, `blocks` — [E07](../foundations/E07-data-model.md) §8); the tree is computed **at request time** by `span`-containment across both vectors together: a symbol whose `span` falls inside another symbol's `span` becomes that symbol's child, regardless of which vector either came from. So a `{% macro %}` (from `macros`) nests under an enclosing `{% block %}` (from `blocks`) — the two vectors are merged, sorted, and folded into one tree by containment alone.

The containment fold is deterministic:

- **Strict containment.** A child's `span` must be **strictly inside** the parent's — `parent.start ≤ child.start` and `child.end ≤ parent.end`, with at least one inequality strict. A symbol nests under the **innermost** symbol that strictly contains it.
- **Boundary rule.** Spans are half-open `[start, end)`; a child sharing exactly one boundary with its parent is still contained, but two symbols with the **identical** span are *not* containment-related and stay **siblings** (this is the duplicate-name `W302` case, §10).
- **Tie-break.** Merge the two vectors and sort by `span.start` ascending, then by `span` length **descending** (longer — i.e. enclosing — first). Equal-span symbols keep their original vector order. This ordering makes the parent precede its children and makes sibling order deterministic.
- **Partial-parse fallback (P3).** On a broken parse, spans may overlap without strict containment. Such a symbol is **not** forced under a partial overlap; it attaches to the nearest strictly-containing ancestor or, failing that, becomes a top-level sibling. The outline degrades to a flatter-but-valid tree rather than erroring.

### 5.3 Workspace symbol search

The workspace search is the document outline's sibling, scaled to every file.

**REQ-SYM-03 — `workspace/symbol` fuzzy-searches every macro and block.**

`workspace/symbol` takes a query string and returns a `WorkspaceSymbol[]` of every **macro** and **block** in the workspace whose name fuzzy-matches the query. Each result carries the symbol name, its `SymbolKind` (per §5.1), a `Location` at its definition, and a `containerName` set to the template path so duplicate names across files stay distinguishable. An empty query returns all macros and blocks.

> **Note:** Workspace search is scoped to **macros and blocks** — the named, *defining* symbols worth jumping to from anywhere. Top-level variables, imports, and `extends`/`include` landmarks are file-local detail and stay in the document outline only. This is a deliberate **subset** of [F09](F09-find-references.md)'s resolvable set — *not* a mirror of it. F09 resolves macros, blocks, imports, **and** scope-local variables (REQ-REF-01); F10 workspace search returns only the two *definitional* kinds (macros and blocks), because imports and scope-locals are usage- or file-local symbols, not workspace-wide definitions you'd navigate to by name.

### 5.4 Fuzzy matching

The query rarely matches exactly; it's a few letters of the name.

**REQ-SYM-04 — Match is fuzzy, returned in a stable deterministic order; tightness ranking is best-effort.**

Matching is subsequence-based and case-insensitive: `pu` matches `post_url`. The **stable deterministic order** below is normative — repeated runs are byte-for-byte identical — while the **tightness ranking** within it is best-effort: most LSP clients (VS Code, Zed) re-sort `workspace/symbol` results client-side, so the server's tier ordering is partly unobservable in-editor. We still rank server-side because it is observable and load-bearing for the CLI and for deterministic golden tests, and it gives clients that *don't* re-rank a sensible default. Results are ordered by the following **total order**, applied in sequence; each tier breaks ties left by the previous one:

1. **Match tightness** (primary, best first): **exact** (the whole name equals the query, case-insensitively) > **prefix** (the name starts with the query) > **contiguous substring** (the query appears as an unbroken run somewhere inside the name) > **scattered subsequence** (the query's characters appear in order but not contiguously).
2. **Name length** (secondary, shorter first): among equal-tightness matches, the shorter name ranks higher — `pub` outranks `pub_notice` for query `pu`.
3. **Workspace order** (final tiebreak, stable): any results still tied keep their original workspace-index order, so repeated runs are byte-for-byte identical.

Case is used only to break ties *within* a tightness tier when an implementation chooses to (exact-case before differing-case); it never changes the tier. Because the order is total and ends in a stable tiebreak, output is deterministic across runs.

### 5.5 Document-symbol shape for imports and cross-template references

Imports come in two shapes, and `extends`/`include` are anonymous landmarks. Each needs `name`, `detail`, `range`, and `selectionRange` pinned so the outline is unambiguous.

**REQ-SYM-05 — Pin the `name`/`detail`/`range`/`selectionRange` of imports, `extends`, and `include`.**

| Construct | `name` | `detail` | `range` | `selectionRange` |
|---|---|---|---|---|
| `{% import "blog/macros.html" as macros %}` (alias-import) | the alias — `macros` | source path — `blog/macros.html` | the whole `{% import … %}` tag | the **alias identifier** (`macros`) |
| `{% from "blog/macros.html" import a, b %}` (from-import) | the **source path** — `blog/macros.html` | source path — `blog/macros.html` | the whole `{% from … import … %}` tag | the **source-path string literal** (`"blog/macros.html"`) |
| `{% extends "base.html" %}` | the parent path — `base.html` | parent path — `base.html` | the whole `{% extends … %}` tag | the **path string literal** (`"base.html"`) |
| `{% include "sidebar.html" %}` | the included path — `sidebar.html` | included path — `sidebar.html` | the whole `{% include … %}` tag | the **path string literal** (`"sidebar.html"`) |

Rationale (see §15): an alias-import binds a single name (`macros`), so that name is the natural label and selection target. A from-import binds *several* names with no single identifier to title the entry, so it is titled by its **source path** and its `selectionRange` spans the source-path string literal — the one span common to the whole import. `extends`/`include` are likewise anonymous, so they too are named and selection-anchored by their path literal. The imported *names* of a from-import (`a`, `b`) are not surfaced as child symbols — they are file-local bindings, not outline structure.

> **Invariant — `selectionRange ⊆ range`.** Every `selectionRange` is a real token range fully contained within its symbol's `range`, as LSP requires. Where the `name`/`detail` is an *unquoted* path (`base.html`, `blog/macros.html`), that string is only the display label — the `selectionRange` still spans the **path string literal including its surrounding quotes** (the `"base.html"` token, quotes included), never a synthetic unquoted sub-range. The same holds for the alias identifier (`macros`) and source-path literal of imports.

## 6. UI Mockups

### 6.1 Document outline — the `post.html` tree

The outline of `templates/blog/post.html`: the `extends` landmark, then the
`blog/macros.html` from-import (named by its source path, §5.5/D5), then the
`content` block this template fills.

```
┌─ OUTLINE — templates/blog/post.html ─────────────────────────────────────┐
│                                                                            │
│  ☖ base.html                          {% extends "base.html" %}  [Module]  │
│  ◇ blog/macros.html                   from "blog/macros.html"  [Namespace] │
│  ⬡ content                            {% block content %}       [Class]    │
│                                                                            │
└───────────────────────────────────────────────────────────────────────────┘
  ☖ Module (extends/include)   ◇ Namespace (import)   ⬡ Class (block)
  ƒ Function (macro)   𝑥 Variable (top-level set)
```

Nesting (§5.2), the `Function` macro kind, and the top-level `Variable` `set`
kind are exercised by `blog/macros.html` and the synthetic-doc test rows in
§11.2, not by this cast file.

### 6.2 Workspace symbol search — typing "pu"

The command palette filters every macro and block to fuzzy matches, each tagged with its template.

```
┌─ Go to Symbol in Workspace ──────────────────────────────────────────────┐
│  > pu                                                                      │
├───────────────────────────────────────────────────────────────────────────┤
│  ƒ  post_url            templates/blog/macros.html:1     [Function]        │
│  ƒ  pubdate             templates/blog/macros.html:14    [Function]        │
│  ⬡  pub_notice          templates/email/digest.html:3    [Class]           │
│                                                                            │
│  ⏎ open    ⇧⏎ open to the side    matches: name subsequence, ranked        │
└───────────────────────────────────────────────────────────────────────────┘
```

## 9. Examples & Use Cases

Opening `templates/blog/post.html` in `starlette-blog`, the outline shows the `extends "base.html"` landmark (kind `Module`), the `blog/macros.html` import as a `Namespace`, the `content` block (kind `Class`) with any macros it nests, and top-level `{% set %}` variables (§5.1, §5.2, §5.5). Typing `comment` into the workspace symbol search surfaces `comment_card` from `blog/macros.html` even though that file isn't open (§5.3). Typing `pu` surfaces `post_url` via subsequence matching, ranked by tightness then length (§5.4).

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
| `{% block content %}` → `Class`, no detail | integration | starlette-blog (`blog/post.html`) | symbol `content`, kind `Class`, detail empty | REQ-SYM-01 |
| `{% macro post_url(post) %}` → `Function`, param-list detail | integration | starlette-blog (`blog/macros.html`) | symbol `post_url`, kind `Function`, detail `(post)` | REQ-SYM-01 |
| `{% macro comment_card(comment, show_actions=true) %}` → `Function`, detail shows keyword default | integration | starlette-blog (`blog/macros.html`) | detail `(comment, show_actions=true)` | REQ-SYM-01 |
| top-level `{% set page_title = … %}` → `Variable` | integration | synthetic doc (top-level `set` in a template) | symbol `page_title`, kind `Variable`, no detail | REQ-SYM-01 |
| `{% extends "base.html" %}` → `Module`, parent-path detail | integration | starlette-blog (`blog/post.html`) | symbol `base.html`, kind `Module`, detail `base.html` | REQ-SYM-01 |
| `{% include "sidebar.html" %}` → `Module`, included-path detail | integration | synthetic doc (a template with an `{% include %}`) | symbol `sidebar.html`, kind `Module`, detail `sidebar.html` | REQ-SYM-01 |
| Each symbol carries full range (whole construct) and selection range (the name) | integration | starlette-blog (`blog/macros.html`) | `range` spans the tag; `selectionRange` spans the name only | REQ-SYM-01 |
| **Negative:** loop variable (`{% for c in … %}`) is **not** an outline symbol | integration | starlette-blog (`blog/post.html`) | no symbol named `c` | REQ-SYM-01 |
| **Negative:** macro parameter is **not** a top-level symbol | integration | starlette-blog (`blog/macros.html`) | `post`/`comment` appear only as detail, not as sibling symbols | REQ-SYM-01 |
| **Negative:** `{% set %}` inside a block is **not** outline-worthy | integration | synthetic doc (`set` nested in a block) | no `Variable` symbol for the in-block `set` | REQ-SYM-01 |
| **Edge (§10):** syntactically broken template yields a partial outline, never an error (P3) | integration | syntax-errors | symbols Pass 1 extracted are returned; missing constructs simply absent; no error response | REQ-SYM-01 |
| **Edge (§10, E31):** inline-template symbols appear with ranges in host-file coordinates | integration | call-and-paths (inline/E31 case) | inline macro/block symbols present, ranges in host file | REQ-SYM-01 |

**Document outline — import / `extends` / `include` shape (REQ-SYM-05)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Alias-import shape — `{% import "blog/macros.html" as macros %}` | integration | synthetic doc (alias `import`) | name `macros`, kind `Namespace`, detail `blog/macros.html`, `range` spans the tag, `selectionRange` spans the alias identifier `macros` | REQ-SYM-05 |
| From-import shape — `{% from "blog/macros.html" import post_url %}` | integration | starlette-blog (`email/digest.html`) | name `blog/macros.html`, kind `Namespace`, detail `blog/macros.html`, `range` spans the tag, `selectionRange` spans the source-path string literal | REQ-SYM-05 |
| `extends` shape — `{% extends "base.html" %}` | integration | starlette-blog (`blog/post.html`) | name `base.html`, kind `Module`, `range` spans the tag, `selectionRange` spans the path string literal | REQ-SYM-05 |
| `include` shape — `{% include "sidebar.html" %}` | integration | synthetic doc (an `{% include %}`) | name `sidebar.html`, kind `Module`, `range` spans the tag, `selectionRange` spans the path string literal | REQ-SYM-05 |
| **Negative:** from-import imported names (`a`, `b`) are **not** child symbols | integration | synthetic doc (`{% from "x" import a, b %}`) | the from-import entry has no `children`; `a`/`b` are absent as symbols | REQ-SYM-05 |

**Document outline — nesting (REQ-SYM-02)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Macro defined inside a block nests as that block's child | integration | starlette-blog (`blog/post.html`) | macro symbol is a `children` entry of the `content` `Class` | REQ-SYM-02 |
| **Cross-vector nesting:** a `{% macro %}` (from `macros`) nests under an enclosing `{% block %}` (from `blocks`) by span-containment alone | integration | synthetic doc (a `{% block %}` whose body contains a `{% macro %}`) | the macro symbol is a `children` entry of the block symbol even though the two come from different `TemplateIndex` vectors (E07 §8) | REQ-SYM-02 |
| **Edge (§10):** deeply nested block-in-macro-in-block nests to match | integration | synthetic doc (block ▸ macro ▸ block) | three-level `children` chain mirrors span containment, computed at request time across the flat vectors | REQ-SYM-02 |
| **Edge (§10, W302):** duplicate macro name in one file lists both definitions in the outline | integration | duplicates | two sibling symbols with the same name; the `W302` diagnostic is separate (F01) | REQ-SYM-02 |
| **Negative:** sibling (non-contained) constructs do **not** nest | integration | starlette-blog (`blog/macros.html`) | `post_url` and `comment_card` are siblings, neither a child of the other | REQ-SYM-02 |

**Workspace symbol search (REQ-SYM-03)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Search returns every macro **and** block across files, each with kind and `containerName` | integration | starlette-blog | results include `post_url`/`comment_card` (`Function`) and `content`/`head`/`body`/`footer` (`Class`); each `containerName` is its template path; each `Location` at the definition | REQ-SYM-03 |
| **Edge (§10):** empty query returns all macros and blocks | integration | starlette-blog | every macro and block in the workspace returned | REQ-SYM-03 |
| **Edge (§10):** same name as a macro in one file and a block in another — both appear, disambiguated by `containerName` | integration | synthetic doc + starlette-blog | two results sharing the name, different `containerName`/kind | REQ-SYM-03 |
| **Edge (§10, E31):** inline-template macro/block appears in workspace search with host-file `Location` | integration | call-and-paths (inline/E31 case) | inline symbol present, `Location` in host file | REQ-SYM-03 |
| **Negative:** top-level variables are **not** in workspace results | integration | synthetic doc (top-level `set`) | the `set` variable name is absent from results | REQ-SYM-03 |
| **Negative:** imports are **not** in workspace results | integration | starlette-blog (`email/digest.html`) | the `from`-import is absent from results | REQ-SYM-03 |
| **Negative:** `extends`/`include` landmarks are **not** in workspace results | integration | starlette-blog (`blog/post.html`) | the `extends "base.html"` entry is absent from results (outline-only, §5.3) | REQ-SYM-03 |
| **Edge (§10):** large workspace stays within latency budget by reading the prebuilt index | integration | large-workspace | empty-query result returns within the budget (< 100 ms, P6) | REQ-SYM-03 |

**Fuzzy matching & ranking (REQ-SYM-04)**

| Behavior / scenario | Type | Fixtures | Expected outcome | Verifies |
|---|---|---|---|---|
| Subsequence match: `pu` matches `post_url` | integration | starlette-blog | `post_url` is among the results for `pu` | REQ-SYM-04 |
| Case-insensitive: `PU` / `Post` match `post_url` | integration | starlette-blog | `post_url` present for the differing-case query | REQ-SYM-04 |
| Ranking tiers (primary): exact > prefix > contiguous-substring > scattered-subsequence | integration | synthetic doc (names `pu`, `pub_notice`, `excerpt_pubdate`, `post_url`) | for query `pu`, order is exact (`pu`) > prefix (`pub_notice`) > contiguous-substring (`excerpt_pubdate`) > scattered subsequence (`post_url`) | REQ-SYM-04 |
| Ranking tie-break (secondary): shorter name wins among equal-tightness matches | integration | synthetic doc (`pub`, `pub_notice` — both prefix matches of `pu`) | `pub` ranks above `pub_notice` | REQ-SYM-04 |
| Ranking final tiebreak: equal-tightness, equal-length results keep workspace order (deterministic) | integration | starlette-blog | repeated runs return byte-for-byte identical order for fully tied results | REQ-SYM-04 |
| **Negative:** a query that is not a subsequence of any name returns no matches | integration | starlette-blog | query `zzz` returns an empty result set | REQ-SYM-04 |

### 11.3 Fixtures

- `starlette-blog` for the outline, mapping, workspace search, and fuzzy ranking; `syntax-errors` for the partial-outline edge; `duplicates` for the same-name-in-one-file outline edge; `call-and-paths` for the inline/E31 cases; `large-workspace` for the empty-query latency budget. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry). Edge constructs with no fixture (top-level/in-block `set`, alias `import`, `{% include %}`, from-import-names exclusion, cross-vector and deep block▸macro▸block nesting, cross-file same-name, and the ranking-tier/length corpus) use synthetic in-memory `didOpen` documents.

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-SYM-01 | §11.2 *Document outline — `SymbolKind` mapping* rows (block→`Class`/macro→`Function`/macro-with-default/top-level-`set`→`Variable`/`extends`→`Module`/`include`→`Module` kinds, range + selectionRange, the three local-exclusion negatives, and the broken-template and inline/E31 edges) |
| REQ-SYM-02 | §11.2 *Document outline — nesting* rows (child nesting, cross-vector macro-under-block, deep block▸macro▸block, duplicate-name-in-file, non-contained-sibling negative) |
| REQ-SYM-03 | §11.2 *Workspace symbol search* rows (all macros + blocks with `containerName`, empty query, cross-file same-name, inline/E31, variable/import/`extends`-`include` negatives, large-workspace latency) |
| REQ-SYM-04 | §11.2 *Fuzzy matching & ranking* rows (subsequence, case-insensitive, the four ranking tiers, length tie-break, stable final tiebreak, no-match negative) |
| REQ-SYM-05 | §11.2 *Document outline — import / `extends` / `include` shape* rows (alias-import name/detail/range/selectionRange, from-import source-path name + string-literal selectionRange, `extends` and `include` shapes, from-import-names-not-children negative) |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of both symbol requests**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `documentSymbol` on `blog/post.html` | happy | the `extends "base.html"` `Module` landmark, the `macros` `Namespace` import, and the `content` `Class` with its nested macro child — correct kinds/details |
| E2E-02 | `documentSymbol` on `blog/macros.html` — macro kinds & details | happy | `post_url` and `comment_card` as sibling `Function`s with param-list details, neither nested under the other |
| E2E-03 | `documentSymbol` on `email/digest.html` — from-import shape | happy | the `from "blog/macros.html"` import appears as a `Namespace`, name `blog/macros.html`, source-path detail, `selectionRange` on the source-path string literal |
| E2E-03b | `documentSymbol` — alias-import shape | happy (synthetic `didOpen` with `{% import "blog/macros.html" as macros %}`) | name `macros`, kind `Namespace`, detail `blog/macros.html`, `selectionRange` on the alias identifier |
| E2E-04 | `documentSymbol` outline excludes locals | happy | no symbol for the `{% for c … %}` loop variable or macro parameters |
| E2E-05 | `workspace/symbol` with `"pu"` | happy | `post_url` among the fuzzy matches, tagged `Function` with its `containerName` |
| E2E-06 | `workspace/symbol` with an empty query | happy | every macro and block (`post_url`, `comment_card`, `content`, `head`, `body`, `footer`) returned, none of the variables, imports, or `extends`/`include` landmarks |
| E2E-07 | `workspace/symbol` ranking — `"pu"` orders tighter matches first | happy | results come back in the tier-then-length order of §5.4, with a stable final tiebreak |
| E2E-08 | `documentSymbol` on a broken file | error path | a partial outline from what Pass 1 extracted, no crash |
| E2E-09 | `workspace/symbol` with a non-matching query (`"zzz"`) | error path | an empty result set, no error |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — a read-only LSP handler over stdio (P2); single-user developer tool; no host execution (P1). The trust boundary is the workspace index — both requests are pure reads of it (§3).
- **Input & validation** — all template content is untrusted; symbols are read from the index only, never by executing templates (P1).
- **Data sensitivity** — symbol names and locations point only into the user's own workspace; nothing leaves the machine.
- **Baseline** — meets OWASP ASVS L1. STRIDE: the only untrusted input is template text, handled by static tree-sitter parsing (P1, P3) with no execution path to threaten.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the outline pane and the symbol palette.

### 13.4 Performance & Scale

- **Latency** — the document outline reads one `TemplateIndex` and workspace search reads the prebuilt `WorkspaceIndex`; both return in < 100 ms (P6). The index itself is built within the 2 s budget ([E30](../foundations/E30-extraction-and-indexing.md)).

## 15. Open Questions & Decisions

- **D1 — `block → Class`, not `Module` (decided 2026-06-25).** The original draft mapped `{% block %}` to `Module`, which (a) collided with the `Namespace`/`Module` space used for imports and (b) read oddly — a template block is a nestable, named container, not a module. `Class` is the LSP kind whose semantics ("a nestable named container") and distinct editor icon fit best, while leaving `Module` free for whole-template references. Final mapping: `block → Class`, `macro → Function`, top-level `set → Variable`, imports → `Namespace`, `extends`/`include` → `Module` (§5.1).
- **D2 — imports are `Namespace`, consistently (decided 2026-06-25).** Both import shapes (`{% import … as … %}` and `{% from … import … %}`) map to `Namespace` — an import brings a named bag of symbols into scope. `Module` is reserved for the whole-template `extends`/`include` references, keeping the two concepts visually and semantically distinct.
- **D3 — `extends`/`include` earn outline slots (decided 2026-06-25).** They are navigable cross-template landmarks authors expect to jump to from the outline, and already exist as `TemplateReference`s in [E07](../foundations/E07-data-model.md) (REQ-DATA-05). They appear as `Module` symbols (§5.1, §5.5) but are **outline-only** — excluded from workspace search (§5.3).
- **D4 — top-level `{% set %}` keeps its outline slot (decided 2026-06-25).** A top-level `set` is part of the file's declared *interface*, so it stays in the outline (kind `Variable`); in-block/in-macro `set`, loop variables, and macro parameters are local detail and are excluded. Top-level `set` is still excluded from workspace search — it is not a cross-file *defining* symbol (§5.3).
- **D5 — from-import is titled by its source path (decided 2026-06-25).** A from-import binds several names with no single identifier to title the outline entry, so it is named by its source path with the `selectionRange` on the source-path string literal; an alias-import is named by (and selection-anchored on) its single alias identifier (§5.5).
- **D6 — workspace search is a deliberate subset of F09's resolvable set (decided 2026-06-25).** F10 workspace search returns only the *defining* kinds (macros, blocks); it intentionally does **not** mirror [F09](F09-find-references.md), whose resolvable set also includes imports and scope-local variables (§5.3).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P3, P6; [E07-data-model](../foundations/E07-data-model.md) — the symbol types shaped into responses; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — the workspace index; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F08-go-to-definition](F08-go-to-definition.md) — jumping to a symbol; [F09-find-references](F09-find-references.md) — for **macros and blocks only**, the same definitions, as usages (F10's outline additionally surfaces `extends`/`include` landmarks and top-level `set`, which F09 does not resolve); [F12-folding-range](F12-folding-range.md) — the file's structure as collapsible regions.
- **Roadmap:** ships in **M3 — Read features** ([roadmap](../roadmap.md)), alongside the other navigation features (F08/F09, F11–F16).

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-24** — Outline example names the `blog/macros.html` import (a `from`-import shown as a `Namespace`, §5.1), not an alias namespace, matching how `post.html` imports.
- **2026-06-25** — Expanded §11.2 test plan to cover every REQ sub-case, §10 edge, and §6 state in both happy and negative polarities (per-construct `SymbolKind` rows incl. keyword-default detail and alias-import slot, range/selectionRange, the three local-exclusion negatives, deep block▸macro▸block nesting, duplicate-name-in-file, cross-file same-name, inline/E31 in both views, variable/import workspace negatives, fuzzy ranking + stability + no-match); rewrote §11.4 to map each REQ to its grouped rows; expanded §12.2 E2E scenarios to 9 covering both requests, all symbol kinds, ranking, empties, and the broken-file and no-match paths.
- **2026-06-26** — v0.3 (spec-review fixes): fixed the §6.1 outline mockup to the cast — from-import row named by its source path `blog/macros.html` (jinja-lsp-5ze), and dropped the non-cast `excerpt()` macro and `page_title` set, noting those kinds are exercised by `macros.html`/synthetic rows (jinja-lsp-5fc); added the `selectionRange ⊆ range` invariant and clarified path string literals include their quotes (jinja-lsp-a7g); defined the §5.2 span-containment fold — strict containment, half-open boundary rule, start-then-length tie-break, partial-parse fallback (jinja-lsp-464); softened REQ-SYM-04 to "stable deterministic order; tightness ranking best-effort", acknowledging client re-sort (jinja-lsp-rr1); scoped the §16 F09 equivalence to macros/blocks only (jinja-lsp-29e); replaced "bijection" with "requirement coverage" (jinja-lsp-rx8); added an M3 roadmap reference (jinja-lsp-6by); added the required §13.1 Access & authorization and Baseline bullets (jinja-lsp-9ix).
- **2026-06-25** — v0.2: remapped `block → Class` (was `Module`) and made imports consistently `Namespace`; added `{% extends %}`/`{% include %}` as outline-only `Module` landmarks; added **REQ-SYM-05** pinning the `name`/`detail`/`range`/`selectionRange` of both import shapes and of `extends`/`include` (new §5.5); replaced the §5.4 ranking prose with a concrete total order (exact > prefix > contiguous-substring > scattered-subsequence; length secondary; stable workspace-order final tiebreak); stated nesting is computed at request time by span-containment across the flat `macros`/`blocks` vectors and added a cross-vector nesting test; corrected the §5.3 Note to call workspace search a deliberate *subset* of F09's resolvable set, not a mirror; added §15 Open Questions & Decisions (D1–D6); updated §6 mockups, §9, §11.2/§11.4/§12.2, and the requirement coverage (REQ↔test mapping).
