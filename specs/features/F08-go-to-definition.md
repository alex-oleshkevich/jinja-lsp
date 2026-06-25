# F08 — Go to Definition

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Jump from a Jinja symbol's usage site to where it's defined — a macro call to its `{% macro %}`, a template path to the file, a `from`-import to the macro it names, a child block to its parent, an alias to its declaration.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F09-find-references](F09-find-references.md), [F10-symbols](F10-symbols.md), [F06-hover](F06-hover.md)

> Requirement tag: **DEF**

---

## 1. Purpose & Scope

You're reading `templates/blog/post.html`, your cursor lands on `post_url(post)`, and you want to see what that macro actually does. Pressing *Go to Definition* should drop you onto the `{% macro post_url(post) %}` line in `blog/macros.html` — not leave you guessing where it lives.

This spec defines `textDocument/definition`: which Jinja symbols jump, where they land, and — just as important — which symbols deliberately don't jump because the host-language LSP owns them.

This spec covers:

- The five jump kinds: macro call, template path, `from`-import name, child block, import alias.
- The negative contract — generic variables don't jump.
- The response shape — `Location` vs `LocationLink`.

## 2. Non-Goals / Out of Scope

- Finding every *usage* of a symbol — that's [F09-find-references](F09-find-references.md).
- The outline and workspace symbol search — [F10-symbols](F10-symbols.md).
- Resolving template paths and building the import graph — [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md).
- Hovering for the definition's docs without jumping — [F06-hover](F06-hover.md).

## 3. Background & Rationale

A Jinja template rarely stands alone. It `extends` a base, `import`s macros from a shared file, and overrides blocks declared elsewhere. Following those links by hand — opening the right file, scrolling to the right line — is exactly the friction an LSP removes.

We can do this because Pass 2 already resolves every cross-template reference into the [WorkspaceIndex](../glossary.md) ([E30](../foundations/E30-extraction-and-indexing.md)). Go-to-definition is a pure read of that index ([E01](../foundations/E01-architecture.md) REQ-ARCH-07): take the symbol under the cursor, look up its definition, return its location. No parsing, no execution (P1).

## 4. Concepts & Definitions

- **Jump target** — the definition a usage site resolves to: a macro, a template file, a block, or an import alias.
- **Resolvable symbol** — a symbol jinja-lsp can statically resolve from the index. Generic variables are *not* resolvable (§5.6).
- **`LocationLink`** — an LSP response that carries both the *origin* range (the word you clicked) and the *target* range, enabling editor peek UIs. (Canonical term in [glossary](../glossary.md).)

## 5. Detailed Specification

### 5.1 Macro call → macro definition

The everyday case: your cursor is on a macro call, and you want its `{% macro %}`.

**REQ-DEF-01 — A macro call jumps to its definition.**

When the cursor is on the callee of a macro call (`{{ post_url(post) }}` or `{% call post_url(post) %}`), resolve the macro through the [WorkspaceIndex](../glossary.md) and return the `MacroDefinition`'s name range ([E07](../foundations/E07-data-model.md)). The macro may be defined in the current template or imported from another — both resolve. If the call binds to no known macro, return nothing (the host LSP and [F01](F01-diagnostics.md)'s `JINJA-E103` handle that).

### 5.2 Template path → template file

The `extends`, `include`, `import`, and `from … import` tags all name a template by a string path. That path should jump to the file.

**REQ-DEF-02 — A template-path string jumps to the file.**

When the cursor is inside the path literal of an `{% extends "base.html" %}`, `{% include … %}`, `{% import … %}`, or `{% from … import … %}` tag, resolve the path against the configured templates directories ([E30](../foundations/E30-extraction-and-indexing.md)) and return a `Location` at the **start of the target file** (line 0, col 0). A dynamic path (`{% extends layout_var %}`, the `is_dynamic` flag on the `TemplateReference` — [E07](../foundations/E07-data-model.md)) is not statically resolvable, so it doesn't jump.

### 5.3 `from … import` name → macro definition

`{% from "blog/macros.html" import post_url %}` names a *specific* macro. The cursor can land on `post_url` here, not just on the path.

**REQ-DEF-03 — A `from`-import name jumps to the macro it imports.**

