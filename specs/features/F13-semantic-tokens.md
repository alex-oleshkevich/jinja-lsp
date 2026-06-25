# F13 — Semantic Tokens

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Color Jinja by *meaning*, not just syntax — a known macro distinct from an unknown variable, a built-in filter from a user filter — using a token legend the editor maps to theme colors.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [F02-builtin-registry](F02-builtin-registry.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F12-folding-range](F12-folding-range.md), [F04-user-hints](F04-user-hints.md), [F06-hover](F06-hover.md)

> Requirement tag: **SEM**

---

## 1. Purpose & Scope

Plain syntax highlighting colors a name by its *shape* — it can tell `{{ post_url }}` is an identifier, but not whether `post_url` is a macro you defined, a built-in, or a typo. Semantic tokens add the meaning: the language server knows `post_url` resolves to a macro, `truncate` is a built-in filter, and `psot` resolves to nothing — and colors each accordingly.

This spec defines `semanticTokens/full` and `semanticTokens/range`, and — the load-bearing part — the **token legend**: the explicit list of token types and modifiers an editor theme maps to colors.

This spec covers:

- The token legend: every token type and modifier, defined in a table.
- The two requests: `semanticTokens/full` and `semanticTokens/range`.
- How tokens are derived from the index (resolved vs unresolved, built-in vs user).
- Why delta updates are deferred.

## 2. Non-Goals / Out of Scope

- Syntax-level highlighting (delimiters, keywords by shape) — that's the editor's tree-sitter/TextMate grammar; semantic tokens *augment* it.
- The diagnostics that flag unresolved symbols — [F01-diagnostics](F01-diagnostics.md). Semantic tokens *color* an unknown variable; F01 *squiggles* it.
- `semanticTokens/full/delta` — deferred (§5.4).
- Choosing the actual colors — that's the editor theme's job; we only classify.

## 3. Background & Rationale

The classifications that matter most for Jinja are exactly the ones a context-free grammar can't make. "Is this name a macro that exists?" requires the [WorkspaceIndex](../glossary.md). "Is this filter a built-in or one the user hinted?" requires the [built-in registry](../glossary.md) ([F02](F02-builtin-registry.md)). Those are *semantic* facts, and semantic tokens are the LSP channel for shipping them to the theme.

Computing them is a pure read ([E01](../foundations/E01-architecture.md) REQ-ARCH-07): walk the file's references ([E07](../foundations/E07-data-model.md)), resolve each against the index, and emit a token with the type and modifiers that describe what it resolved to. A macro that resolves gets the `macro` type with a `defined` modifier; a variable that resolves to nothing gets `variable` with `unknown`. The theme does the rest.

## 4. Concepts & Definitions

- **Semantic token** — a classification attached to a span. (Canonical definition in [glossary](../glossary.md).)
- **Legend** — the ordered lists of token *types* and *modifiers* the server declares at `initialize`; tokens are encoded as indices into these lists.
- **Token type / modifier** — the *what* (a `macro`) and the *qualifiers* (`defined`, `builtin`).

## 5. Detailed Specification

### 5.1 The token legend

The legend is the contract: the editor learns these names once at `initialize` and maps each to a theme color or text style. This is the heart of the spec.

**REQ-SEM-01 — Declare this token-type legend.**

The server declares exactly these token types, in this order (the index into the list is the wire encoding):

| # | Token type | Meaning | Example | Distinguishes what syntax can't |
|---|---|---|---|---|
| 0 | `macro` | a macro name (definition or call) | `post_url` | macro vs ordinary identifier |
| 1 | `variable` | a variable / identifier reference | `post` | resolvable vs unknown (via modifiers) |
| 2 | `parameter` | a macro parameter name | `words` in `macro excerpt(post, words)` | a param vs a free variable |
| 3 | `filter` | a `\|` filter name | `truncate` | built-in vs user filter (via modifiers) |
| 4 | `function` | a global/function call | `url_for` | a function vs a macro |
| 5 | `test` | an `is` test name | `defined` in `is defined` | a test vs a filter |
| 6 | `block` | a block name | `content` | the block name vs surrounding text |
| 7 | `keyword` | a Jinja statement keyword | `for`, `block`, `extends` | reinforces keyword coloring inside delimiters |

