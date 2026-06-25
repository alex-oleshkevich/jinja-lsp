# F01 — Diagnostics

> **Status:** Approved
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** The diagnostics catalog — 21 diagnostic checks — and the `noqa` inline-suppression mechanism, shared by the LSP server and the `check` CLI.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md)   ·   **Related:** [F19-cli-linter](F19-cli-linter.md), [F04-user-hints](F04-user-hints.md), [F17-code-actions](F17-code-actions.md), [E15-app-config](../foundations/E15-app-config.md)

> Requirement tag: **DIAG**

---

## 1. Purpose & Scope

Diagnostics are the heart of jinja-lsp: the checks that turn a silent runtime error into a squiggle while you type. This spec defines every diagnostic code, what triggers it, and how users suppress findings inline with `noqa`.

This spec covers:

- The diagnostic checks (codes per the constitution §4.2 scheme).
- The one new check this spec owns: `JINJA-W107 invalid-noqa`.
- How `lint.select` / `lint.ignore` filter findings.
- The `noqa` inline-suppression directives.
- Per-file (Pass 1) vs cross-file (Pass 2) classification.

`JINJA-W106 unknown-attribute` is the 21st code but is owned by [F04-user-hints](F04-user-hints.md) (it only fires against hinted attribute lists).

## 2. Non-Goals / Out of Scope

- The fixes for these diagnostics — owned by [F17-code-actions](F17-code-actions.md).
- The CLI rendering of diagnostics — owned by [F19-cli-linter](F19-cli-linter.md).
- Config key mechanics (`select`/`ignore` parsing, discovery) — owned by [E15-app-config](../foundations/E15-app-config.md).
- The `unknown-attribute` check — owned by [F04-user-hints](F04-user-hints.md).

## 3. Background & Rationale

Jinja errors usually surface only at render time, often in production. jinja-lsp catches 21 classes of them statically, each with a stable slug, plus inline `noqa` suppression and the `invalid-noqa` check that keeps suppressions honest.

## 4. Concepts & Definitions

- **Check** — one rule that consumes extracted facts and emits zero or more diagnostics.
- **Per-file vs cross-file** — a check runs in Pass 1 (one `TemplateIndex`) or Pass 2 (the `WorkspaceIndex`). (See [E01](../foundations/E01-architecture.md).)
- **`noqa` directive** — an inline suppression comment. (Canonical definition in [glossary](../glossary.md).)

## 5. Detailed Specification

### 5.1 The catalog

Each check below lists its code, the facts it consumes ([E07](../foundations/E07-data-model.md)), and its pass. Output always shows `code slug`, e.g. `JINJA-E101 undefined-variable`.

| Code | Slug | Fires when | Pass |
|---|---|---|---|
| `JINJA-E001` | `syntax-error` | tree-sitter reports an `ERROR`/`MISSING` node | 1 |
| `JINJA-E101` | `undefined-variable` | an identifier resolves to no in-scope variable, macro, global, or hinted context var | 2 |
| `JINJA-E102` | `undefined-filter` | a `\|` filter isn't a built-in, pack, custom, or hinted filter | 1 |
| `JINJA-E103` | `undefined-function` | a called name isn't a known macro, global, or hinted function | 2 |
| `JINJA-E104` | `undefined-test` | an `is` test isn't a known built-in/custom/hinted test | 1 |
| `JINJA-W201` | `unused-variable` | a `{% set %}`/loop/macro-param variable is never referenced in its scope | 1 |
| `JINJA-W202` | `unused-macro` | a defined macro is never called or exported | 2 |
| `JINJA-W203` | `unused-import` | an `import`/`from-import` alias is never used | 2 |
| `JINJA-W301` | `duplicate-block` | two blocks share a name in the same template | 1 |
| `JINJA-W302` | `duplicate-macro` | two macros share a name in the same template | 1 |
| `JINJA-W303` | `duplicate-import-alias` | two imports bind the same alias | 1 |
| `JINJA-W304` | `duplicate-from-import` | a name is `from`-imported twice | 1 |
| `JINJA-W305` | `name-shadowing` | a variable shadows an outer-scope name | 1 |
| `JINJA-E401` | `invalid-super` | `super()` is called where no parent block exists | 2 |
| `JINJA-W402` | `unreachable-content` | a child template has renderable content outside any block | 2 |
| `JINJA-E403` | `missing-required-block` | a child doesn't override a parent block marked `required` | 2 |
| `JINJA-E404` | `recursive-import` | an import/extends chain forms a cycle | 2 |
| `JINJA-E501` | `wrong-call-args` | a macro/function call's args don't match the definition's params | 2 |
| `JINJA-E601` | `template-does-not-exist` | an `extends`/`include`/`import` path resolves to no template | 2 |
| `JINJA-W107` | `invalid-noqa` | a `noqa` references a code/prefix that doesn't exist (this spec, §5.4) | 1 |

