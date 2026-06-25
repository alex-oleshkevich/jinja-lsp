# F09 — Find References

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Given a macro, block, or import definition, find every place it's used across the whole workspace — the inverse of go-to-definition, and the reference graph the [F17](F17-code-actions.md) rename command builds on.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F08-go-to-definition](F08-go-to-definition.md), [F11-document-highlight](F11-document-highlight.md), [F16-call-hierarchy](F16-call-hierarchy.md), [F15-code-lens](F15-code-lens.md)

> Requirement tag: **REF**

---

## 1. Purpose & Scope

You're about to change the `post_url` macro's signature, and the first question is: who calls it? *Find References* answers that — every call site across every template, listed in one panel, so you can see the blast radius before you touch a line.

This spec defines `textDocument/references`: which symbols have findable references, how usages are collected from the [WorkspaceIndex](../glossary.md), and how `includeDeclaration` controls whether the definition itself is part of the list.

This spec covers:

- The symbol kinds with workspace-wide references: macros, blocks, imports.
- Collecting usages across files from the `Reference` data.
- The `includeDeclaration` flag.
- The negative contract — only Jinja-resolvable symbols, never generic variables.

## 2. Non-Goals / Out of Scope

- Jumping to a single definition — that's [F08-go-to-definition](F08-go-to-definition.md).
- File-local occurrence highlighting on cursor-rest — [F11-document-highlight](F11-document-highlight.md) (the same data, a narrower scope).
- The call graph as a navigable tree — [F16-call-hierarchy](F16-call-hierarchy.md).
- The "N references" lens — [F15-code-lens](F15-code-lens.md) (it counts what this spec collects).
- Performing the rename edit itself — owned by [F17-code-actions](F17-code-actions.md)'s rename command, which consumes this reference graph (§3). F09 supplies the references; F17 rewrites them.

## 3. Background & Rationale

Find-references is cheap for us because the work is already done. Pass 1 extracts a `Reference` for every usage site — identifier, attribute access, filter, function call, test ([E07](../foundations/E07-data-model.md)) — and Pass 2 resolves those references to their definitions across the workspace ([E30](../foundations/E30-extraction-and-indexing.md)). Finding references is a pure read of that resolved graph ([E01](../foundations/E01-architecture.md) REQ-ARCH-07): pick the symbol, return every reference that points at it.

> **Note:** This reference graph is exactly what **rename** needs — rewrite every reference plus the declaration. The workspace-rename command in [F17](F17-code-actions.md) is built directly on it: it resolves the symbol under the cursor, then rewrites the declaration and every reference this graph returns.

## 4. Concepts & Definitions

- **Reference** — a usage site of a symbol. (Canonical definition in [glossary](../glossary.md).)
- **Declaration** — the symbol's definition site (the `{% macro %}`, `{% block %}`, or `{% import %}`).
- **`includeDeclaration`** — the LSP request flag controlling whether the declaration itself appears in the result list.

## 5. Detailed Specification

### 5.1 What has references

Three Jinja symbol kinds resolve across the workspace, so they're the ones that answer find-references.

**REQ-REF-01 — Macros, blocks, and imports have workspace-wide references.**

Given the cursor on a definition or a usage of:

- a **macro** — return every call site (`{{ m(…) }}`, `{% call m(…) %}`) and every `from … import` that names it, across all templates;
- a **block** — return every override of that block in descendant templates, plus the declaration in each ([template chain](../glossary.md));
- an **import** (alias or `from`-import name) — return every usage of the imported symbol in the importing template.

References are collected from the resolved `Reference` set in the [WorkspaceIndex](../glossary.md) ([E07](../foundations/E07-data-model.md)).

### 5.2 Workspace-wide collection

References don't stop at the current file — a macro defined once is called from many templates.

**REQ-REF-02 — Collection spans the whole workspace.**

When asked for a macro's or block's references, scan every `TemplateIndex` in the workspace, not just the current file. Each result is a `Location` (URI + range) at the usage's identifier range. Results are returned in a stable order — by URI, then by position — so editors and tests see deterministic output.

> **Note:** A multi-folder workspace resolves references **within** each folder; cross-folder references aren't linked ([E30](../foundations/E30-extraction-and-indexing.md)). This matches how Pass 2 builds one `WorkspaceIndex` per folder.

### 5.3 The `includeDeclaration` flag

