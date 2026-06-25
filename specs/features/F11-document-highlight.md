# F11 — Document Highlight

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** When the cursor rests on a Jinja symbol, highlight every occurrence of it in the current file — distinguishing the write (definition) from the reads (usages). The file-local, automatic counterpart to find-references.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F09-find-references](F09-find-references.md), [F08-go-to-definition](F08-go-to-definition.md), [F10-symbols](F10-symbols.md)

> Requirement tag: **HL**

---

## 1. Purpose & Scope

Rest your cursor on a variable name and every other place it appears in the same file lights up — softly, automatically, no command needed. That's document highlight: the editor's quiet way of showing you "here's everywhere this thing is used right here." It's the file-local sibling of [F09](F09-find-references.md), which spans the whole workspace on demand.

This spec defines `textDocument/documentHighlight`: which symbols highlight, how occurrences are found within the one file, and how a definition is marked as a *write* versus a usage as a *read*.

This spec covers:

- The symbols that highlight: variables, macros, blocks, imports.
- File-local occurrence collection from the existing `Reference` data.
- The write (definition) vs read (usage) highlight kinds.

## 2. Non-Goals / Out of Scope

- Workspace-wide reference search — [F09-find-references](F09-find-references.md). Document highlight never leaves the current file.
- Jumping to a definition — [F08-go-to-definition](F08-go-to-definition.md).
- The outline of the file — [F10-symbols](F10-symbols.md).

## 3. Background & Rationale

Document highlight reuses data that already exists. Pass 1 extracts a `Reference` for every usage site and a definition for every macro, block, variable, and import in the file ([E07](../foundations/E07-data-model.md)). Highlighting is a pure read of that one `TemplateIndex` ([E01](../foundations/E01-architecture.md) REQ-ARCH-07): find the symbol under the cursor, collect its same-file occurrences, tag each as read or write.

Because it's file-local, it doesn't need Pass 2's cross-template resolution — which is what makes it cheap enough to run on every cursor move. Where [F09](F09-find-references.md) walks the whole `WorkspaceIndex`, F11 walks one file's facts.

## 4. Concepts & Definitions

- **Highlight kind** — the LSP `DocumentHighlightKind`: `Write` for a definition, `Read` for a usage, `Text` when neither applies.
- **Occurrence** — one appearance of a symbol's name in the current file.
- **Reference** — a usage site. (Canonical definition in [glossary](../glossary.md).)

## 5. Detailed Specification

### 5.1 What highlights

Document highlight covers more than find-references does, because *within one file* even a local variable's occurrences are worth showing.

**REQ-HL-01 — Highlight variables, macros, blocks, and imports.**

When the cursor rests on the name of a variable (`{% set %}` target, loop variable, macro parameter), a macro, a block, or an import alias, collect every occurrence of that name in the **current file** and return a `DocumentHighlight[]`. Unlike [F09](F09-find-references.md), local variables *do* highlight here — the scope is one file, so there's no risk of stepping on the host LSP across the workspace.

### 5.2 File-local collection

Occurrences come from the file's own extracted facts, never from re-scanning text.

**REQ-HL-02 — Collect occurrences from the current `TemplateIndex` only.**

Occurrences are the definition plus every `Reference` to the symbol within the same file ([E07](../foundations/E07-data-model.md)). Collection never crosses into other files — that's [F09](F09-find-references.md)'s job. Each occurrence is a range at the symbol's name. A variable is resolved within its [variable scope](../glossary.md), so a `post` loop variable and an outer `post` are treated as distinct symbols.

### 5.3 Write vs read kinds

Marking the definition differently from its usages helps the eye find "where this starts."

**REQ-HL-03 — Mark the definition `Write`, usages `Read`.**

The symbol's definition occurrence (the `{% macro %}`, `{% block %}`, `{% set %}`, `{% import … as %}` name) is returned with `kind = Write`. Every usage is `kind = Read`. When no definition exists in the file (an imported macro used but defined elsewhere), all occurrences are `Read`. Editors typically tint writes and reads differently, so this distinction is visible.

### 5.4 Negative behavior

Document highlight stays as quiet as it is helpful.

**REQ-HL-04 — Non-symbol positions return nothing.**

When the cursor isn't on a highlightable Jinja symbol — in host-language text, on a delimiter, on whitespace — return an empty result, never an error. An unrecognized position simply produces no highlights.

## 6. UI Mockups

### 6.1 A variable highlighted across the file