**REQ-DIAG-01 — Slugs are exact.** Every diagnostic has a stable kebab-case slug; new codes follow the same kebab style.

### 5.2 Path-check edge cases (E601)

**REQ-DIAG-02 — Don't flag intentionally-absent or dynamic paths.**

`JINJA-E601 template-does-not-exist` must NOT fire for `{% include "x" ignore missing %}` (the `ignore_missing` flag on the `TemplateReference`) nor for a dynamic path like `{% extends layout_var %}` (the `is_dynamic` flag). Both flags come from [E07](../foundations/E07-data-model.md). This is P4 — we only flag what's positively wrong.

### 5.3 Configuration

**REQ-DIAG-03 — `select`/`ignore` filter by code or class prefix.**

`lint.select` and `lint.ignore` accept a full code (`JINJA-E101`) or a class prefix (`JINJA-E1`, `JINJA-W`) — never a slug (constitution §4.2). All checks are enabled by default **except** `JINJA-W106`, which is opt-in ([F04](F04-user-hints.md)). When `select` is set, only the listed codes run; `ignore` subtracts from the active set.

### 5.4 `noqa` inline suppression

Jinja has no `#` line comments, so suppression rides inside Jinja comments (whitespace-control `{#- -#}` is accepted too).

**REQ-DIAG-04 — `noqa` directive forms.**

- `{# noqa #}` — suppress **all** diagnostics on the line.
- `{# noqa: JINJA-E101, JINJA-W2 #}` — suppress only the listed IDs. Canonical separator is `:`; a bare space (`{# noqa JINJA-E101 #}`) is tolerated. Each ID is a full code or class prefix — **not** a slug.
- `{# noqa-file #}` / `{# noqa-file: JINJA-W2 #}` — at the top of the file, suppress all / listed codes for the whole file.

**REQ-DIAG-05 — Scope model (ruff-style).**

A line-level `noqa` suppresses diagnostics whose primary range is on the **same physical line** as the comment. For a tag spanning lines, a `noqa` on the reported line **or** on the line of the tag's opening delimiter both count. Suppression is applied after computing diagnostics and after config `select`/`ignore`, before results are stored — so pull-mode clients see suppressed results too. The `noqa` comment never triggers a diagnostic itself.

**REQ-DIAG-06 — Invalid IDs raise `JINJA-W107`.**

A `noqa` that references a code or class prefix that doesn't exist — including a bare slug, which isn't valid input — raises `JINJA-W107 invalid-noqa` on the directive, so typos don't silently fail to suppress. Valid IDs in the same directive still apply. There is deliberately **no** `unused-noqa` check (a `noqa` suppressing nothing is fine).

## 6. UI Mockups

### 6.1 Diagnostic squiggle + hover (editor)

How a finding appears inline, with its code, slug, and message.

```
templates/blog/post.html
  4 │ {{ post.titel }}
    │    ~~~~~~~~~~
    │    ╰─ JINJA-E101 undefined-variable: 'post.titel' is not defined
    │       did you mean 'title'?  (quick fix available — F17)
```

### 6.2 `noqa` suppression in source