When the cursor is on an imported name in a `{% from X import Y %}` (or `Y as alias`) statement, resolve template `X`, find the `MacroDefinition` named `Y` inside it, and jump to that macro's definition — not merely to the top of `X`. This is more precise than the path jump in §5.2: it lands you on the exact macro.

### 5.4 Child block → parent block

When a child template overrides a block, the author usually wants to see what the parent's block looked like.

**REQ-DEF-04 — A block name in a child jumps to the same block in its parent.**

When the cursor is on a block name in `{% block content %}` and the template `extends` a parent that declares a block of the same name, jump to the parent's `{% block content %}` ([E07](../foundations/E07-data-model.md) `BlockDefinition`). Walk the [template chain](../glossary.md) upward and land on the **nearest** ancestor that declares the block. A block that introduces a new name (no ancestor declares it) doesn't jump.

### 5.5 Import alias → its declaration

`{% import "blog/macros.html" as macros %}` binds the alias `macros`. Using `{{ macros.post_url(post) }}` later, the cursor on `macros` should jump back to where the alias was declared.

**REQ-DEF-05 — An import alias jumps to its `import … as` declaration.**

When the cursor is on an alias usage (the namespace part of `macros.post_url`), jump to the `ImportAlias` declaration ([E07](../foundations/E07-data-model.md)) in the current template. If the cursor is on the *attribute* part (`post_url`), resolve through the alias to that macro's definition in the source template (a §5.1-style macro jump).

### 5.6 Negative contract — generic variables don't jump

This is the most important rule in the spec, because getting it wrong means stepping on the host LSP.

**REQ-DEF-06 — Only Jinja-resolvable symbols jump; generic variables return nothing.**