The LSP request carries a flag for whether the definition belongs in the list. Some users want "all 7 usages"; others want "the definition plus its 6 callers."

**REQ-REF-03 — Honor `includeDeclaration`.**

When `context.includeDeclaration` is `true`, the result includes the symbol's declaration range alongside its usages. When `false`, only usages are returned. The declaration is the `MacroDefinition`/`BlockDefinition`/`ImportAlias` name range ([E07](../foundations/E07-data-model.md)).

### 5.4 Negative contract — only Jinja-resolvable symbols

Like go-to-definition, find-references stays in its lane.

**REQ-REF-04 — Generic variables have no references; return an empty result.**

A bare variable (`{{ post }}`, `{{ user.email }}`, a loop or `{% set %}` variable) returns an empty result — these are host-owned context variables or locals, not Jinja-resolvable symbols (P5). Returning nothing lets the editor fall through to the host LSP cleanly. An empty result is never an error.

## 6. UI Mockups

### 6.1 References panel — a macro's usages across files

Find References on `post_url` lists its declaration and every call site, grouped by file. The declaration carries a marker; usages are plain.

```
┌─ References to  post_url  (6) ────────────────────────────────────────────┐
│                                                                            │
│  ▾ templates/blog/macros.html                                             │
│      1   {% macro post_url(post) %}            ◆ declaration              │
│                                                                            │
│  ▾ templates/blog/post.html                                               │
│      2   {% from "blog/macros.html" import post_url %}                    │
│      4   <a href="{{ post_url(post) }}">{{ post.title }}</a>              │
│      9   {{ post_url(related) }}                                           │
│                                                                            │
│  ▾ templates/email/digest.html                                            │
│      3   {% from "blog/macros.html" import post_url %}                    │
│     12   {{ post_url(post) }}                                              │
│                                                                            │
│  [ includeDeclaration: ☑ ]   ⏎ open   ⇧⏎ open to the side                 │
└───────────────────────────────────────────────────────────────────────────┘
```

### 6.2 A block's overrides across the inheritance chain

References on the `content` block surface every template that overrides it.

```
┌─ References to  block content  (3) ──────────────────────────────────────┐
│  ▾ templates/base.html                                                    │
│      8   {% block content %}{% endblock %}     ◆ declaration              │
│  ▾ templates/blog/post.html                                               │
│      3   {% block content %}                   override                   │
│  ▾ templates/email/digest.html                                            │
│      5   {% block content %}                   override                   │
└───────────────────────────────────────────────────────────────────────────┘
```

## 9. Examples & Use Cases

In `starlette-blog`, `post_url` is defined in `blog/macros.html` and imported with `from … import` into both `blog/post.html` and `email/digest.html`, where it is called three times (twice in `post.html`, once in `digest.html`). Find References on the macro name returns all six sites — declaration, the two import bindings (REQ-REF-01), and the three calls — when `includeDeclaration` is on, and five when it's off.

Find References on the `content` block (declared in `base.html`) returns its declaration plus every child override (§5.1). Find References on `post` — a hinted context variable — returns nothing (§5.4).

## 10. Edge Cases & Failure Modes

- **A macro never used** → returns just the declaration (with `includeDeclaration`) or an empty list (without). [F01](F01-diagnostics.md)'s `JINJA-W202 unused-macro` flags this separately.
- **A macro imported under different aliases in different files** → all usages resolve to the one definition; every alias's call sites are collected.
- **A block overridden several levels deep** → every override in the chain is a reference, not only the immediate child.
- **Cursor on a usage rather than the definition** → resolve to the definition first, then collect — the result is identical regardless of where you invoke it.
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → references inside inline regions are collected too, reported in host-file coordinates.
- **Unresolved macro call** → no definition to anchor on; returns an empty result (and [F01](F01-diagnostics.md) `JINJA-E103` flags the call).

## 11. Testing

