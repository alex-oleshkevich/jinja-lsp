# F12 — Folding Range

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Tell the editor which Jinja regions collapse — blocks, loops, conditionals, macros, calls, multi-line comments, and `{% raw %}` — derived from tree-sitter spans rather than guessed from indentation.

> **Depends on:** [constitution](../constitution.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md), [E07-data-model](../foundations/E07-data-model.md)   ·   **Related:** [F10-symbols](F10-symbols.md), [F13-semantic-tokens](F13-semantic-tokens.md)

> Requirement tag: **FOLD2**

---

## 1. Purpose & Scope

A long template is easier to read when you can fold the parts you're not looking at — collapse the `{% for %}` loop, the big `{% block content %}`, the license comment at the top. This spec defines `textDocument/foldingRange`: which Jinja constructs are foldable, where each region starts and ends, and whether it folds as a *region* or a *comment*.

This spec covers:

- The foldable constructs: `block`, `for`, `if`, `macro`, `call`, multi-line `{# … #}`, `{% raw %}`.
- The region kind: `region` for tags, `comment` for comment blocks.
- Why we derive folds from tree-sitter spans instead of indentation.

## 2. Non-Goals / Out of Scope

- Folding the host language (HTML/SQL/text) — that stays with the host LSP and the editor (P5).
- The structural outline of named symbols — [F10-symbols](F10-symbols.md) (related, but a different request).
- Collapsing based on `#region` markers — Jinja has no such convention.

## 3. Background & Rationale

Editors fold by indentation when no language server tells them otherwise — and for Jinja, that misfires. A `{% for %}` loop wrapping a single line of HTML often has *less* indentation than the markup it contains; a block's `{% endblock %}` may sit at the same column as its body. Indentation-based folding collapses the wrong ranges or none at all.

We do better because tree-sitter already knows each construct's exact span. Pass 1 parses the template into a tree where `{% block %}…{% endblock %}` is one node with a precise start and end ([E30](../foundations/E30-extraction-and-indexing.md)). Folding is a pure read of those spans ([E01](../foundations/E01-architecture.md) REQ-ARCH-07): walk the tree, emit a `FoldingRange` for each foldable node. The fold matches the *logical* structure, not the visual whitespace.

## 4. Concepts & Definitions

- **Folding range** — a collapsible region. (Canonical definition in [glossary](../glossary.md).)
- **Region kind** — the LSP `FoldingRangeKind`: `region` for code-like regions, `comment` for comment blocks.
- **Span** — a tree-sitter node's start/end position, the source of every fold.

## 5. Detailed Specification

### 5.1 Foldable constructs

Every paired Jinja tag and every multi-line comment is a fold.

**REQ-FOLD2-01 — These constructs fold.**

`textDocument/foldingRange` returns a `FoldingRange` for each of:

| Construct | From | To |
|---|---|---|
| `{% block %} … {% endblock %}` | the `block` opening line | the `endblock` line |
| `{% for %} … {% endfor %}` | the `for` opening line | the `endfor` (or `else`-then-`endfor`) line |
| `{% if %} … {% endif %}` | the `if` opening line | the `endif` line |
| `{% macro %} … {% endmacro %}` | the `macro` opening line | the `endmacro` line |
| `{% call %} … {% endcall %}` | the `call` opening line | the `endcall` line |
| `{# … #}` spanning ≥ 2 lines | the comment's first line | its last line |
| `{% raw %} … {% endraw %}` | the `raw` opening line | the `endraw` line |

Ranges come from the tree-sitter span of each construct ([E30](../foundations/E30-extraction-and-indexing.md)). Nested constructs each produce their own range, so an editor can fold the outer loop and the inner `if` independently.

### 5.2 Region kind

The two kinds let editors fold-all-comments separately from fold-all-regions.

**REQ-FOLD2-02 — Comment blocks are `comment`; everything else is `region`.**

A multi-line `{# … #}` carries `kind = FoldingRangeKind.Comment`. Every tag-based fold (`block`, `for`, `if`, `macro`, `call`, `raw`) carries `kind = FoldingRangeKind.Region`. This is what makes "Fold All Block Comments" work distinctly from "Fold All Regions."

### 5.3 Fold boundaries

A fold should hide the body while leaving the opening line readable.

**REQ-FOLD2-03 — Fold to the line before the close, keeping the opener visible.**

A `FoldingRange` runs from `startLine` (the construct's opening line) to `endLine` (the closing tag's line). When collapsed, the opening line stays visible and the body — including the closing tag — hides. A construct entirely on one line (a single-line `{% if x %}…{% endif %}` or a one-line `{# … #}`) is **not** foldable: there's nothing to collapse.

### 5.4 Whitespace-control and `else` branches

