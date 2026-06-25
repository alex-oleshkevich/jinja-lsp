# F08 — Go to Definition

> **Status:** Approved
>
> **Version:** 0.2   ·   **Last updated:** 2026-06-25
>
> **Purpose:** Jump from a Jinja symbol's usage site to where it's defined — a macro call to its `{% macro %}`, a template path to the file, a `from`-import to the macro it names, a block / `self` / `super` to the block declaration, an alias to its declaration, a scope-local variable to its binding, and a hinted symbol to its hint file.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F09-find-references](F09-find-references.md), [F10-symbols](F10-symbols.md), [F06-hover](F06-hover.md)

> Requirement tag: **DEF**

---

## 1. Purpose & Scope

You're reading `templates/blog/post.html`, your cursor lands on `post_url(post)`, and you want to see what that macro actually does. Pressing *Go to Definition* should drop you onto the `{% macro post_url(post) %}` line in `blog/macros.html` — not leave you guessing where it lives.

This spec defines `textDocument/definition`: which Jinja symbols jump, where they land, and — just as important — which symbols deliberately don't jump because the host-language LSP owns them.

This spec covers:

- The jump kinds: macro call; template path (incl. list/tuple includes); `from`-import name; block name / `self.<block>` / `super()`; import alias; scope-local variable; and a hinted symbol to its hint file.
- The negative contract — host-injected context variables and built-in callables don't jump.
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
- **Resolvable symbol** — a symbol jinja-lsp can statically resolve to a template-owned (or hint-backed) definition. Host-injected context variables are *not* resolvable (§5.8).
- **Scope-local** — a variable bound by an enclosing `{% for %}`/`{% set %}`/`{% with %}`/`{% call %}` or a macro parameter, with a `valid_range` ([E07](../foundations/E07-data-model.md) `VariableScope`).
- **`LocationLink`** — an LSP response that carries both the *origin* range (the word you clicked) and the *target* range, enabling editor peek UIs. (Canonical term in [glossary](../glossary.md).)

## 5. Detailed Specification

### 5.1 Macro call → macro definition

The everyday case: your cursor is on a macro call, and you want its `{% macro %}`.

**REQ-DEF-01 — A macro call jumps to its definition.**

When the cursor is on the callee of a macro call (`{{ post_url(post) }}` or `{% call post_url(post) %}`), resolve the macro through the [WorkspaceIndex](../glossary.md) and return the `MacroDefinition`'s name range ([E07](../foundations/E07-data-model.md)). The macro may be defined in the current template or imported from another — both resolve. If the call binds to no known macro, return nothing (the host LSP and [F01](F01-diagnostics.md)'s `JINJA-E103` handle that).

### 5.2 Template path → template file

The `extends`, `include`, `import`, and `from … import` tags all name a template by a string path. That path should jump to the file.

**REQ-DEF-02 — A template-path string jumps to the file.**

When the cursor is inside the path literal of an `{% extends "base.html" %}`, `{% include … %}`, `{% import … %}`, or `{% from … import … %}` tag, resolve the path against the configured templates directories ([E30](../foundations/E30-extraction-and-indexing.md)) and return a `Location` at the **start of the target file** (line 0, col 0). Inside a **list/tuple include** (`{% include ["a.html", "b.html"] %}`), the candidate under the cursor resolves; a candidate that doesn't resolve simply doesn't jump while its siblings still do. A dynamic path (`{% extends layout_var %}`, the `is_dynamic` flag on the `TemplateReference` — [E07](../foundations/E07-data-model.md)) is not statically resolvable, so it doesn't jump.

### 5.3 `from … import` name → macro definition

`{% from "blog/macros.html" import post_url %}` names a *specific* macro. The cursor can land on `post_url` here, not just on the path.

**REQ-DEF-03 — A `from`-import name jumps to the macro it imports.**

When the cursor is on an imported name in a `{% from X import Y %}` (or `Y as alias`) statement, resolve template `X`, find the `MacroDefinition` named `Y` inside it, and jump to that macro's definition — not merely to the top of `X`. This is more precise than the path jump in §5.2: it lands you on the exact macro.

### 5.4 Block, `self.<block>`, and `super()`