A bare variable reference (`{{ post }}`, `{{ user.email }}`, a `{% for %}` loop variable, a `{% set %}` target's later use) does **not** jump. These are either context variables injected by host code (invisible to static analysis — P1) or locals whose definition is a different concern. Returning nothing here keeps jinja-lsp a [companion, not a replacement](../constitution.md) (P5): the Python LSP owns Python symbols, and we never compete for them.

> **Note:** "returns nothing" means an empty result, never an error. The editor falls through to its other providers cleanly.

### 5.7 Response shape

**REQ-DEF-07 — Prefer `LocationLink`; fall back to `Location`.**

When the client advertises `textDocument.definition.linkSupport`, return a `LocationLink[]` so the editor can show the origin word and a peek of the target. Otherwise return `Location[]`. A jump always resolves to exactly one target, so the array holds a single element; an unresolved symbol returns an empty array.

## 6. UI Mockups

### 6.1 Jumping from a macro call to its definition

The cursor sits on `post_url` in `post.html`; *Go to Definition* opens `macros.html` at the macro. The origin word is underlined; the target line is highlighted.

```
┌─ templates/blog/post.html ───────────────────────────────────────────────┐
│  1 │ {% extends "base.html" %}                                            │
│  2 │ {% from "blog/macros.html" import post_url %}                        │
│  3 │ {% block content %}                                                  │
│  4 │   <a href="{{ post_url(post) }}">{{ post.title }}</a>                │
│    │                  ‾‾‾‾‾‾‾‾ ⌖ cursor — Go to Definition                │
│  5 │ {% endblock %}                                                       │
└───────────────────────────────────────────────────────────────────────────┘
                                   │  jumps to
                                   ▼
┌─ templates/blog/macros.html ─────────────────────────────────────────────┐
│  1 │ {% macro post_url(post) %}                                           │
│    │           ███████ ◄── definition (target range highlighted)         │
│  2 │   {{ url_for("post", slug=post.slug) }}                             │
│  3 │ {% endmacro %}                                                       │
└───────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Peek view (LocationLink)

With `linkSupport`, the same jump can render inline as a peek without leaving the file.

```
  4 │   <a href="{{ post_url(post) }}">…</a>
    │                ╰─ peek: blog/macros.html:1
    │      ┌───────────────────────────────────────────────┐
    │      │ {% macro post_url(post) %}                     │
    │      │   {{ url_for("post", slug=post.slug) }}        │
    │      │ {% endmacro %}                                 │
    │      └───────────────────────────────────────────────┘
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` opens with `{% extends "base.html" %}` and `{% from "blog/macros.html" import post_url %}`:

- Cursor on `"base.html"` → jumps to the top of `templates/base.html` (§5.2).
- Cursor on `post_url` in the import → jumps to `{% macro post_url(post) %}` in `blog/macros.html` (§5.3).
- Cursor on `post_url` in `{{ post_url(post) }}` → same macro definition (§5.1).
- Cursor on `content` in `{% block content %}` → jumps to `{% block content %}` in `base.html` (§5.4).
- Cursor on `post` in `{{ post_url(post) }}` → returns nothing; `post` is a hinted context variable owned by the host (§5.6).

## 10. Edge Cases & Failure Modes

- **Macro shadowed by a local import** → resolve to the nearest binding the call actually uses (the import in the current template wins over a same-named macro elsewhere).
- **Block declared in the child but not the parent** → no jump; it's a new block, not an override.
- **`extends` chain with the block in a grandparent** → walk the chain and land on the nearest ancestor declaring it (§5.4).
- **Dynamic path (`is_dynamic`)** → no jump (§5.2); consistent with [F01](F01-diagnostics.md) REQ-DIAG-02 not flagging it.
- **Cursor inside an inline template** ([E31](../foundations/E31-inline-templates.md)) → jumps work identically; targets report host-file coordinates via the inline range map.
- **Unsaved edits in the target file** → resolve against the in-memory document, not the on-disk copy ([E01](../foundations/E01-architecture.md)).

## 11. Testing

Go-to-definition is verified by integration tests over the `starlette-blog` fixture and protocol journeys via `pytest-lsp`.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-DEF-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Macro call resolves to its `{% macro %}` (local + imported) | integration | starlette-blog | REQ-DEF-01 |
| `extends`/`include`/`import` path resolves to the file start | integration | starlette-blog | REQ-DEF-02 |
| `from X import Y` name resolves to macro `Y` in `X` | integration | starlette-blog | REQ-DEF-03 |
| Child block name resolves to nearest parent block | integration | inheritance | REQ-DEF-04 |
| Import alias resolves to its `import … as` declaration | integration | starlette-blog | REQ-DEF-05 |
| Generic variable returns an empty result | integration | starlette-blog | REQ-DEF-06 |
| `LocationLink` returned when client advertises `linkSupport`; else `Location` | e2e (pytest-lsp) | starlette-blog | REQ-DEF-07 |

### 11.3 Fixtures

- `starlette-blog` for the macro/path/import/alias jumps; `inheritance` for child→parent block jumps. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-DEF-01 | macro-call jump test |
| REQ-DEF-02 | template-path jump test |
| REQ-DEF-03 | from-import-name jump test |
| REQ-DEF-04 | child→parent block jump test |
| REQ-DEF-05 | import-alias jump test |
| REQ-DEF-06 | negative-contract test |
| REQ-DEF-07 | response-shape e2e |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the jump kinds and the negative contract**, exercised through `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Definition on a macro call | happy | `Location`/`LocationLink` at the macro definition |
| E2E-02 | Definition on an `extends` path | happy | `Location` at the target file's start |
| E2E-03 | Definition on a `from`-import name | happy | `Location` at the named macro |
| E2E-04 | Definition on a child block name | happy | `Location` at the parent block |
| E2E-05 | Definition on a generic variable | negative | empty result, no error |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; resolution reads the syntax tree and the index only, never executing templates (P1). Template paths are resolved within configured directories; `../` escapes are rejected ([E30](../foundations/E30-extraction-and-indexing.md)).
- **Data sensitivity** — locations point only into the user's own workspace files; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders all navigation UI.

### 13.4 Performance & Scale

- **Latency** — definition is a pure index lookup and returns in < 100 ms (P6), well within budget since Pass 2 has already resolved the cross-template links.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5, P6; [E07-data-model](../foundations/E07-data-model.md) — the symbol types resolved; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — path resolution and the import graph; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F09-find-references](F09-find-references.md) — the inverse direction; [F10-symbols](F10-symbols.md) — the same definitions as an outline; [F06-hover](F06-hover.md) — the definition's docs without jumping.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-24** — `post_url`'s body shown as `url_for(...)`, matching F15/F16's depiction of the same macro.