Trim markers and `else`/`elif` clauses don't break a fold.

**REQ-FOLD2-04 — Whitespace-control markers and intermediate clauses fold cleanly.**

A construct using whitespace control (`{%- for … -%}`) folds by its node span regardless of the trim markers. An `{% if %}` with `{% elif %}`/`{% else %}` folds as one region from `if` to `endif`; a `{% for %}` with an `{% else %}` folds from `for` to `endfor`. (Per-branch sub-folds are out of scope for v1.)

## 6. UI Mockups

### 6.1 A template with regions collapsed

`templates/blog/post.html` with the header comment and the `content` block folded. The gutter shows the fold chevrons; collapsed lines render an ellipsis.

```
┌─ templates/blog/post.html ───────────────────────────────────────────────┐
│  1  ⊟ {# Post detail page — extends base, fills content. …  ⋯ #}  [comment]│
│  4    {% extends "base.html" %}                                           │
│  5    {% from "blog/macros.html" import post_url %}                       │
│  6  ⊟ {% block content %} ⋯ {% endblock %}                      [region]   │
│ 18    {% block footer %}                                                  │
│ 19      <small>{{ post.author }}</small>                                  │
│ 20    {% endblock %}                                                      │
└───────────────────────────────────────────────────────────────────────────┘
  ⊟ collapsed (click to expand)    ⊞ expanded
```

### 6.2 The same block expanded, with a nested loop foldable

Expanding `content` reveals a `{% for %}` that folds on its own.

```
  6  ⊞ {% block content %}                                        [region]
  7       <h1>{{ post.title }}</h1>
  8     ⊟ {% for comment in post.comments %} ⋯ {% endfor %}       [region]
 15    {% endblock %}
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` opens with a multi-line `{# … #}` license/summary comment (folds as `comment`), then a `{% block content %}` containing a `{% for comment in post.comments %}` loop. Each is its own region: you can fold the whole block, or expand it and fold just the loop (§5.1). The single-line `{% extends "base.html" %}` produces no fold (§5.3).

## 10. Edge Cases & Failure Modes

- **One-line construct** → no fold (§5.3).
- **Unclosed tag** (`{% for %}` with no `{% endfor %}`) → tree-sitter recovers with a `MISSING` node; we emit no range rather than a fold running to end-of-file (P3). [F01](F01-diagnostics.md) `JINJA-E001` flags the syntax error.
- **Deeply nested constructs** → each level folds independently (§5.1).
- **`{% raw %}` containing what looks like tags** → folds as one `raw` region; its contents are literal text, not parsed.
- **Whitespace-control markers** → don't affect the fold span (§5.4).
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → foldable constructs inside an inline region fold too, in host-file coordinates.

## 11. Testing

Folding is verified by integration tests asserting exact ranges over fixture templates plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-FOLD2-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Each construct yields a range with correct start/end | integration | starlette-blog | REQ-FOLD2-01 |
| Comment blocks are `comment`; tags are `region` | integration | starlette-blog | REQ-FOLD2-02 |
| Single-line constructs yield no fold | integration | starlette-blog | REQ-FOLD2-03 |
| Whitespace-control + `if/elif/else` fold as one region | integration | starlette-blog | REQ-FOLD2-04 |
| Unclosed tag yields no range (no run-to-EOF) | integration | syntax-errors | REQ-FOLD2-01 |

### 11.3 Fixtures

- `starlette-blog` for the construct catalog; `syntax-errors` for the unclosed-tag recovery case. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-FOLD2-01 | per-construct range test |
| REQ-FOLD2-02 | region-kind test |
| REQ-FOLD2-03 | single-line-no-fold test |
| REQ-FOLD2-04 | whitespace-control + branch test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the foldable constructs and the kind mapping**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `foldingRange` on `post.html` | happy | ranges for the comment, block, and loop with correct kinds |
| E2E-02 | A single-line `{% if %}` | happy | no range emitted |
| E2E-03 | An unclosed `{% for %}` | error path | no run-to-EOF range; server stays healthy |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; folds read tree-sitter spans only and never execute templates (P1).
- **Data sensitivity** — ranges describe only the open file's structure; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the fold chevrons and collapsed regions.

### 13.4 Performance & Scale

- **Latency** — folding is a single pass over the parse tree and returns in < 100 ms (P6), even for large templates.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P3, P5, P6; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — the tree-sitter spans; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers; [E07-data-model](../foundations/E07-data-model.md) — the constructs whose spans fold.
- **Related:** [F10-symbols](F10-symbols.md) — the named-symbol structure; [F13-semantic-tokens](F13-semantic-tokens.md) — another tree-driven, span-based feature.

## 17. Changelog

- **2026-06-24** — Initial draft.
