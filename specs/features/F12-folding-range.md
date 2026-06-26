# F12 — Folding Range

> **Status:** Draft
>
> **Version:** 0.2   ·   **Last updated:** 2026-06-25
>
> **Purpose:** Tell the editor which Jinja regions collapse — derived purely from delimiter **structure** (any balanced `{% name %}…{% endname %}` pair, multi-line comments, multi-line tags) rather than from each tag's semantics or from indentation.

> **Depends on:** [constitution](../constitution.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md), [E07-data-model](../foundations/E07-data-model.md), [E16-conventions](../foundations/E16-conventions.md), [E31-inline-templates](../foundations/E31-inline-templates.md)   ·   **Related:** [F10-symbols](F10-symbols.md), [F13-semantic-tokens](F13-semantic-tokens.md), [F01-diagnostics](F01-diagnostics.md)

> Requirement tag: **FOLD2**

---

## 1. Purpose & Scope

A long template is easier to read when you can fold the parts you're not looking at — collapse the `{% for %}` loop, the big `{% block content %}`, the license comment at the top, even a project's own `{% cache %}…{% endcache %}` extension tag. This spec defines `textDocument/foldingRange`, and it defines it **structurally**: folding never asks what a tag *means*, only how its delimiters *pair up*. Any balanced `{% name %}…{% endname %}` folds, every multi-line `{# … #}` folds, every multi-line `{{ … }}`/`{% … %}` tag folds. The spec fixes where each region starts and ends and whether it folds as a *region* or a *comment*.

This spec covers:

- **The universal structural model** — folding keyed off Jinja's `end<name>` closing convention, with no hardcoded tag list, so built-in and custom/extension tags fold alike.
- The three foldable shapes: a balanced `{% name %}…{% endname %}` pair, a multi-line `{# … #}` comment, and a single multi-line `{{ … }}`/`{% … %}` tag.
- The region kind: `region` for tags, `comment` for comment blocks.
- The 0-based `startLine`/`endLine` convention and exactly which line a fold hides.
- Why we derive folds from tree-sitter delimiter structure instead of indentation or tag semantics.

## 2. Non-Goals / Out of Scope

- **Per-tag semantics** — folding does not know, and does not need to know, what `block`/`for`/`cache`/`form` *do*. It pairs delimiters; the meaning of the tag is irrelevant (§3, §5.1).
- **Per-branch sub-folds** — an `{% if %}` folds as one region from `if` to `endif`; we do **not** emit separate folds for each `{% elif %}`/`{% else %}` arm (§5.1).
- **Folding the host language** (HTML/SQL/text) — that stays with the host LSP and the editor (P5). Jinja and host folds **coexist** (§5.5).
- The structural outline of named symbols — [F10-symbols](F10-symbols.md) (related, but a different request).
- Collapsing based on `#region` markers — Jinja has no such convention.

## 3. Background & Rationale

Editors fold by indentation when no language server tells them otherwise — and for Jinja, that misfires. A `{% for %}` loop wrapping a single line of HTML often has *less* indentation than the markup it contains; a block's `{% endblock %}` may sit at the same column as its body. Indentation-based folding collapses the wrong ranges or none at all.

We do better, and we do it **without a catalog of foldable tags**. Jinja's block tags share one universal closing convention: a tag opened with `{% name %}` is closed with `{% endname %}`. That single rule is all folding needs. We walk the template's `{%`/`{#`/`{{` delimiters in order, push each opener `name` onto a stack, and when a `{% endname %}` matches the name on top of the stack we emit a `FoldingRange` from the opener to the closer. Because the rule is keyed off the `end<name>` convention and not a list, **custom and extension tags fold for free**: `{% cache %}…{% endcache %}`, `{% form %}…{% endform %}`, a project's own `{% sidebar %}…{% endsidebar %}` — all fold with zero spec changes and zero hardcoded names. The intermediate clauses `{% elif %}`/`{% else %}` (in an `if`) and `{% else %}` (in a `for`) are not closers; the fold simply runs from the opener to its matching `end<name>`.

This is a *pure read* of tree-sitter delimiter spans ([E01](../foundations/E01-architecture.md) REQ-ARCH-07, [E30](../foundations/E30-extraction-and-indexing.md)): no template is rendered (P1), no tag is interpreted. An unmatched opener — a `{% for %}` with no `{% endfor %}` — leaves a name on the stack that never pairs, so it yields **no range at all**, never a fold running to end-of-file (P3, [E16](../foundations/E16-conventions.md)). The fold matches the *delimiter* structure, not the visual whitespace and not the tag's meaning.