The cursor rests on `post` inside the loop body; the loop variable's definition (the `{% for %}` target) is boxed as a write, and each read is underlined. The outer-scope `request` is untouched — different symbol.

```
┌─ templates/email/digest.html ────────────────────────────────────────────┐
│  1 │ {% for post in request.state.posts %}                                │
│    │         ▓▓▓▓  ◄── write (loop variable defined here)                  │
│  2 │   <h2>{{ post.title }}</h2>                                          │
│    │          ‾‾‾‾  ◄── read                                              │
│  3 │   <p>{{ post.body | truncate(40) }}</p>                             │
│    │         ‾‾‾‾  ◄── read                                               │
│  4 │   <a href="{{ post_url(post) }}">more</a>                           │
│    │                       ‾‾‾‾  ◄── read                                 │
│  5 │ {% endfor %}                                                         │
│                                                                            │
│  ▓ write (definition)    ‾ read (usage)                                    │
└───────────────────────────────────────────────────────────────────────────┘
```

### 6.2 A macro highlighted (definition + call)

Resting on `excerpt` boxes the `{% macro %}` name and underlines its one call.

```
  1 │ {% macro excerpt(post, words) %}{{ post.body }}{% endmacro %}
  2 │ {{ excerpt(post, 40) }}
       ▓▓▓▓▓▓▓ (write)        ‾‾‾‾‾‾‾ (read)
```

## 9. Examples & Use Cases

Editing `templates/email/digest.html` in `starlette-blog`, you rest the cursor on the `post` loop variable. Its definition in `{% for post in … %}` highlights as a write; each `post.title`, `post.body`, and `post_url(post)` in the loop highlights as a read (§5.1, §5.3). The unrelated `request` global stays dark. Resting on `post_url` (imported, defined elsewhere) highlights only its call site as a read, since the definition isn't in this file (§5.3).

## 10. Edge Cases & Failure Modes

- **Two variables sharing a name in different scopes** → only the occurrences in the cursor's scope highlight; the other scope's `post` is a distinct symbol (§5.2).
- **Imported macro used but not defined locally** → all occurrences are `Read`; no `Write` in this file (§5.3).
- **Cursor on host-language text** → empty result (§5.4).
- **A symbol used once and never else** → a single highlight (its definition or its sole usage).
- **Broken template** → highlight whatever Pass 1 extracted; missing occurrences are simply absent (P3).
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → highlights stay within the inline region, in host-file coordinates.

## 11. Testing

Document highlight is verified by integration tests over the `starlette-blog` fixture plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-HL-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Variable, macro, block, import each highlight their occurrences | integration | starlette-blog | REQ-HL-01 |
| Occurrences collected from current file only, scope-aware | integration | starlette-blog | REQ-HL-02 |
| Definition marked `Write`, usages `Read` | integration | starlette-blog | REQ-HL-03 |
| Imported macro (no local def) → all `Read` | integration | starlette-blog | REQ-HL-03 |
| Non-symbol cursor position returns empty | integration | starlette-blog | REQ-HL-04 |

### 11.3 Fixtures

- `starlette-blog`, reusing `email/digest.html` for the loop-variable case. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-HL-01 | per-symbol-kind highlight test |
| REQ-HL-02 | file-local + scope test |
| REQ-HL-03 | write/read kind test |
| REQ-HL-04 | empty-position test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of highlight collection and the write/read distinction**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Cursor on a loop variable | happy | write on the `for` target, reads on each usage |
| E2E-02 | Cursor on a locally-defined macro | happy | write on the `{% macro %}`, read on the call |
| E2E-03 | Cursor on an imported macro's call | happy | a single `Read`, no write |
| E2E-04 | Cursor on host-language text | negative | empty result, no error |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; highlights read the index only and never execute templates (P1).
- **Data sensitivity** — ranges point only into the open file; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor renders the highlight tints.

### 13.4 Performance & Scale

- **Latency** — highlight runs on every cursor move, so it must be fast; reading one `TemplateIndex` keeps it well under the 100 ms budget (P6).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P3, P6; [E07-data-model](../foundations/E07-data-model.md) — the `Reference` data reused; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F09-find-references](F09-find-references.md) — the workspace-wide counterpart, sharing the same reference data; [F08-go-to-definition](F08-go-to-definition.md) — jumping to the write; [F10-symbols](F10-symbols.md) — the file's symbols as an outline.

## 17. Changelog

- **2026-06-24** — Initial draft.