Find-references is verified by integration tests over multi-file fixtures plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-REF-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| **REQ-REF-01 — macro kind** | | | |
| Cursor on `{% macro post_url(post) %}` decl in `macros.html` → all 3 call sites (`post_url(post)` and `post_url(related)` in `post.html` line 4/9, `post_url(post)` in `digest.html`) plus both `from … import post_url` binding sites (`post.html`, `digest.html`) collected | integration | starlette-blog | REQ-REF-01 |
| `{% call … %}` call site of a macro is collected — cursor on a macro defined and invoked via `{% call comment_card(c) %}…{% endcall %}` returns the `{% call %}` site as a reference | integration | synthetic doc (a `{% macro %}` + `{% call %}` pair, not in baseline) | REQ-REF-01 |
| `{{ m(…) }}` call site of a macro is collected — cursor on `comment_card` decl in `macros.html` returns the `comment_card(c, show_actions=true)` call in `post.html`'s comment loop | integration | starlette-blog | REQ-REF-01 |
| `from … import` binding name is itself a reference — cursor on `post_url` in `{% from "blog/macros.html" import post_url %}` (`post.html`) resolves to the macro and the binding range is in the result set | integration | starlette-blog | REQ-REF-01 |
| **REQ-REF-01 — block kind** | | | |
| Cursor on `{% block content %}` decl in `base.html` → declaration plus the `content` overrides in `post.html` and `digest.html` | integration | starlette-blog | REQ-REF-01 |
| Block overridden several levels deep — every override in a 3-level chain (grandparent decl, parent override, child override) is returned, not only the immediate child (§10) | integration | inheritance | REQ-REF-01 |
| **REQ-REF-01 — import kind** | | | |
| Import (`from`-import name) usages — cursor on the imported `post_url` returns every usage of that name within the importing template (`digest.html`: the call on line 12) | integration | starlette-blog | REQ-REF-01 |
| Import alias slot (`{% import "x" as y %}`) — cursor on alias `y` returns every `y.member` usage in the importing template | integration | synthetic doc (`{% import … as y %}` alias slot, not in baseline) | REQ-REF-01 |
| **REQ-REF-02 — workspace collection & ordering** | | | |
| Macro references span the whole workspace — `post_url` usages in both `post.html` and `digest.html` are collected, not just the current file | integration | starlette-blog | REQ-REF-02 |
| Each result is a `Location` at the usage's identifier range (URI + range covers the name token only, not the whole tag) | integration | starlette-blog | REQ-REF-02 |
| Results ordered by URI, then by position — deterministic output across runs | integration | starlette-blog | REQ-REF-02 |
| Multi-folder isolation — a macro defined in folder A is **not** reported at a same-named usage in folder B; references resolve within each folder only (§5.2 Note) | integration | large-workspace (multi-folder) | REQ-REF-02 |
| **REQ-REF-03 — includeDeclaration toggle** | | | |
| `includeDeclaration: true` on `post_url` → declaration range present alongside the 5 usages (6 total) | integration | starlette-blog | REQ-REF-03 |
| `includeDeclaration: false` on `post_url` → only the 5 usages, declaration absent | integration | starlette-blog | REQ-REF-03 |
| Declaration range is the name range of the `MacroDefinition`/`BlockDefinition`/`ImportAlias`, not the whole construct | integration | starlette-blog | REQ-REF-03 |
| **REQ-REF-04 — negative contract** | | | |
| Bare context variable (`{{ post }}`) returns an empty result, not an error | integration | starlette-blog | REQ-REF-04 |
| Attribute access (`{{ user.email }}`) returns an empty result | integration | synthetic doc (`{{ user.email }}`) | REQ-REF-04 |
| Loop variable (`{% for c in post.comments %}` → cursor on `c`) returns an empty result | integration | starlette-blog | REQ-REF-04 |
| `{% set %}` variable returns an empty result | integration | synthetic doc (`{% set x = … %}{{ x }}`) | REQ-REF-04 |
| **§10 edges & §6 states** | | | |
| Macro never used, `includeDeclaration: true` → just the declaration | integration | unused-symbols | REQ-REF-01, REQ-REF-03 |
| Macro never used, `includeDeclaration: false` → empty list | integration | unused-symbols | REQ-REF-03, REQ-REF-04 |
| Macro imported under different aliases in different files → all call sites across both aliases resolve to the one definition and are collected | integration | synthetic docs (two importers, distinct alias names) | REQ-REF-01, REQ-REF-02 |
| Cursor on a usage rather than the definition → resolve-then-collect yields the identical result set as cursor-on-definition | integration | starlette-blog | REQ-REF-01 |
| Inline template (E31) — a macro usage inside an inline region is collected and reported in host-file coordinates | integration | call-and-paths (inline cases) | REQ-REF-01, REQ-REF-02 |
| Unresolved macro call — cursor on a call with no resolvable definition returns an empty result (no anchor) | integration | call-and-paths | REQ-REF-04 |
| §6.1 panel data — macro result groups usages by file and tags the declaration distinctly from plain usages (declaration carries the decl marker; usages are plain) | integration | starlette-blog | REQ-REF-01, REQ-REF-03 |
| §6.2 panel data — block result tags the base declaration vs. each override distinctly | integration | starlette-blog | REQ-REF-01 |