**REQ-SEM-02 — Declare this token-modifier legend.**

Modifiers qualify a type; several can apply to one token (they're a bitset):

| # | Modifier | Applies to | Meaning |
|---|---|---|---|
| 0 | `defined` | `macro`, `variable`, `function`, `filter`, `test` | resolves to a known symbol in the index/registry |
| 1 | `unknown` | `macro`, `variable`, `function`, `filter`, `test` | resolves to nothing — a likely typo |
| 2 | `builtin` | `filter`, `function`, `test` | comes from the core registry or an extension pack ([F02](F02-builtin-registry.md)/[F03](F03-extension-packs.md)) |
| 3 | `user` | `filter`, `function`, `test`, `macro`, `variable` | comes from a user hint or a macro/variable the user defined ([F04](F04-user-hints.md)) |

So `truncate` is `filter` + `{builtin, defined}`; a hinted `markdown` filter is `filter` + `{user, defined}`; a misspelled `truncat` is `filter` + `{unknown}`. The theme can dim `unknown` and tint `user` filters differently from `builtin` ones.

### 5.2 The two requests

Editors ask for the whole file or just the visible viewport.

**REQ-SEM-03 — Support `full` and `range`.**

`semanticTokens/full` returns tokens for the entire document; `semanticTokens/range` returns tokens for a given range (the viewport), so large files stay responsive. Both encode tokens as the LSP delta-position integer array, relative to the legend declared at `initialize`. A `range` response is a strict subset of what `full` would return for the same lines.

### 5.3 Token derivation

Each token's type and modifiers come from resolving the symbol, not from its spelling.

**REQ-SEM-04 — Resolve every token against the index and registry.**

For each reference in the file ([E07](../foundations/E07-data-model.md)), resolve it: a macro call against the [WorkspaceIndex](../glossary.md); a filter/function/test against the [built-in registry](../glossary.md) ([F02](F02-builtin-registry.md), including packs and hints). Emit the matching type (§5.1) with `defined`/`unknown` reflecting whether it resolved and `builtin`/`user` reflecting its source. Tokens are emitted only for the Jinja layer — host-language bytes are never tokenized (P5).

### 5.4 Delta is deferred

There's a third request for incremental updates; we're not shipping it yet.

**REQ-SEM-05 — `full/delta` is deferred.**

`semanticTokens/full/delta` (sending only the changed tokens after an edit) is a performance optimization, not a correctness feature. v1 does not advertise it; clients fall back to `full`/`range`, which are fast enough at our file sizes (P6). Revisit if profiling shows re-tokenizing large files is a hot path.

> **Note:** Because we declare a stable legend and a `range` request, an editor already only re-pulls the viewport on scroll — most of delta's benefit without its bookkeeping.

## 6. UI Mockups

### 6.1 The legend as the theme sees it

The legend maps each token type + modifier to an example coloring an editor theme would apply. This is what makes a known macro look different from a typo.

```
┌─ Semantic token legend → example theme coloring ─────────────────────────┐
│                                                                            │
│  token (type + modifiers)        example          rendered as             │
│  ──────────────────────────────────────────────────────────────────────  │
│  macro {defined}                 post_url          ● teal, bold           │
│  macro {unknown}                 psot_url          ● red, wavy underline  │
│  variable {defined,user}         post              ● blue                 │
│  variable {unknown}              psot              ● red, dimmed          │
│  parameter                       words             ● light-blue italic    │
│  filter {builtin,defined}        truncate          ● purple              │
│  filter {user,defined}           markdown          ● purple, italic      │
│  filter {unknown}                truncat           ● red, dimmed         │
│  function {builtin,defined}      url_for           ● gold                │
│  test {builtin,defined}          defined           ● green               │
│  block                           content           ● orange              │
│  keyword                         for / block       ● magenta            │
│                                                                            │
└───────────────────────────────────────────────────────────────────────────┘
```

### 6.2 A colored line

`{{ post_url(post) | truncat }}` after semantic coloring: the macro is "defined," the variable resolves via a hint, and the misspelled filter is flagged "unknown."

```
  4 │ {{ post_url ( post ) | truncat }}
       ▔▔▔▔▔▔▔▔        ▔▔▔▔     ▔▔▔▔▔▔▔
       macro{defined}  var      filter{unknown}
                       {defined,user}   ← dimmed red; F01 also squiggles E102
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` line `{{ post_url(post) | truncate(40) }}` colors `post_url` as `macro {defined}` (resolved via the import), `post` as `variable {defined, user}` (a hinted context var), and `truncate` as `filter {builtin, defined}` (§5.4). Misspell it to `truncat` and the token becomes `filter {unknown}` — the theme dims it, and [F01](F01-diagnostics.md) `JINJA-E102` squiggles it in parallel.

## 10. Edge Cases & Failure Modes

- **Disabled extension pack** → a pack-only filter resolves to nothing and tokens as `unknown` (consistent with [F03](F03-extension-packs.md) making disabled packs invisible).
- **Hinted symbol overriding a built-in** → tokens as `{user, defined}`, reflecting the highest-priority registry source ([F04](F04-user-hints.md)).
- **Broken template** → tokenize whatever Pass 1 extracted; unparseable spans are skipped, never erroring (P3).
- **`range` viewport split across a token** → the token is included if it overlaps the range; positions stay file-absolute on decode.
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → tokens emit in host-file coordinates so the editor colors the inline region correctly.
- **Legend evolution** → adding a type/modifier is a versioned change; the order of existing entries must never shift (it's the wire encoding).

## 11. Testing

Semantic tokens are verified by integration tests decoding the token array over fixtures plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-SEM-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Declared legend matches §5.1/§5.2 exactly, in order | unit | — | REQ-SEM-01, REQ-SEM-02 |
| `full` tokenizes a whole file with correct types/modifiers | integration | starlette-blog | REQ-SEM-03, REQ-SEM-04 |
| `range` returns a subset matching `full` for those lines | integration | starlette-blog | REQ-SEM-03 |
| Resolved vs unknown; builtin vs user modifiers correct | integration | starlette-blog, user-hints | REQ-SEM-04 |
| Disabled pack → pack filter tokens as `unknown` | integration | starlette-blog | REQ-SEM-04 |
| `full/delta` is not advertised | e2e (pytest-lsp) | starlette-blog | REQ-SEM-05 |

### 11.3 Fixtures

- `starlette-blog` for the resolved/built-in cases; `user-hints` for the `user` modifier. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-SEM-01 | legend-types test |
| REQ-SEM-02 | legend-modifiers test |
| REQ-SEM-03 | full + range subset test |
| REQ-SEM-04 | resolution + modifier test |
| REQ-SEM-05 | delta-not-advertised e2e |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the legend and both requests**, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `initialize` then read the declared legend | happy | legend equals §5.1/§5.2 in order |
| E2E-02 | `semanticTokens/full` on `post.html` | happy | macro `defined`, filter `builtin`, var `user` |
| E2E-03 | `semanticTokens/range` over a viewport | happy | subset consistent with `full` |
| E2E-04 | Misspell a filter | happy | the token carries the `unknown` modifier |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — all template content is untrusted; tokens are derived from the index only and never by executing templates (P1).
- **Data sensitivity** — tokens classify only the open file's own spans; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — no GUI; the editor theme renders all coloring. (Themes own contrast and color-blind palettes.)

### 13.4 Performance & Scale

- **Latency** — `full` and `range` are single passes over the file's references and return in < 100 ms (P6); `range` keeps large files responsive without delta.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5, P6; [E07-data-model](../foundations/E07-data-model.md) — the references tokenized; [F02-builtin-registry](F02-builtin-registry.md) — built-in vs user resolution; [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F12-folding-range](F12-folding-range.md) — another tree-driven feature; [F04-user-hints](F04-user-hints.md) — the `user` modifier's source; [F06-hover](F06-hover.md) — the docs behind a resolved token; [F01-diagnostics](F01-diagnostics.md) — squiggling what tokens color `unknown`.

## 17. Changelog

- **2026-06-24** — Initial draft.