When a child overrides a block — or references another block through `self`/`super` — the author wants the block's declaration.

**REQ-DEF-04 — Block names, `self.<block>`, and `super()` jump to the block declaration.**

Three cursor positions resolve through the [template chain](../glossary.md) ([E07](../foundations/E07-data-model.md) `BlockDefinition`):

- On a block name in `{% block content %}` (or `{% endblock content %}`) that overrides an ancestor's block → jump to the **nearest ancestor** declaring it. A block introducing a new name (no ancestor declares it) doesn't jump.
- On a `self.<name>` reference (`{{ self.content() }}`) → jump to that block's definition, resolved through the chain.
- On `super()` inside an overriding block → jump to the **parent's** same-named block (the one `super()` renders). When the parent declares no such block, nothing jumps (consistent with [F01](F01-diagnostics.md)'s `JINJA-E401 invalid-super`).

### 5.5 Import alias → its declaration

`{% import "blog/macros.html" as macros %}` binds the alias `macros`. Using `{{ macros.post_url(post) }}` later, the cursor on `macros` should jump back to where the alias was declared.

**REQ-DEF-05 — An import alias jumps to its `import … as` declaration.**

When the cursor is on an alias usage (the namespace part of `macros.post_url`), jump to the `ImportAlias` declaration ([E07](../foundations/E07-data-model.md)) in the current template. If the cursor is on the *attribute* part (`post_url`), resolve through the alias to that macro's definition in the source template (a §5.1-style macro jump).

### 5.6 Scope-local variable → binding site

A variable bound inside the template is a definition jinja-lsp owns and can land you on.

**REQ-DEF-08 — A scope-local variable jumps to its binding site.**

A usage of a variable bound by an enclosing construct — a `{% for x in … %}` loop variable (and tuple-unpacking targets `{% for k, v in … %}`), a `{% set x = … %}` / `{% set x %}…{% endset %}` binding, a `{% with x = … %}` binding, a `{% call(arg) … %}` argument, or a macro parameter inside the macro body — jumps to that binding's definition site, chosen by the binding whose `valid_range` contains the cursor ([E07](../foundations/E07-data-model.md) `VariableScope`). These are template-owned bindings jinja-lsp resolves statically — the same ones [F11](F11-document-highlight.md) marks `Write` and [F06](F06-hover.md) hovers. Only host-injected context variables and free variables fall to the negative contract (§5.8).

### 5.7 Hinted symbol → its hint file

A symbol documented by the user's own hint jumps to where they declared it.

**REQ-DEF-09 — A hinted context variable jumps to its hint file.**