### 11.3 Fixtures

- `starlette-blog` for macro/import references; `inheritance` for block-override references. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-REF-01 | macro-kind tests (`{{ m() }}`, `{% call %}`, `from … import` binding), block-kind tests (overrides incl. deep chain), import-kind tests (`from`-import name + alias slot) |
| REQ-REF-02 | workspace-span, identifier-range `Location`, URI-then-position ordering, and multi-folder isolation tests |
| REQ-REF-03 | includeDeclaration true/false toggle + declaration-name-range tests |
| REQ-REF-04 | negative-contract tests (bare var, attribute, loop var, `{% set %}` var, unresolved call) |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of reference collection and the declaration toggle**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | References on the `post_url` macro **definition** in `macros.html`, `includeDeclaration: true` (starlette-blog) | happy | declaration + both `from … import` bindings + 3 call sites across `post.html` and `digest.html` (6 locations), ordered by URI then position |
| E2E-02 | Same macro, `includeDeclaration: false` (starlette-blog) | happy | the 5 usages only (2 bindings + 3 calls); declaration absent |
| E2E-03 | References on the `post_url` macro from a **usage** site (a call in `post.html`), `includeDeclaration: true` (starlette-blog) | happy | identical 6-location set as E2E-01 — invocation point doesn't change the result |
| E2E-04 | References on the `content` **base block** in `base.html` (starlette-blog) | happy | declaration + the `post.html` and `digest.html` overrides, each tagged distinctly |
| E2E-05 | References on a `{% call comment_card(c) %}` site collected alongside `{{ }}` calls (synthetic `didOpen` doc with a `{% macro %}` + `{% call %}` pair) | happy | both the `{% call %}` and any `{{ }}` invocation returned as references |
| E2E-06 | References on a generic context variable (`{{ post }}`) (starlette-blog) | negative | empty result, no error |
| E2E-07 | References on an attribute access (`{{ user.email }}`) (synthetic `didOpen` doc) | negative | empty result, no error |
| E2E-08 | References on a macro that is never called, `includeDeclaration: false` (unused-symbols) | negative | empty list (no usages), no error |
| E2E-09 | References on an unresolved macro call — no definition to anchor on (call-and-paths) | error | empty result; the call is flagged separately by F01 `JINJA-E103` |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; reference collection reads the index only and never executes templates (P1).
- **Data sensitivity** — locations point only into the user's own workspace; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the references panel.

### 13.4 Performance & Scale

- **Latency** — collection is a pure scan of the already-resolved `Reference` set and returns in < 100 ms (P6); the workspace is indexed within the 2 s budget ([E30](../foundations/E30-extraction-and-indexing.md)).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5, P6; [E07-data-model](../foundations/E07-data-model.md) — the `Reference` data; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — cross-file resolution; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F08-go-to-definition](F08-go-to-definition.md) — the inverse direction; [F11-document-highlight](F11-document-highlight.md) — the file-local counterpart; [F16-call-hierarchy](F16-call-hierarchy.md) — the call graph; [F15-code-lens](F15-code-lens.md) — the reference count.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-24** — `digest.html` references `post_url` via `from … import` (was an alias); mockup and §9 now count both import bindings plus three calls (6 with declaration), consistent with F15/F16.
- **2026-06-25** — Expanded §11.2 and §12.2 to full combinatorial coverage: each REQ-REF sub-behavior (macro `{{ }}`/`{% call %}`/`from … import` kinds, block deep-chain overrides, import alias slot, identifier-range `Location`, multi-folder isolation, declaration-name-range, every §5.4 variable form), every §10 edge (never-used, multi-alias, deep override, cursor-on-usage, inline, unresolved call), and both §6 panel states now map to concrete test rows; §11.4 lists every REQ once; §12.2 adds happy/negative/error journeys (E2E-01–09).
