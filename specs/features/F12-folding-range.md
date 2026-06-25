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

Every foldable construct kind in §5.1 gets its own happy row (correct start/end span) and, where a negative polarity exists, a paired negative row. Synthetic `didOpen` documents supply the constructs absent from the baseline fixture (`raw`, `call`, `if/elif/else`, `for/else`, deeply nested, whitespace-control, single-line variants), per [E17-testing §5](../foundations/E17-testing.md#starlette-blog).

**Per-construct ranges (REQ-FOLD2-01) — one row per kind:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `{% block content %}…{% endblock %}` in `post.html` yields a range from the `block` opening line to the `endblock` line | integration | starlette-blog | REQ-FOLD2-01 |
| `{% for c in post.comments %}…{% endfor %}` inside `content` yields a range from `for` line to `endfor` line | integration | starlette-blog | REQ-FOLD2-01 |
| Multi-line `{# Post detail page — extends base… #}` header in `post.html` yields a range from its first line to its last line | integration | starlette-blog | REQ-FOLD2-01 |
| `{% macro post_url(post) %}…{% endmacro %}` in `macros.html` (spanning ≥ 2 lines) yields a range from `macro` line to `endmacro` line | integration | starlette-blog | REQ-FOLD2-01 |
| `{% if x %}<body>…{% endif %}` (multi-line) yields a range from `if` line to `endif` line | integration | synthetic doc | REQ-FOLD2-01 |
| `{% call comment_card(c) %}<body>…{% endcall %}` (multi-line) yields a range from `call` line to `endcall` line | integration | synthetic doc | REQ-FOLD2-01 |
| `{% raw %}…{% endraw %}` (multi-line) yields a range from `raw` line to `endraw` line | integration | synthetic doc | REQ-FOLD2-01 |

**Region kind mapping (REQ-FOLD2-02) — both polarities, every kind:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| The multi-line `{# … #}` fold in `post.html` carries `kind = Comment` | integration | starlette-blog | REQ-FOLD2-02 |
| The `block` and `for` folds in `post.html` carry `kind = Region` (not Comment) | integration | starlette-blog | REQ-FOLD2-02 |
| `macro`, `if`, `call`, `raw` folds each carry `kind = Region` | integration | starlette-blog + synthetic doc | REQ-FOLD2-02 |

**Fold boundaries & single-line negatives (REQ-FOLD2-03):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| A multi-line fold's `endLine` is the closing tag's line (body incl. close hides, opener stays visible) | integration | starlette-blog | REQ-FOLD2-03 |
| Single-line `{% extends "base.html" %}` yields no fold | integration | starlette-blog | REQ-FOLD2-03 |
| Single-line `{% if x %}…{% endif %}` on one line yields no fold | integration | synthetic doc | REQ-FOLD2-03 |
| One-line `{# … #}` comment yields no fold | integration | synthetic doc | REQ-FOLD2-03 |

**Whitespace-control & intermediate clauses (REQ-FOLD2-04):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `{%- for c in post.comments -%}…{%- endfor -%}` folds by node span; trim markers don't shift `startLine`/`endLine` | integration | synthetic doc | REQ-FOLD2-04 |
| `{% if %}…{% elif %}…{% else %}…{% endif %}` folds as one region from `if` to `endif` (no per-branch sub-folds) | integration | synthetic doc | REQ-FOLD2-04 |
| `{% for %}…{% else %}…{% endfor %}` folds as one region from `for` to `endfor` | integration | synthetic doc | REQ-FOLD2-04 |

**§10 edges & §6 states (negative / structural):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Unclosed `{% for %}` (MISSING `endfor`) yields no range — no run-to-EOF fold (§10) | integration | syntax-errors | REQ-FOLD2-01, REQ-FOLD2-03 |
| Unclosed `{% block %}` with no `{% endblock %}` yields no range (§10) | integration | syntax-errors | REQ-FOLD2-01, REQ-FOLD2-03 |
| Deeply nested `for` ▸ `if` ▸ `block` each yield an independent range; outer span encloses inner spans (§10) | integration | synthetic doc | REQ-FOLD2-01 |
| `post.html` block ▸ nested loop: both ranges present, foldable independently (§6.2) | integration | starlette-blog | REQ-FOLD2-01 |
| `{% raw %}{% for %}…{% endfor %}{% endraw %}` yields exactly one `raw` region; inner tag-like text produces no nested fold (§10) | integration | synthetic doc | REQ-FOLD2-01 |
| Inline template ([E31](../foundations/E31-inline-templates.md)): a foldable `{% for %}` inside an inline region yields a range in host-file coordinates (§10) | integration | call-and-paths | REQ-FOLD2-01 |
| §6.1 layout: `post.html` returns the comment fold (Comment), the `content` block fold (Region), and leaves the single-line `extends`/`from` unfolded — exactly the gutter shown | integration | starlette-blog | REQ-FOLD2-01, REQ-FOLD2-02, REQ-FOLD2-03 |

### 11.3 Fixtures

- `starlette-blog` for the on-disk construct catalog (`block`, `for`, multi-line `{# #}`, `macro`, single-line `extends`/`from`); `syntax-errors` for the unclosed-tag recovery cases; `call-and-paths` for the inline/E31 host-coordinate case. Constructs absent from those fixtures (`raw`, `call`, `if/elif/else`, `for/else`, single-line `{% if %}`/`{# #}`, deeply nested, whitespace-control) use synthetic in-memory `didOpen` documents, per [E17-testing §5](../foundations/E17-testing.md#starlette-blog). Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-FOLD2-01 | per-construct range tests (block, for, comment, macro, if, call, raw), nested-independence, raw-literal, inline host-coord, and unclosed-tag negatives |
| REQ-FOLD2-02 | region-vs-comment kind tests across all kinds, plus the §6.1 layout row |
| REQ-FOLD2-03 | endLine-keeps-opener test and single-line negatives (`extends`, one-line `if`, one-line `{# #}`), plus unclosed-tag negatives |
| REQ-FOLD2-04 | whitespace-control span test, `if/elif/else` one-region test, `for/else` one-region test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the foldable constructs and the kind mapping**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `foldingRange` on `post.html` (didOpen → request) | happy | ranges for the multi-line comment (Comment), `content` block (Region), and nested `for` loop (Region), each with correct start/end (§6.1, §6.2) |
| E2E-02 | `foldingRange` over a `didOpen` doc holding `macro`, `call`, and `raw` blocks | happy | three Region ranges, one per construct, with spans from opener to closer |
| E2E-03 | `foldingRange` over a `didOpen` doc with `{% if %}…{% elif %}…{% else %}…{% endif %}` | happy | a single Region range from `if` to `endif`; no per-branch sub-folds |
| E2E-04 | `foldingRange` over a `didOpen` doc with whitespace-control `{%- for -%}…{%- endfor -%}` | happy | one Region range by node span; trim markers don't shift boundaries |
| E2E-05 | `foldingRange` on a single-line `{% if x %}…{% endif %}` | negative | no range emitted |
| E2E-06 | `foldingRange` on a single-line `{% extends "base.html" %}` and a one-line `{# … #}` | negative | no ranges emitted for either |
| E2E-07 | `foldingRange` on an unclosed `{% for %}` (no `{% endfor %}`) | error path | no run-to-EOF range; server stays healthy and responds to the next request |

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
- **2026-06-25** — Expanded §11.2 to one row per foldable construct kind (block, for, comment, macro, if, call, raw) in both polarities, plus rows for every §10 edge (unclosed-tag negatives, deep nesting, raw-literal, inline host-coords) and §6 states; rewrote §11.4 so each REQ lists its covering rows; expanded §12.2 to seven sequential E2E scenarios spanning happy, negative, and error paths.