```
  4 │ {{ undefined_global }}   {# noqa: JINJA-E101 #}  ← E101 suppressed on line 4
  1 │ {# noqa-file: JINJA-W2 #}                        ← all 2xx suppressed file-wide
  7 │ {{ x }}  {# noqa: JINJA-E999 #}                  ← raises JINJA-W107 invalid-noqa
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` calls `{{ post_url(post) }}` where `post_url` is imported from `blog/macros.html`. If the import is removed, Pass 2 raises `JINJA-E103 undefined-function` at the call. If a developer writes `{{ post.titel }}`, Pass 2 raises `JINJA-E101 undefined-variable` (the `post` context var is hinted; `titel` isn't an attribute). A trailing `{# noqa: JINJA-E101 #}` silences just that line.

## 10. Edge Cases & Failure Modes

- **Half-typed expression** → tree-sitter recovers; only `JINJA-E001` fires, not a cascade of undefined-symbol errors (P3).
- **`noqa` with a mix of valid and invalid IDs** → valid IDs suppress; invalid IDs raise `JINJA-W107`.
- **`select` and `ignore` overlap** → `ignore` wins for the overlapping code; [E15](../foundations/E15-app-config.md) warns about the overlap.
- **`super()` in a non-extending template** → `JINJA-E401`.
- **Import cycle a→b→a** → `JINJA-E404` once per cycle, not per edge.

## 11. Testing

Each code has a dedicated broken fixture with a golden `expected-diagnostics.json`; `noqa` behavior is unit- and e2e-tested.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-DIAG-NN` maps to a test; every code in §5.1 has a fixture that triggers it. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Each code fires on its fixture; golden diff matches | golden (check) | undefined-vars, unused-symbols, duplicates, inheritance, call-and-paths, syntax-errors | REQ-DIAG-01 |
| `ignore missing` + dynamic `extends` suppress E601 | unit + golden | call-and-paths | REQ-DIAG-02 |
| `select`/`ignore` by code and class prefix | unit | starlette-blog | REQ-DIAG-03 |
| `noqa`, `noqa: CODE`, `noqa-file` suppress correctly | unit + e2e | user-hints, starlette-blog | REQ-DIAG-04, REQ-DIAG-05 |
| Invalid `noqa` ID raises W107; valid IDs still apply | unit | user-hints | REQ-DIAG-06 |
| Server and `check` report identical codes/ranges | integration | starlette-blog | REQ-DIAG-01 |

### 11.3 Fixtures

- One broken fixture per code class, each with `expected-diagnostics.json`. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-DIAG-01 | per-code golden fixtures + parity test |
| REQ-DIAG-02 | E601 edge-case test |
| REQ-DIAG-03 | select/ignore unit tests |
| REQ-DIAG-04 | noqa-forms unit tests |
| REQ-DIAG-05 | noqa-scope unit + e2e |
| REQ-DIAG-06 | invalid-noqa unit test |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of diagnostic scope** through both E2E branches: golden `check` for the catalog, `pytest-lsp` for publish behavior.

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Open a file with an undefined variable | happy | `publishDiagnostics` includes `JINJA-E101` at the right range |
| E2E-02 | Add a `{# noqa #}` and save | happy | the finding clears via an updated publish |
| E2E-03 | `check` over each broken fixture | happy | json output equals the golden file |
| E2E-04 | Typo a code in a `noqa` | error | `JINJA-W107` appears on the directive |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; checks read the syntax tree only and never execute it (P1).
- **Data sensitivity** — diagnostics quote only the user's own source; nothing leaves the machine.

### 13.4 Performance & Scale

- **Latency** — per-file checks run within Pass 1's budget; cross-file checks run in the debounced Pass 2 (< 2 s for 500 templates, per [E30](../foundations/E30-extraction-and-indexing.md)).

## 15. Open Questions & Decisions

- **Decided** — no `unused-noqa` check (keeps the surface small). `JINJA-W106` is opt-in. TCP transport dropped (server-wide, ADR-009).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — the code scheme; [E07-data-model](../foundations/E07-data-model.md) — the facts checks consume; [E30-extraction-and-indexing](../foundations/E30-extraction-and-indexing.md) — when checks run.
- **Related:** [F19-cli-linter](F19-cli-linter.md) — CLI rendering; [F17-code-actions](F17-code-actions.md) — the fixes; [F04-user-hints](F04-user-hints.md) — `W106` and hinted symbols; [E15-app-config](../foundations/E15-app-config.md) — `select`/`ignore`.

## 17. Changelog

- **2026-06-24** — Initial draft: the 21-check catalog + `invalid-noqa`, the `noqa` directive forms and scope model, and E601 edge cases.