## 4. Concepts & Definitions

- **Folding range** — a collapsible region. (Canonical definition in [glossary](../glossary.md).)
- **Structural fold** — a fold derived purely from delimiter pairing, with no knowledge of the tag's semantics.
- **Balanced pair** — an opener `{% name %}` matched, by name, to its `{% endname %}` closer via a stack. The unit of a structural region fold.
- **`end<name>` convention** — Jinja's universal rule that a block tag `{% name %}` closes with `{% endname %}`; the key the folder pairs on.
- **Region kind** — the LSP `FoldingRangeKind`: `region` for code-like regions, `comment` for comment blocks.
- **`startLine` / `endLine`** — the LSP `FoldingRange` boundaries, both **0-based** (line 0 is the file's first line); see §5.4.
- **Span** — a tree-sitter node's start/end position, the source of every fold.

## 5. Detailed Specification

### 5.1 The universal structural model — any balanced `{% name %}…{% endname %}` folds

Folding pairs delimiters; it never reads tag semantics.

**REQ-FOLD2-01 — Every balanced `{% name %}…{% endname %}` pair folds, with no hardcoded tag list.**

`textDocument/foldingRange` returns a `region` `FoldingRange` for **every** balanced opener/closer pair, matched by name off Jinja's `end<name>` convention. The folder walks the template's tags in source order, pushing each opener `name` onto a stack; when a `{% endname %}` matches the name on top of the stack, it pops and emits one range from the opener to the closer. The rule is keyed off the `end<name>` *convention*, **not** a list of known tags, so:

- The built-in block tags fold: `block`, `for`, `if`, `macro`, `call`, `filter`, `with`, `autoescape`, `trans`, `raw`, and any others sharing the convention.
- **Custom / extension tags fold identically and for free** — `{% cache %}…{% endcache %}`, `{% form %}…{% endform %}`, a project's own `{% sidebar %}…{% endsidebar %}` — with no spec or code change.
- Intermediate clauses are **not** closers: an `{% if %}` containing `{% elif %}`/`{% else %}` folds as **one** region from `if` to `endif`; a `{% for %}` containing an `{% else %}` folds from `for` to `endfor`. (Per-branch sub-folds are out of scope — §2.)
- `{% raw %}…{% endraw %}` folds as one region; its contents are literal text and produce no nested fold (the inner `{% … %}` are not real tags).

Ranges come from the tree-sitter delimiter spans ([E30](../foundations/E30-extraction-and-indexing.md)). Nested pairs each produce their own range, so an editor can fold an outer `{% for %}` and an inner `{% if %}` independently; the outer range encloses the inner.

A pair both of whose delimiters sit on the **same line** is not foldable (§5.4) — there is nothing to collapse.

### 5.2 Multi-line comments fold as `comment`

A `{# … #}` comment that spans more than one line folds.

**REQ-FOLD2-02 — A multi-line `{# … #}` comment folds with `kind = Comment`.**

A `{# … #}` whose opening `{#` and closing `#}` are on different lines yields a `FoldingRange` from its first line to its last line, carrying `kind = FoldingRangeKind.Comment`. A `{# … #}` entirely on one line is **not** foldable. The distinct `Comment` kind is what makes the editor's "Fold All Block Comments" act separately from "Fold All Regions" — every structural pair fold (§5.1) and multi-line-tag fold (§5.3) carries `kind = FoldingRangeKind.Region` instead.

### 5.3 A multi-line tag folds across its own lines

A single `{{ … }}` or `{% … %}` can itself span several lines; that span folds.

**REQ-FOLD2-03 — A multi-line `{{ … }}` or `{% … %}` tag folds across its lines, as `region`.**

When one delimiter pair — `{{ … }}` or `{% … %}` — opens on one line and closes on a later line (a long expression, a multi-line macro signature, a wrapped filter chain), it yields a `region` `FoldingRange` from its opening line to its closing line. This is independent of pair-folding (§5.1): the multi-line opener `{% macro f(a,\n b) %}` may *both* fold as a multi-line tag and, with its `{% endmacro %}`, fold as a balanced pair — the editor gets two ranges. A tag entirely on one line yields no multi-line-tag fold.

### 5.4 Fold boundaries — 0-based, and the closing line is what hides

One unambiguous convention, stated with a worked example.

**REQ-FOLD2-04 — `startLine` is the opener (stays visible); `endLine` is the closing line (collapses into the fold). Both are 0-based.**

A `FoldingRange` runs from `startLine` to `endLine`, and both are **0-based** per LSP (line 0 is the file's first line). The convention is fixed as:

- `startLine` = the **opener's line**. When the region is collapsed this line stays **visible** (it carries the fold chevron and the trailing ellipsis).
- `endLine` = the **closing tag's line** (or, for a comment/multi-line-tag, the line holding the closing `#}`/`}}`/`%}`). This line — and everything between it and the opener — **collapses into** the fold and is hidden when folded.

A pair, comment, or tag entirely on one line has `startLine == endLine` and is therefore **not** foldable: there is nothing to hide.

**Worked example (0-based).** Given this file (the gutter numbers an editor shows are **1-based**; LSP line numbers are **0-based**, so subtract one):

```
 editor gutter   LSP line   source
      1             0        {% block content %}
      2             1          <h1>{{ post.title }}</h1>
      3             2        {% endblock %}
```

The fold is `{ startLine: 0, endLine: 2, kind: Region }`. Collapsed, the user still sees editor-gutter line 1 (`{% block content %}`, LSP line 0) with a chevron; gutter lines 2–3 (LSP lines 1–2, the body **and** the `{% endblock %}`) are hidden. (This exact triple is pinned in §11.2.)

### 5.5 Host folding coexists (P5)

Jinja folds the Jinja layer; the host LSP folds its own.

**REQ-FOLD2-05 — Jinja and host folds coexist; jinja-lsp emits only Jinja-layer ranges.**

jinja-lsp returns folding ranges for the Jinja layer only (pairs, comments, multi-line tags) and never for the host language (HTML/SQL/text) — that stays with the host LSP and the editor (P5). The two responses **coexist**: an editor merges folding ranges from every server, so a Jinja `{% block %}` fold and an HTML `<section>` fold can overlap on the same lines, and the editor renders both chevrons independently. jinja-lsp neither suppresses nor depends on the host's ranges.

### 5.6 Unclosed and unbalanced openers yield no range

Never run a fold to end-of-file.

**REQ-FOLD2-06 — An unmatched opener yields no range (P3).**

Pairing is by name-matching the closer to its opener via the stack (§5.1). An opener whose `{% endname %}` is missing — a `{% for %}` with no `{% endfor %}`, or a `{% endfor %}` with no opener — never pairs: it leaves a name on (or pops nothing from) the stack and produces **no** `FoldingRange`. We emit no range rather than a fold running to end-of-file (P3, [E16](../foundations/E16-conventions.md) — partial input degrades, never corrupts). tree-sitter recovers the broken tag with a `MISSING`/`ERROR` node and [F01](F01-diagnostics.md) `JINJA-E001` flags the syntax error; well-formed pairs elsewhere in the file still fold normally.

## 6. UI Mockups

> **Gutter numbers below are 1-based**, as editors display them. LSP `FoldingRange.startLine`/`endLine` are **0-based** (§5.4) — subtract one to get the protocol value (gutter line 1 = LSP line 0).

### 6.1 A template with regions collapsed

`templates/blog/post.html` with the header comment, the `content` block, and a project-defined `{% cache %}` extension tag folded. The gutter shows the fold chevrons; collapsed lines render an ellipsis. Note the `cache` fold needs **no** special support — it pairs by the `end<name>` convention like any other tag (§5.1).

```
┌─ templates/blog/post.html ───────────────────────────────────────────────┐
│  1  ⊟ {# Post detail page — extends base, fills content. …  ⋯ #}  [comment]│
│  4    {% extends "base.html" %}                                           │
│  5    {% from "blog/macros.html" import post_url %}                       │
│  6  ⊟ {% block content %} ⋯ {% endblock %}                      [region]   │
│ 18  ⊟ {% cache 3600 "sidebar" %} ⋯ {% endcache %}               [region]   │
│ 24    {% block footer %}                                                  │
│ 25      <small>{{ post.author }}</small>                                  │
│ 26    {% endblock %}                                                      │
└───────────────────────────────────────────────────────────────────────────┘
  ⊟ collapsed (click to expand)    ⊞ expanded
  (custom {% cache %} folds via the end<name> convention — no hardcoded list)
```

### 6.2 The same block expanded, with a nested loop foldable

Expanding `content` reveals a `{% for %}` that folds on its own; the outer range encloses the inner.

```
  6  ⊞ {% block content %}                                        [region]
  7       <h1>{{ post.title }}</h1>
  8     ⊟ {% for comment in post.comments %} ⋯ {% endfor %}       [region]
 15    {% endblock %}
```

### 6.3 A multi-line tag folding on its own

A long `{% set %}` (or any tag) whose delimiters span several lines folds across them (§5.3), independent of any opener/closer pairing. Gutter is 1-based; the fold here is LSP `{ startLine: 7, endLine: 9, kind: Region }`.

```
  8  ⊟ {% set nav = [          ⋯ ] %}                            [region]
 11    {{ render_nav(nav) }}
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` opens with a multi-line `{# … #}` license/summary comment (folds as `comment`, §5.2), then a `{% block content %}` containing a `{% for comment in post.comments %}` loop — each its own region (§5.1): fold the whole block, or expand it and fold just the loop. A project that registers a `{% cache %}…{% endcache %}` extension tag gets that fold too, automatically, because folding pairs on the `end<name>` convention and never inspects what `cache` means (§5.1). The single-line `{% extends "base.html" %}` produces no fold (§5.4).

## 10. Edge Cases & Failure Modes

- **One-line pair / comment / tag** (`startLine == endLine`) → no fold (§5.4).
- **Custom / extension tag** (`{% cache %}…{% endcache %}`, `{% form %}…{% endform %}`) → folds as a `region` exactly like a built-in tag, via the `end<name>` convention; no hardcoded list, no special case (§5.1).
- **Unclosed opener** (`{% for %}` with no `{% endfor %}`) → never pairs; we emit **no** range rather than a fold running to end-of-file (P3, §5.6). tree-sitter recovers with a `MISSING`/`ERROR` node and [F01](F01-diagnostics.md) `JINJA-E001` flags the syntax error; well-formed pairs elsewhere still fold.
- **Stray closer** (`{% endfor %}` with no opener) → pops nothing from the stack; yields no range (§5.6).
- **Intermediate clauses** — `{% if %}…{% elif %}…{% else %}…{% endif %}` folds as **one** region `if`→`endif`; `{% for %}…{% else %}…{% endfor %}` folds `for`→`endfor`. The clauses are not closers (§5.1).
- **Deeply nested pairs** → each level folds independently; the outer range encloses the inner (§5.1).
- **`{% raw %}` containing what looks like tags** → folds as one `raw` region; its contents are literal text, so the inner `{% … %}` produce no nested fold (§5.1).
- **Whitespace-control markers** (`{%- for … -%}`) → are part of the same delimiter; they don't shift the fold's `startLine`/`endLine` (§5.1, §5.4).
- **Multi-line tag** — a single `{{ … }}`/`{% … %}` spanning lines folds on its own (§5.3); a multi-line opener may yield both a tag fold and a pair fold.
- **Host coexistence** — an HTML/SQL fold from the host LSP can overlap a Jinja fold on the same lines; both chevrons render, jinja-lsp emits only the Jinja range (§5.5, P5).
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → only a **multi-line** embedded region can fold; a single-line embedded template (`render_template_string("<h1>{{ post.title }}</h1>")`) is one line and yields no fold (needs ≥ 2 lines, §5.4). A multi-line embedded template's folds map to **host-file coordinates** (REQ-INLN-03).

## 11. Testing

Folding is verified by integration tests asserting exact ranges over fixture templates plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-FOLD2-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

Tests are organized by REQ. The structural model (§5.1) is exercised across built-in tags **and a custom tag** to prove no hardcoded list is involved. Synthetic `didOpen` documents supply constructs absent from the baseline fixture (custom `cache`, `raw`, `call`, `if/elif/else`, `for/else`, multi-line tag, deeply nested, whitespace-control, single-line variants, stray closer), per [E17-testing §5](../foundations/E17-testing.md#starlette-blog).

**Universal pair folding (REQ-FOLD2-01) — built-in, custom, intermediate clauses, nesting, raw:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `{% block content %}…{% endblock %}` in `post.html` yields one `region` from the `block` line to the `endblock` line | integration | starlette-blog | REQ-FOLD2-01 |
| `{% for c in post.comments %}…{% endfor %}` inside `content` yields a `region` from `for` line to `endfor` line | integration | starlette-blog | REQ-FOLD2-01 |
| `{% macro post_url(post) %}…{% endmacro %}` in `macros.html` (≥ 2 lines) yields a `region` from `macro` to `endmacro` | integration | starlette-blog | REQ-FOLD2-01 |
| **Custom tag** `{% cache 3600 "k" %}…{% endcache %}` (a name with no spec/code support) folds as a `region` via the `end<name>` convention | integration | synthetic doc | REQ-FOLD2-01 |
| `{% call comment_card(c) %}…{% endcall %}` (multi-line) yields a `region` from `call` to `endcall` | integration | synthetic doc | REQ-FOLD2-01 |
| `{% if %}…{% elif %}…{% else %}…{% endif %}` folds as **one** `region` `if`→`endif` (clauses are not closers; no per-branch sub-folds) | integration | synthetic doc | REQ-FOLD2-01 |
| `{% for %}…{% else %}…{% endfor %}` folds as one `region` `for`→`endfor` | integration | synthetic doc | REQ-FOLD2-01 |
| `{%- for c in post.comments -%}…{%- endfor -%}` folds by delimiter span; trim markers don't shift `startLine`/`endLine` | integration | synthetic doc | REQ-FOLD2-01 |
| Deeply nested `{% for %}` ▸ `{% if %}` ▸ `{% block %}` each yield an independent `region`; the outer range encloses the inner ranges | integration | synthetic doc | REQ-FOLD2-01 |
| `{% raw %}{% for %}…{% endfor %}{% endraw %}` yields exactly one `raw` `region`; the inner tag-like text produces no nested fold | integration | synthetic doc | REQ-FOLD2-01 |
| `post.html` block ▸ nested loop (§6.2): both ranges present, foldable independently | integration | starlette-blog | REQ-FOLD2-01 |

**Multi-line comments (REQ-FOLD2-02) — both polarities:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Multi-line `{# Post detail page — extends base… #}` header in `post.html` yields one fold from its first line to its last line, `kind = Comment` | integration | starlette-blog | REQ-FOLD2-02 |
| A one-line `{# … #}` comment yields no fold | integration | synthetic doc | REQ-FOLD2-02 |
| The `block`/`for` pair folds in `post.html` carry `kind = Region` (not Comment), confirming the comment kind is distinct | integration | starlette-blog | REQ-FOLD2-02 |

**Multi-line tag folding (REQ-FOLD2-03) — both polarities:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| A multi-line `{% set nav = [ … ] %}` spanning ≥ 2 lines yields a `region` from its opening line to its closing line | integration | synthetic doc | REQ-FOLD2-03 |
| A multi-line `{{ a + b + … }}` expression spanning ≥ 2 lines yields a `region` across its lines | integration | synthetic doc | REQ-FOLD2-03 |
| A multi-line `{% macro f(a,\n b) %}…{% endmacro %}` yields **both** a multi-line-tag fold (the opener) and a pair fold (opener→`endmacro`) | integration | synthetic doc | REQ-FOLD2-03, REQ-FOLD2-01 |
| A single-line `{{ post.title }}` yields no multi-line-tag fold | integration | starlette-blog | REQ-FOLD2-03 |

**Boundaries: 0-based, endLine collapses (REQ-FOLD2-04):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| **Pinned 0-based assertion:** the `{% block content %}` / body / `{% endblock %}` from §5.4 yields exactly `{ startLine: 0, endLine: 2, kind: Region }` (the `endblock` line collapses; the opener stays visible) | integration | synthetic doc | REQ-FOLD2-04 |
| Single-line `{% extends "base.html" %}` (`startLine == endLine`) yields no fold | integration | starlette-blog | REQ-FOLD2-04 |
| Single-line `{% if x %}…{% endif %}` on one line yields no fold | integration | synthetic doc | REQ-FOLD2-04 |

**Host coexistence (REQ-FOLD2-05):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `foldingRange` on `post.html` returns Jinja-layer ranges only (pairs/comment), never an HTML-element range — host folds are left to the host LSP | integration | starlette-blog | REQ-FOLD2-05 |
| A Jinja `{% block %}` fold and an overlapping HTML `<section>` region can both be present in the merged editor view; jinja-lsp's response carries only the Jinja range and does not suppress the host's | integration | starlette-blog | REQ-FOLD2-05 |

**Unmatched / unclosed openers (REQ-FOLD2-06):**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Unclosed `{% for %}` (MISSING `endfor`) yields no range — no run-to-EOF fold | integration | syntax-errors | REQ-FOLD2-06 |
| Unclosed `{% block %}` with no `{% endblock %}` yields no range | integration | syntax-errors | REQ-FOLD2-06 |
| Stray `{% endfor %}` with no opener pops nothing and yields no range | integration | synthetic doc | REQ-FOLD2-06 |
| An unclosed opener does not suppress a well-formed pair earlier in the same file | integration | syntax-errors | REQ-FOLD2-06, REQ-FOLD2-01 |

**Inline / E31 host coordinates:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| A **multi-line** embedded template containing `{% for %}…{% endfor %}` yields a `region` in **host-file** coordinates | integration | call-and-paths | REQ-FOLD2-01 |
| A **single-line** embedded template (`render_template_string("<h1>{{ post.title }}</h1>")`) is one line and yields no fold | integration | call-and-paths | REQ-FOLD2-04 |

**§6 layout state:**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| §6.1 layout: `post.html` returns the comment fold (Comment), the `content` block fold (Region), the custom `{% cache %}` fold (Region), and leaves the single-line `extends`/`from` unfolded — exactly the gutter shown | integration | starlette-blog + synthetic doc | REQ-FOLD2-01, REQ-FOLD2-02, REQ-FOLD2-04 |

### 11.3 Fixtures

- `starlette-blog` for the on-disk catalog (`block`, `for`, multi-line `{# #}`, `macro`, single-line `extends`/`from`, single-line `{{ }}`, the host-coexistence check); `syntax-errors` for the unclosed-opener recovery cases; `call-and-paths` for the inline/E31 host-coordinate cases (both the multi-line fold and the single-line no-fold). Constructs absent from those fixtures — the **custom `cache` tag**, `raw`, `call`, `if/elif/else`, `for/else`, the multi-line `{% set %}`/`{{ }}`/`macro` tags, the §5.4 0-based pin, deeply nested, whitespace-control, single-line `{% if %}`/`{# #}`, stray closer — use synthetic in-memory `didOpen` documents, per [E17-testing §5](../foundations/E17-testing.md#starlette-blog). Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-FOLD2-01 | universal pair-fold tests — built-in (`block`, `for`, `macro`, `call`), **custom `cache`**, `if/elif/else` and `for/else` one-region, whitespace-control span, deep-nesting independence, raw-literal, nested-loop independence; plus the multi-line-`macro` dual-fold, the inline multi-line host-coord fold, the unclosed-doesn't-suppress row, and the §6.1 layout row |
| REQ-FOLD2-02 | multi-line-`{# #}` Comment-kind test, one-line-comment negative, and the `block`/`for` Region-kind contrast |
| REQ-FOLD2-03 | multi-line `{% set %}` and `{{ }}` tag folds, the multi-line-`macro` dual-fold, and the single-line `{{ }}` negative |
| REQ-FOLD2-04 | the pinned `{ startLine: 0, endLine: 2 }` 0-based assertion, the single-line `extends` and one-line `if` negatives, and the single-line inline-template negative |
| REQ-FOLD2-05 | Jinja-only-ranges test and the host-overlap coexistence test |
| REQ-FOLD2-06 | unclosed-`for` and unclosed-`block` no-range tests, the stray-`endfor` no-range test, and the unclosed-doesn't-suppress test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the structural model and the kind mapping** — built-in pairs, a custom tag, multi-line comments, multi-line tags, nesting, host coexistence, and unmatched-opener handling — via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `foldingRange` on `post.html` (didOpen → request) | happy | ranges for the multi-line comment (Comment), `content` block (Region), and nested `for` loop (Region), each with correct **0-based** start/end (§6.1, §6.2) |
| E2E-02 | `foldingRange` over a `didOpen` doc holding a **custom** `{% cache %}…{% endcache %}` tag (no spec/code support for `cache`) | happy | one Region range opener→`endcache`, proving the `end<name>` convention folds custom tags with no hardcoded list |
| E2E-03 | `foldingRange` over a `didOpen` doc holding `macro`, `call`, and `raw` blocks | happy | three Region ranges, one per pair, span opener→closer; the `raw` body's tag-like text yields no nested fold |
| E2E-04 | `foldingRange` over a `didOpen` doc with `{% if %}…{% elif %}…{% else %}…{% endif %}` | happy | a single Region range `if`→`endif`; no per-branch sub-folds |
| E2E-05 | `foldingRange` over a `didOpen` doc with a multi-line `{% set nav = [ … ] %}` tag | happy | one Region range across the tag's lines (§5.3) |
| E2E-06 | `foldingRange` over a `didOpen` doc with a multi-line `{# … #}` and a one-line `{# … #}` | mixed | one Comment range for the multi-line comment; none for the one-line comment |
| E2E-07 | **Pinned 0-based assertion** — `foldingRange` over the §5.4 three-line `{% block %}` doc | happy | exactly `{ startLine: 0, endLine: 2, kind: Region }` |
| E2E-08 | `foldingRange` over a `didOpen` doc with deeply nested `for` ▸ `if` ▸ `block` | happy | three independent Region ranges; each outer encloses its inner |
| E2E-09 | `foldingRange` on a single-line `{% extends "base.html" %}` and a single-line `{% if x %}…{% endif %}` | negative | no ranges emitted for either |
| E2E-10 | `foldingRange` on `post.html`, asserting no HTML-element ranges are returned | coexistence | only Jinja-layer ranges present; host folds left to the host LSP (P5) |
| E2E-11 | `foldingRange` on an unclosed `{% for %}` (no `{% endfor %}`), with a well-formed `{% block %}` earlier | error path | no run-to-EOF range; the earlier `block` still folds; server stays healthy and responds to the next request |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; folds read tree-sitter spans only and never execute templates (P1).
- **Data sensitivity** — ranges describe only the open file's structure; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the fold chevrons and collapsed regions.

### 13.4 Performance & Scale

- **Latency** — folding is a single pass over the parse tree and returns in < 100 ms (P6), even for large templates.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1 (no rendering), P3 (no run-to-EOF on unmatched openers), P5 (host folds coexist), P6 (single-pass latency); [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — the tree-sitter delimiter spans; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers; [E07-data-model](../foundations/E07-data-model.md) — the delimiter structure whose spans fold; [E16-conventions](../foundations/E16-conventions.md) — partial-parse recovery behind the no-range-on-unclosed rule; [E31-inline-templates](../foundations/E31-inline-templates.md) — host-coordinate mapping for multi-line embedded regions.
- **Related:** [F10-symbols](F10-symbols.md) — the named-symbol structure; [F13-semantic-tokens](F13-semantic-tokens.md) — another tree-driven, span-based feature; [F01-diagnostics](F01-diagnostics.md) — `JINJA-E001` on the unclosed tag a fold declines to range.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-25** — Expanded §11.2 to one row per foldable construct kind (block, for, comment, macro, if, call, raw) in both polarities, plus rows for every §10 edge (unclosed-tag negatives, deep nesting, raw-literal, inline host-coords) and §6 states; rewrote §11.4 so each REQ lists its covering rows; expanded §12.2 to seven sequential E2E scenarios spanning happy, negative, and error paths.
- **2026-06-25** — **Redesigned to a universal, delimiter-structural folder (v0.2).** Folding no longer depends on per-tag semantics: §5.1 now folds **any** balanced `{% name %}…{% endname %}` pair via Jinja's `end<name>` convention with a name-matching stack and **no hardcoded tag list**, so built-in *and* custom/extension tags (`{% cache %}`, `{% form %}`) fold for free. Restructured the REQ set to six: FOLD2-01 universal pair fold, FOLD2-02 multi-line comment (`Comment`), FOLD2-03 multi-line `{{ }}`/`{% %}` tag fold, FOLD2-04 boundaries, FOLD2-05 host coexistence, FOLD2-06 unmatched-opener → no range. Fixed the review findings: stated `startLine`/`endLine` are **0-based** (mockup gutters are 1-based) and pinned `{ startLine: 0, endLine: 2 }`; resolved the boundary convention to `endLine` = the closing line that collapses while the opener stays visible, with a worked example, and fixed the contradictory §5.3 heading; added §5.5 host coexistence (P5); and descoped single-line inline folds while mapping multi-line embedded regions to host coordinates (§10, E31). Rewrote §1/§3/§4, updated §6 mockups (custom `{% cache %}` fold + 1-based gutter note + new §6.3 multi-line tag), §9, §10; rebuilt §11.2/§11.4 and expanded §12.2 to eleven scenarios. Added E16/E31/F01 to Depends/Related. Bumped to v0.2.
