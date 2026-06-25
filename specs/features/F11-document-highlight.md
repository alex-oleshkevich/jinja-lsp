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

Each row is a concrete cursor position over a named construct, with the exact expected highlight set. "Synthetic doc" = an in-memory `didOpen` document for a construct not present in the `starlette-blog` baseline ([E17 §5](../foundations/E17-testing.md#starlette-blog)).

| # | Construct / cursor · fixture | Type | Expected outcome | Verifies |
|---|---|---|---|---|
| **REQ-HL-01 — which symbols highlight** | | | | |
| 1 | Cursor on `post` loop variable in `{% for post … %}` · `email/digest.html` | integration | a `DocumentHighlight[]` covering the `for` target + each in-loop `post` | REQ-HL-01 |
| 2 | Cursor on `{% set %}` target name · synthetic doc (`{% set total = 0 %}…{{ total }}`) | integration | highlights at the `set` target + each `total` read | REQ-HL-01 |
| 3 | Cursor on a macro parameter inside its body · synthetic doc (`{% macro m(words) %}{{ words }}{% endmacro %}`) | integration | highlights at the parameter decl + each `words` use in the body | REQ-HL-01 |
| 4 | Cursor on a locally-defined macro name `post_url` · `blog/macros.html` | integration | highlights at the `{% macro %}` name + any same-file call | REQ-HL-01 |
| 5 | Cursor on a block name `content` · `base.html` | integration | highlights at the `{% block content %}` name (+ `endblock` label / overrides per Pass 1 facts) | REQ-HL-01 |
| 6 | Cursor on an import alias `y` · synthetic doc (`{% import "x" as y %}{{ y.z }}`) | integration | highlights at the alias slot + each `y` use | REQ-HL-01 |
| **REQ-HL-02 — file-local, scope-aware collection** | | | | |
| 7 | Cursor on `post_url` call in `blog/post.html` (defined in `blog/macros.html`) | integration | only the same-file occurrence highlights; the def in `macros.html` is **not** included | REQ-HL-02 |
| 8 | Cursor on inner-scope `post` loop var when an outer `post` of the same name exists · synthetic doc (`{% for post in xs %}{{ post }}{% endfor %}{{ post }}`) | integration | only the in-loop occurrences highlight; the outer-scope `post` stays dark (distinct symbol) | REQ-HL-02 |
| 9 | Cursor on `post` in `email/digest.html` loop; unrelated `request` global present | integration | `post` occurrences highlight; `request` is untouched (§6.1) | REQ-HL-02 |
| **REQ-HL-03 — write vs read kinds** | | | | |
| 10 | Cursor on the `for` target `post` · `email/digest.html` | integration | the `for` target is `kind = Write`; every in-loop `post` is `kind = Read` (§6.1) | REQ-HL-03 |
| 11 | Cursor on a locally-defined macro `m` with one call · synthetic doc | integration | the `{% macro %}` name is `Write`; the call is `Read` (§6.2) | REQ-HL-03 |
| 12 | Cursor on imported macro `post_url`'s call · `email/digest.html` (def lives in `blog/macros.html`) | integration | all occurrences are `Read`; no `Write` in this file | REQ-HL-03 |
| 13 | Cursor on a symbol with a single occurrence: a never-read `{% set x = 1 %}` · synthetic doc | integration | one highlight, `kind = Write` (the lone definition) | REQ-HL-03 |
| 14 | Cursor on a symbol used exactly once with no local def (imported, single call) · synthetic doc | integration | one highlight, `kind = Read` (the sole usage) | REQ-HL-03 |
| **REQ-HL-04 — non-symbol positions** | | | | |
| 15 | Cursor on host-language HTML text (e.g. `<h2>`) · `email/digest.html` | integration | empty result, no error | REQ-HL-04 |
| 16 | Cursor on a delimiter (`{{`, `{%`) · synthetic doc | integration | empty result, no error | REQ-HL-04 |
| 17 | Cursor on whitespace inside an expression · synthetic doc | integration | empty result, no error | REQ-HL-04 |
| **§10 edges** | | | | |
| 18 | Cursor on a symbol in a broken template (unclosed tag) where Pass 1 extracted only some occurrences · `syntax-errors` | integration | highlights whatever Pass 1 extracted; missing occurrences simply absent, no error (P3) | REQ-HL-02 |
| 19 | Cursor on a symbol inside an inline/embedded template region · `call-and-paths` ([E31](../foundations/E31-inline-templates.md)) | integration | highlights stay within the inline region, returned in host-file coordinates | REQ-HL-02 |

### 11.3 Fixtures

- `starlette-blog`, reusing `email/digest.html` for the loop-variable case, `blog/macros.html` for the locally-defined macro, `blog/post.html` for the cross-file import call, and `base.html` for the block. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).
- `syntax-errors` for the broken-template edge (row 18) and `call-and-paths` for the inline/embedded edge (row 19), both registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).
- Synthetic in-memory `didOpen` documents for constructs absent from the baseline: `{% set %}` target, macro parameter, `{% import … as %}` alias, single-occurrence symbols, and delimiter/whitespace cursor positions (per [E17 §5](../foundations/E17-testing.md#starlette-blog)).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-HL-01 | rows 1–6 — loop var, `set` target, macro param, local macro, block, import alias |
| REQ-HL-02 | rows 7–9, 18, 19 — file-local only, scope-aware, broken-template, inline |
| REQ-HL-03 | rows 10–14 — write/read kinds, imported-elsewhere all-`Read`, single-occurrence write & read |
| REQ-HL-04 | rows 15–17 — host text, delimiter, whitespace |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of highlight collection and the write/read distinction**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Cursor on a loop variable (`post` in `email/digest.html`) | happy | `Write` on the `for` target, `Read` on each in-loop usage |
| E2E-02 | Cursor on a `{% set %}` target (synthetic doc) | happy | `Write` on the `set` target, `Read` on each subsequent use |
| E2E-03 | Cursor on a locally-defined macro (`post_url` in `blog/macros.html`) | happy | `Write` on the `{% macro %}` name, `Read` on its same-file call |
| E2E-04 | Cursor on a block name (`content` in `base.html`) | happy | `Write` on the `{% block %}` name; only same-file occurrences |
| E2E-05 | Cursor on an import alias (synthetic `{% import "x" as y %}`) | happy | `Write` on the alias slot, `Read` on each `y` use |
| E2E-06 | Cursor on an imported macro's call (`post_url` in `email/digest.html`, defined elsewhere) | happy | all occurrences `Read`, no `Write` |
| E2E-07 | Cursor on an inner-scope `post` while an outer `post` exists (synthetic doc) | happy | only the in-loop occurrences highlight; the outer `post` stays dark |
| E2E-08 | Cursor on a single-occurrence definition (`{% set x = 1 %}`, never read) | happy | one highlight, `kind = Write` |
| E2E-09 | Cursor on host-language HTML text | negative | empty result, no error |
| E2E-10 | Cursor on a delimiter (`{{` / `{%`) | negative | empty result, no error |
| E2E-11 | Cursor on whitespace inside an expression | negative | empty result, no error |
| E2E-12 | Cursor on a symbol in a broken template (`syntax-errors`) | negative | highlights only what Pass 1 extracted; no error (P3) |

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
- **2026-06-25** — Expanded §11.2 to 19 concrete rows and §12.2 to 12 E2E scenarios for full combinatorial coverage (Write/Read kinds; loop var / set target / macro param / local macro / block / import alias; defined-in-file vs imported-elsewhere; scope-distinct names; single-occurrence write & read; host-text / delimiter / whitespace negatives; broken-template and inline §10 edges). Rebuilt §11.4 as a one-row-per-REQ bijection.