A hinted context variable (`post`) or one of its declared attributes jumps to its declaration in the originating hint file — a `*.hints.md` sidecar or a configured `hints` directory ([F04](F04-user-hints.md)). This requires the registry to record each hint's source location ([E07](../foundations/E07-data-model.md), [F04](F04-user-hints.md)). A hinted symbol whose source file can't be resolved falls through to the negative contract (§5.8). (Built-in and pack callables have no source file and don't jump — hover [F06](F06-hover.md) shows their docs instead; see [OQ-DEF-1](#15-open-questions--decisions) for custom builtins.)

### 5.8 Negative contract — host-owned symbols don't jump

This is the most important rule in the spec, because getting it wrong means stepping on the host LSP.

**REQ-DEF-06 — Host-owned and unresolvable symbols return nothing.**

A symbol jinja-lsp can't resolve to a template- or hint-backed definition returns an empty result: a host-injected context variable with **no** hint (`{{ request }}`, an un-hinted `{{ post }}`), an attribute on an unknown or un-hinted receiver (`{{ user.email }}`), and a built-in or pack callable (which has no source file). These are owned by the host Python LSP or have no definition site. Returning nothing here keeps jinja-lsp a [companion, not a replacement](../constitution.md) (P5): the Python LSP owns Python symbols, and we never compete for them.

> **Note:** "returns nothing" means an empty result, never an error. The editor falls through to its other providers cleanly.

### 5.9 Response shape

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
- Cursor on `content` in `{% block content %}` → jumps to `{% block content %}` in `base.html` (§5.4); on `{{ super() }}` inside it → the same parent block; on `{{ self.footer() }}` → `base.html`'s `footer` block.
- Cursor on `c` in `{% for c in post.comments %}…{{ c.body }}` → jumps to the loop binding `{% for c … %}` (§5.6).
- Cursor on `post` (a hinted `context_variable`) → jumps to its declaration in the `post` hint file (§5.7); on un-hinted `{{ request }}` → returns nothing, owned by the host (§5.8).

## 10. Edge Cases & Failure Modes

- **Macro shadowed by a local import** → resolve to the nearest binding the call actually uses (the import in the current template wins over a same-named macro elsewhere).
- **Block declared in the child but not the parent** → no jump; it's a new block, not an override.
- **`extends` chain with the block in a grandparent** → walk the chain and land on the nearest ancestor declaring it (§5.4).
- **Dynamic path (`is_dynamic`)** → no jump (§5.2); consistent with [F01](F01-diagnostics.md) REQ-DIAG-02 not flagging it.
- **Loop / `set` / `with` variable used outside its scope** → no binding's `valid_range` contains the cursor → nothing (§5.6).
- **`super()` where the parent declares no matching block** → nothing (consistent with [F01](F01-diagnostics.md) `JINJA-E401`).
- **`self.<name>` for a block no ancestor declares** → nothing (§5.4).
- **Hinted variable whose hint file is unresolvable, or a core/pack built-in** → nothing; only resolvable hints jump (§5.7).
- **Cursor inside an inline template** ([E31](../foundations/E31-inline-templates.md)) → jumps work identically; targets report host-file coordinates via the inline range map.
- **Unsaved edits in the target file** → resolve against the in-memory document, not the on-disk copy ([E01](../foundations/E01-architecture.md)).

## 11. Testing

Go-to-definition is verified by integration tests over the `starlette-blog` fixture and protocol journeys via `pytest-lsp`.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-DEF-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `post_url` in `{{ post_url(post) }}` resolves to `{% macro post_url %}` in `blog/macros.html`; a locally-defined macro resolves in-file; a `{% call x() %}` callee resolves | integration | starlette-blog | REQ-DEF-01 |
| `"base.html"` in `{% extends %}` resolves to `base.html` at line 0; the candidate under the cursor in a list-include resolves; a dynamic path does not | integration | starlette-blog, call-and-paths | REQ-DEF-02 |
| `post_url` in `{% from "blog/macros.html" import post_url %}` resolves to the macro itself, not the file top | integration | starlette-blog | REQ-DEF-03 |
| Child `{% block content %}` → nearest ancestor's `content`; `{{ self.footer() }}` → its block; `{{ super() }}` → the parent block; a new-name block and a `super()` with no parent block don't jump | integration | inheritance | REQ-DEF-04 |
| In `{{ macros.post_url(post) }}`: the `macros` namespace → its `{% import … as macros %}`; the `post_url` attribute → the macro definition | integration | starlette-blog | REQ-DEF-05 |
| Un-hinted `{{ request }}`, an attribute on an un-hinted receiver, and a built-in filter all return an empty result | integration | starlette-blog | REQ-DEF-06 |
| `LocationLink[]` (single element) when the client advertises `linkSupport`; else `Location[]`; an unresolved symbol returns `[]` | e2e (pytest-lsp) | starlette-blog | REQ-DEF-07 |
| `c` in `{% for c in post.comments %}{{ c.body }}` → the `{% for c … %}` binding; a `{% set %}`/`{% with %}`/macro-param use → its binding; the binding is chosen by `valid_range`; a use outside scope returns `[]` | integration | starlette-blog | REQ-DEF-08 |
| A hinted `post` (and a hinted attribute) → its `*.hints.md` declaration; an unresolvable hint returns `[]` | integration | user-hints | REQ-DEF-09 |

### 11.3 Fixtures

- `starlette-blog` for the macro/path/import/alias jumps; `inheritance` for child→parent block jumps. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-DEF-01 | macro-call jump test (local + imported + `{% call %}`) |
| REQ-DEF-02 | template-path + list-element jump test; dynamic-path negative |
| REQ-DEF-03 | from-import-name jump test |
| REQ-DEF-04 | block + `self.<block>` + `super()` jump tests; new-name and missing-parent negatives |
| REQ-DEF-05 | import-alias namespace + attribute jump test |
| REQ-DEF-06 | host-owned negative-contract test |
| REQ-DEF-07 | response-shape e2e (`LocationLink`/`Location`/empty) |
| REQ-DEF-08 | scope-local binding jump test (in-range + out-of-scope) |
| REQ-DEF-09 | hint-file jump test; unresolvable-hint negative |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the jump kinds and the negative contract**, exercised through `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Definition on `post_url` in `{{ post_url(post) }}` | happy | `Location`/`LocationLink` on `{% macro post_url %}` in `blog/macros.html` |
| E2E-02 | Definition on `"base.html"` in `{% extends %}` | happy | `Location` at `base.html` line 0 |
| E2E-03 | Definition on `post_url` in `{% from "blog/macros.html" import post_url %}` | happy | `Location` on the named macro, not the file top |
| E2E-04 | Definition on the child `{% block content %}` name | happy | `Location` on `base.html`'s `content` block |
| E2E-05 | Definition on `{{ super() }}` inside `content` | happy | `Location` on the parent `content` block |
| E2E-06 | Definition on `c` inside `{% for c in post.comments %}{{ c.body }}` | happy | `Location` on the `{% for c … %}` binding |
| E2E-07 | Definition on a hinted `post` | happy | `Location` in the `post` hint file |
| E2E-08 | Definition on the `macros` namespace in `{{ macros.post_url() }}` | happy | `Location` on its `{% import … as macros %}` |
| E2E-09 | Definition on un-hinted `{{ request }}` | negative | empty result, no error |
| E2E-10 | Definition on `c` referenced after its `{% endfor %}` | negative | empty result (outside `valid_range`) |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; resolution reads the syntax tree and the index only, never executing templates (P1). Template paths are resolved within configured directories; `../` escapes are rejected ([E30](../foundations/E30-extraction-and-indexing.md)).
- **Data sensitivity** — locations point only into the user's own workspace files; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders all navigation UI.

### 13.4 Performance & Scale

- **Latency** — definition is a pure index lookup and returns in < 100 ms (P6), well within budget since Pass 2 has already resolved the cross-template links.

## 15. Open Questions & Decisions

- **Decided** — definition prefers `LocationLink` (origin + target ranges) when the client supports it, falling back to `Location`; every jump resolves to a single target.
- **Decided** — scope-local variables (loop/`set`/`with`/macro-param) and hinted context variables jump (REQ-DEF-08/09); only host-injected context variables and built-in callables fall to the negative contract (REQ-DEF-06).
- **OQ-DEF-1** — should a **custom-builtin** filter/test/function ([F02 §5.6](F02-builtin-registry.md)) jump to the `.md` doc file it was loaded from? This is net-new and requires the registry to retain each doc's source path (which the doc model doesn't currently carry). Deferred; built-in, pack, and custom callables currently don't jump.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5, P6; [E07-data-model](../foundations/E07-data-model.md) — the symbol types, scopes, and `valid_range` resolved; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — path resolution and the import graph; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F09-find-references](F09-find-references.md) — the inverse direction; [F11-document-highlight](F11-document-highlight.md) — the file-local view of the same bindings; [F10-symbols](F10-symbols.md) — the same definitions as an outline; [F04-user-hints](F04-user-hints.md) — the hint files hinted jumps target; [F06-hover](F06-hover.md) — the definition's docs without jumping.

## 17. Changelog

- **2026-06-25** — v0.2: added scope-local jumps (REQ-DEF-08), `self.<block>`/`super()` jumps and list/tuple include paths (REQ-DEF-04, REQ-DEF-02), and hint-file jumps (REQ-DEF-09); narrowed the negative contract (REQ-DEF-06) to host-injected context variables and built-in callables; rebuilt the test and E2E plans. Deferred custom-builtin doc jumps (OQ-DEF-1).
- **2026-06-24** — `post_url`'s body shown as `url_for(...)`, matching F15/F16's depiction of the same macro.
- **2026-06-24** — Initial draft.
