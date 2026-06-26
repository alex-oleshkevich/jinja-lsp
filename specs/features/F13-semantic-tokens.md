# F13 — Semantic Tokens

> **Status:** Draft
>
> **Version:** 0.3   ·   **Last updated:** 2026-06-26
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

> **`unknown` is the quiet axis; the `builtin`/`user`/`macro` axes are the real value.** [F01-diagnostics](F01-diagnostics.md) already owns the *loud* signal for an unresolved symbol — the red squiggle on `JINJA-E101`–`E104`. So `unknown` exists only for *subtle* de-emphasis (a theme dimming a likely typo), never to compete with F01's squiggle. The genuine, F01-can't-do-it value of this feature is the positive classification: telling a *known macro* from an unknown variable, a *built-in* filter from a *user* one. Implementations and themes should prioritize the `macro` / `builtin` / `user` distinctions; `unknown` is a gentle complement to F01, not the headline.

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
| 2 | `parameter` | a macro parameter name | `words` in `macro excerpt(post, words)` | a param *binding* vs a variable *use* |
| 3 | `filter` | a `\|` filter name | `truncate` | built-in vs user filter (via modifiers) |
| 4 | `function` | a global/function call | `url_for` | a function vs a macro (see §5.3.1) |
| 5 | `test` | an `is` test name | `defined` in `is defined` | a test vs a filter |
| 6 | `block` | a block name | `content` | the block name vs surrounding text |

We do **not** declare a `keyword` type. Statement keywords (`for`, `block`, `extends`) are coloured by *shape* — the editor's tree-sitter/TextMate grammar already owns them, and §2 forbids re-colouring what the grammar colours. A semantic `keyword` token would add no fact the grammar lacks, so it would only duplicate, never augment. (Index 7 is reserved against this retired entry — see REQ-SEM-06.)

**REQ-SEM-02 — Declare this token-modifier legend.**

Modifiers qualify a type; several can apply to one token (they're a bitset):

| # | Modifier | Applies to | Meaning |
|---|---|---|---|
| 0 | `defined` | `macro`, `variable`, `function`, `filter`, `test` | resolves to a known symbol in the index/registry |
| 1 | `unknown` | `macro`, `variable`, `filter`, `test` | resolves to nothing — a likely typo. (Not `function`: an unresolved *call* falls to `variable {unknown}`, never `function {unknown}` — §5.3.1 step 3.) |
| 2 | `builtin` | `filter`, `function`, `test` | comes from the core registry or an extension pack ([F02](F02-builtin-registry.md)/[F03](F03-extension-packs.md)) |
| 3 | `user` | `filter`, `function`, `test`, `macro`, `variable` | comes from a user hint or a macro/variable the user defined ([F04](F04-user-hints.md)) |

So `truncate` is `filter` + `{builtin, defined}`; a hinted `markdown` filter is `filter` + `{user, defined}`; a misspelled `truncat` is `filter` + `{unknown}`. The theme can dim `unknown` and tint `user` filters differently from `builtin` ones.

No modifier lists `parameter` or `block` under its *Applies to*: both token types carry **zero modifiers**. A `parameter` is a binding occurrence (§5.3.2) and a `block` is a definition (§5.3.3) — neither resolves against the index or registry, so the `defined`/`unknown` and `builtin`/`user` axes do not apply.

**REQ-SEM-06 — The legend is append-only; retired indices are tombstoned, never reused.**

Both lists above are now frozen. A token type or modifier may be **added** only by appending it at the next free index; an existing entry's index must never shift, because the wire encoding is positional — a token's type/modifier is the *index*, so reordering silently re-paints every token in flight. A retired entry's index is **permanently reserved (tombstoned)**: it is never reused for a different meaning and never removed (removal would re-pack the indices above it). Index 7 of the type list is the first such tombstone — it once held the `keyword` type (REQ-SEM-01), now retired; it stays empty rather than being filled by a future type. This append-only, tombstone-on-retire rule is what lets an editor cache the legend across server versions.

### 5.2 The two requests

Editors ask for the whole file or just the visible viewport.

**REQ-SEM-03 — Support `full` and `range`.**

`semanticTokens/full` returns tokens for the entire document; `semanticTokens/range` returns tokens for a given range (the viewport), so large files stay responsive. Both encode tokens as the LSP delta-position integer array, relative to the legend declared at `initialize`. A `range` response emits the **same tokens** `full` would for the overlapping lines, but **re-encoded relative to the range start** — the first token's delta is measured from the range origin, not the document origin. The raw integer arrays are therefore *not* a literal subset of `full`'s array (the leading deltas differ); only the **decoded** `(absolute-position, type, modifiers)` tuples form a subset. Tests assert subset-ness on the decoded tuples, never on the wire integers.

**Inclusion is by overlap; the first delta is never negative.** A token is emitted iff its span **overlaps** the requested range — including a token whose start falls *above* (before) the range start but whose span reaches into it. LSP deltas are unsigned, so the first emitted token cannot encode a position earlier than the range origin; the server therefore anchors the first token at its own **absolute** start (deltaLine/deltaStartChar measured from document origin `(0,0)`, the conventional baseline for the first entry), then encodes every subsequent token relative to its predecessor as usual. Decoding reconstructs each token's true file-absolute position regardless of where the range began, so the overlap-included token's decoded position equals the one `full` would report.

### 5.3 Token derivation

Each token's type and modifiers come from resolving the symbol, not from its spelling.

**REQ-SEM-04 — Resolve every token against the index and registry.**

For each reference in the file ([E07](../foundations/E07-data-model.md)), resolve it: a macro call against the [WorkspaceIndex](../glossary.md); a filter/function/test against the [built-in registry](../glossary.md) ([F02](F02-builtin-registry.md), including packs and hints). Emit the matching type (§5.1) with `defined`/`unknown` reflecting whether it resolved and `builtin`/`user` reflecting its source. Tokens are emitted only for the Jinja layer — host-language bytes are never tokenized (P5).

The two reference kinds that syntax alone cannot type — a bare call `foo(...)` and an identifier in a macro header — have explicit resolution orders below.

#### 5.3.1 A bare call: `macro` vs `function` vs `variable`

A call site `foo(args)` is, syntactically, just `identifier(args)` — the grammar cannot say whether `foo` is a macro, a registered function, or neither. Resolve it in this fixed order and emit the first hit:

1. **macro-in-index** — `foo` resolves to a local or imported `MacroDefinition` in the [WorkspaceIndex](../glossary.md) (REQ-DATA-11 — a *call* resolves to a `MacroDefinition`) → type `macro`, `{defined}` plus `{user}` (a macro is always user-authored).
2. **registry function** — else `foo` resolves to a `function`-category entry in the [built-in registry](../glossary.md) ([F02 §5.1](F02-builtin-registry.md), keyed `(function, name)`) → type `function`, with `{builtin}` for a core/pack source or `{user}` for a hint-contributed function ([F04](F04-user-hints.md)).
3. **else** — `foo` resolves to nothing callable → type `variable` `{unknown}`. It is *not* typed `function {unknown}`: with no registry or index hit there is no evidence the name is a function rather than a mis-typed variable, and `variable` is the honest fallback for any unresolved identifier.

> **`request` is a `variable`, not a `function`.** [F02](F02-builtin-registry.md) ships the starlette `request` global as a `var_request.md` doc — category `variable`. So `request` resolves at step 3's sibling path (an identifier that hits a `(variable, …)` registry entry) and tokens as `variable {builtin, defined}`, never `function`. Only names backed by a `(function, name)` entry (e.g. `url_for`) take the `function` type. There is no `function {user}` category in F02; the `{user}` modifier on a `function` token comes solely from a [F04](F04-user-hints.md) `function`-category hint, not from any F02 user-function notion.

#### 5.3.2 `parameter` vs `variable`: a binding occurrence vs a use

`parameter` marks the *binding* occurrence of a name in a macro **signature** (`{% macro excerpt(post, words) %}` — `post` and `words`); `variable` marks a *use* of a name in a body. Syntax can't separate them — both are identifiers. The distinguishing fact comes from [E07](../foundations/E07-data-model.md): an occurrence is a `parameter` token iff its enclosing owner (REQ-DATA-12) is a `MacroDefinition` **and** the occurrence is one of that macro's `parameters` (REQ-DATA-01) at its signature span — not a reference inside the body. Every other identifier occurrence is a `variable` use, resolved per §5.3.1 / REQ-DATA-11. (This mirrors how [F06](F06-hover.md) reads E07 slots to type an occurrence.)

#### 5.3.3 `block`: a name from a definition, not a resolved reference

A `block` token marks a block name (`{% block content %}`) — unlike a call or a use, it is a *definition*, not a reference resolved against the index. The token comes straight from [E07](../foundations/E07-data-model.md): an occurrence is a `block` token iff it is the `name` span of a `BlockDefinition` (REQ-DATA-02 — the block's declared name at its definition span). No lookup runs, so the token carries no resolution modifier (§5.2) — the name *is* the definition. (Block usages elsewhere, e.g. an `extends` child re-declaring a block, are likewise the name spans of their own `BlockDefinition`s.)

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
│  function {user,defined}         csrf_token        ● gold, italic        │
│  variable {builtin,defined}      request           ● blue (a var, §5.3.1)│
│  test {builtin,defined}          defined           ● green               │
│  block                           content           ● orange              │
│                                                                            │
│  (statement keywords for/block/extends are coloured by the editor's       │
│   grammar — §2 — and carry no semantic token; see REQ-SEM-01.)            │
│                                                                            │
└───────────────────────────────────────────────────────────────────────────┘
```

### 6.2 A colored line

`{{ post_url(post) | truncat }}` after semantic coloring: the macro is "defined," the variable resolves via a hint, and the misspelled filter is flagged "unknown."

```
  4 │ {{ post_url(post) | truncat }}
         ▔▔▔▔▔▔▔▔ ▔▔▔▔    ▔▔▔▔▔▔▔
         macro    variable filter
         {defined,{defined,{unknown}  ← dimmed red; F01 also squiggles E102
          user}    user}
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` line `{{ post_url(post) | truncate(40) }}` colors `post_url` as `macro {defined}` (resolved via the import), `post` as `variable {defined, user}` (a hinted context var), and `truncate` as `filter {builtin, defined}` (§5.4). Misspell it to `truncat` and the token becomes `filter {unknown}` — the theme dims it, and [F01](F01-diagnostics.md) `JINJA-E102` squiggles it in parallel.

## 10. Edge Cases & Failure Modes

- **Disabled extension pack** → a pack-only filter resolves to nothing and tokens as `unknown` (consistent with [F03](F03-extension-packs.md) making disabled packs invisible).
- **Hinted symbol overriding a built-in** → tokens as `{user, defined}`, reflecting the highest-priority registry source ([F04](F04-user-hints.md)).
- **Broken template** → tokenize whatever Pass 1 extracted; unparseable spans are skipped, never erroring (P3).
- **`range` viewport split across a token** → the token is included if it overlaps the range; positions stay file-absolute on decode.
- **Inline templates** ([E31](../foundations/E31-inline-templates.md)) → tokens emit in host-file coordinates so the editor colors the inline region correctly.
- **Legend evolution** → adding a type/modifier is a versioned, append-only change; the order of existing entries must never shift and a retired entry's index stays tombstoned, never reused or removed (REQ-SEM-06 — it's the wire encoding). Index 7 of the type list is the live tombstone, vacated by the retired `keyword` type.

## 11. Testing

Semantic tokens are verified by integration tests decoding the token array over fixtures plus a `pytest-lsp` protocol journey.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior.** Every `REQ-SEM-NN` maps to at least one test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

Each row names the concrete token (or request) and the type + modifier set it must
decode to. Every §5.1 token type, every §5.2 modifier, both polarities of the
`defined`/`unknown` and `builtin`/`user` axes, every §10 edge, and every §6 legend
row / §6.2 line token are covered. "synthetic doc" = an in-memory `didOpen` document
for constructs not present in a registered fixture (§11.3).

**Legend declaration (REQ-SEM-01, REQ-SEM-02)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Token-type legend = `[macro, variable, parameter, filter, function, test, block]` in exactly that order (indices 0–6); no `keyword` type is declared | unit | — | REQ-SEM-01 |
| Token-modifier legend = `[defined, unknown, builtin, user]` in exactly that order (bits 0–3) | unit | — | REQ-SEM-02 |
| Legend order is stable across versions — re-declaring never shifts an existing index (§10 legend-evolution) | unit | — | REQ-SEM-01, REQ-SEM-02, REQ-SEM-06 |
| Legend is append-only: index 7 (the retired `keyword` slot) stays tombstoned — never reused, never removed; a new type would append at index 7+ only (§5.1, §10) | unit | — | REQ-SEM-06 |

**Token TYPES — one positive row per §5.1 type (REQ-SEM-04)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `post_url` in `post.html` → type `macro` | integration | starlette-blog | REQ-SEM-04 |
| `post` in `{{ post_url(post) }}` → type `variable` | integration | starlette-blog, user-hints | REQ-SEM-04 |
| `post` in `macro post_url(post)` (macros.html) → type `parameter` | integration | starlette-blog | REQ-SEM-04 |
| `truncate` in `{{ … \| truncate(40) }}` → type `filter` | integration | starlette-blog | REQ-SEM-04 |
| `url_for(...)` (a `(function, …)` registry entry) → type `function` | integration | starlette-blog | REQ-SEM-04 |
| `defined` in `{% if x is defined %}` → type `test` | integration | synthetic doc | REQ-SEM-04 |
| `content` block name in `{% block content %}` → type `block` | integration | starlette-blog | REQ-SEM-04 |
| Statement keywords `for`/`block`/`extends` emit **no** semantic token (grammar owns them, §2/REQ-SEM-01) | integration | starlette-blog | REQ-SEM-01, REQ-SEM-04 |

**Bare-call & parameter resolution order (REQ-SEM-04, §5.3.1/§5.3.2)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `post_url(post)` resolves macro-in-index first → type `macro {defined,user}` (step 1) | integration | starlette-blog | REQ-SEM-04 |
| `url_for(...)` misses the index, hits a `(function,…)` registry entry → type `function {builtin,defined}` (step 2) | integration | starlette-blog | REQ-SEM-04 |
| `request` resolves to a `(variable,…)` registry entry → type `variable {builtin,defined}`, never `function` (§5.3.1 note) | integration | starlette-blog | REQ-SEM-04 |
| An unresolved call `foo(...)` (no index, no registry hit) → type `variable {unknown}`, **not** `function {unknown}` (step 3) | integration | undefined-vars | REQ-SEM-04 |
| A name in a macro signature (enclosing owner = its `MacroDefinition`, in its `parameters`) → `parameter`; the same name used in the body → `variable` (§5.3.2, REQ-DATA-12/01) | integration | starlette-blog | REQ-SEM-04 |

**Modifier combinations — both polarities of every axis (REQ-SEM-02, REQ-SEM-04)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `macro` `{defined, user}` — `post_url` resolves to the user-defined macro (§6.1) | integration | starlette-blog | REQ-SEM-04 |
| `macro` `{unknown}` — a misspelled macro call (e.g. `psot_url(post)`) resolves to nothing (§6.1, §6.2) | integration | synthetic doc | REQ-SEM-04 |
| `variable` `{defined, user}` — hinted `post` (type `Post`) (§6.1, §6.2) | integration | user-hints | REQ-SEM-04 |
| `variable` `{unknown}` — `psot` resolves to nothing (§6.1, §6.2) | integration | undefined-vars | REQ-SEM-04 |
| `filter` `{builtin, defined}` — `truncate` from the core registry (§6.1) | integration | starlette-blog | REQ-SEM-04 |
| `filter` `{user, defined}` — a hinted `markdown` filter (§6.1) | integration | user-hints | REQ-SEM-04 |
| `filter` `{unknown}` — misspelled `truncat` resolves to nothing (§6.1, §6.2) | integration | undefined-vars | REQ-SEM-04 |
| `function` `{builtin, defined}` — `url_for` (a `(function,…)` entry) from the starlette pack | integration | starlette-blog | REQ-SEM-04 |
| `variable` `{builtin, defined}` — `request` (a `(variable,…)` entry) from the starlette pack, typed `variable` not `function` (§5.3.1) | integration | starlette-blog | REQ-SEM-04 |
| `function` `{user, defined}` — a hinted user function | integration | user-hints | REQ-SEM-04 |
| `test` `{builtin, defined}` — `defined` / `divisibleby` from the registry | integration | synthetic doc | REQ-SEM-04 |
| `test` `{unknown}` — a misspelled `is` test resolves to nothing | integration | undefined-vars | REQ-SEM-04 |
| `parameter` carries no resolution modifier (a param is neither defined nor unknown) | integration | starlette-blog | REQ-SEM-01, REQ-SEM-04 |
| `block` carries no resolution modifier | integration | starlette-blog | REQ-SEM-01, REQ-SEM-04 |

**The two requests (REQ-SEM-03)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `full` tokenizes the whole `post.html`, every reference typed/modified per §5.1/§5.2 | integration | starlette-blog | REQ-SEM-03, REQ-SEM-04 |
| `range` over a viewport: its **decoded** `(abs-pos, type, mods)` tuples are a subset of `full`'s for the overlapping lines (the wire integer arrays are *not* a literal subset — leading deltas are re-encoded from the range start) (§5.2) | integration | starlette-blog | REQ-SEM-03 |
| `range` includes a token whose span overlaps the viewport edge; decoded positions stay file-absolute (§10 range-split) | integration | starlette-blog | REQ-SEM-03 |
| Both responses encode as LSP delta-position integers relative to the `initialize` legend | integration | starlette-blog | REQ-SEM-03 |

**§10 edges & §6.2 line (REQ-SEM-04)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Disabled extension pack → a pack-only filter tokens as `filter {unknown}` (§10) | integration | starlette-blog | REQ-SEM-04 |
| Hinted symbol overriding a built-in filter → `{user, defined}`, not `{builtin, defined}` (§10) | integration | user-hints | REQ-SEM-04 |
| Broken template → tokenize whatever Pass 1 extracted; unparseable spans skipped, never erroring (§10, P3) | integration | syntax-errors | REQ-SEM-04 |
| Inline/embedded template → tokens emit in host-file coordinates (§10, E31) | integration | call-and-paths | REQ-SEM-03, REQ-SEM-04 |
| Host-language bytes are never tokenized — only Jinja spans (P5) | integration | starlette-blog | REQ-SEM-04 |
| The whole `{{ post_url(post) \| truncat }}` line decodes to `macro{defined,user}` · `variable{defined,user}` · `filter{unknown}` (§6.2) | integration | synthetic doc | REQ-SEM-04 |

**Delta (REQ-SEM-05)**

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `full/delta` is absent from `semanticTokensProvider` server capabilities | e2e (pytest-lsp) | starlette-blog | REQ-SEM-05 |
| Client editing the file falls back to `full`/`range` and still gets correct tokens | e2e (pytest-lsp) | starlette-blog | REQ-SEM-05 |

### 11.3 Fixtures

- `starlette-blog` for the resolved/built-in cases; `user-hints` for the `user` modifier (and the hint-over-builtin override); `undefined-vars` for the `unknown` polarity (variable/filter/test, plus an unresolved bare call that falls to `variable {unknown}` per §5.3.1); `syntax-errors` for the broken-template edge; `call-and-paths` for the inline/embedded-template (E31) edge. Registered in [E17-testing](../foundations/E17-testing.md#5-fixtures-registry).
- Constructs absent from any registered fixture — an `is`-test call site, a misspelled macro call, and the exact §6.2 `{{ post_url(post) | truncat }}` line — use synthetic in-memory `didOpen` documents (per the `starlette-blog` registry note on throwaway probes).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-SEM-01 | legend-types order test (indices 0–6, no `keyword`); the keyword-emits-no-token type row; parameter/block "no resolution modifier" rows |
| REQ-SEM-02 | legend-modifiers order + stability tests; every modifier-combination row (both polarities of `defined`/`unknown` and `builtin`/`user`) |
| REQ-SEM-03 | full + range decoded-tuple-subset + range-overlap + delta-encoding rows; inline host-coordinate row; E2E-03 |
| REQ-SEM-04 | per-type rows; bare-call/parameter resolution-order rows (§5.3.1/§5.3.2); every modifier-combination row; §10 edge rows (disabled pack, hint-override, broken template, inline, host-bytes); §6.2 line row; E2E-02/04/05/06 |
| REQ-SEM-05 | delta-not-advertised + fallback rows; E2E-07 |
| REQ-SEM-06 | legend-stability test + append-only/tombstone (index-7) test; §10 legend-evolution edge |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of this feature's user-visible scope** — the declared legend, both requests (`full` and `range`), the resolved/`unknown`/`user`-override modifier paths, and the deferred-`delta` capability fallback — across all seven §12.2 scenarios, via `pytest-lsp` against the real stdio binary ([E29-e2e-testing](../foundations/E29-e2e-testing.md), Branch B).

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `initialize` then read the declared legend | happy | token types = `[macro,variable,parameter,filter,function,test,block]` (no `keyword`) and modifiers = `[defined,unknown,builtin,user]`, in order |
| E2E-02 | `semanticTokens/full` on `post.html` — resolved symbols | happy | `post_url` → `macro{defined,user}`; `truncate` → `filter{builtin,defined}`; `post` → `variable{defined,user}`; `request` → `variable{builtin,defined}`; `content` → `block`; `for`/`block` emit no semantic token (grammar owns them) |
| E2E-03 | `semanticTokens/range` over a viewport of `post.html` | happy | the decoded `(abs-pos,type,mods)` tuples are a subset of `full`'s for those lines (wire arrays differ — leading deltas re-encoded from the range start), positions file-absolute even when a token straddles the viewport edge |
| E2E-04 | Misspell a filter (`truncate` → `truncat`) | negative | the filter token carries `{unknown}`, not `{builtin,defined}` |
| E2E-05 | Reference an undefined variable (`psot`) and an undefined `is` test | negative | `variable{unknown}` and `test{unknown}` tokens emitted (still colored, not dropped) |
| E2E-06 | Open a file with a `user`-hinted filter overriding a built-in | happy | the filter tokens as `{user,defined}`, reflecting the highest-priority source |
| E2E-07 | Inspect `semanticTokensProvider` capability; then edit and re-pull | negative | `full/delta` is absent; client falls back to `full`/`range` and still gets correct tokens |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — a read-only handler over stdio (P2); single-user developer tool; no host execution, no writes, no host-language analysis (P1, P5).
- **Input & validation** — all template content is untrusted; tokens are derived from the index only and never by executing templates (P1).
- **Data sensitivity** — tokens classify only the open file's own spans; nothing leaves the machine.
- **Baseline** — OWASP ASVS L1; STRIDE: the only untrusted input is template text, handled by static parse (P1), so tampering/info-disclosure surfaces reduce to graceful degradation (P3).

### 13.2 Accessibility

- **N/A** — no GUI; the editor theme renders all coloring. (Themes own contrast and color-blind palettes.)

### 13.4 Performance & Scale

- **Latency** — `full` and `range` are single passes over the file's references and return in < 100 ms (P6); `range` keeps large files responsive without delta.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5, P6; [E07-data-model](../foundations/E07-data-model.md) — the references tokenized, reference resolution (REQ-DATA-11) behind §5.3.1, enclosing-owner (REQ-DATA-12) + macro `parameters` (REQ-DATA-01) behind §5.3.2; [F02-builtin-registry](F02-builtin-registry.md) — built-in vs user resolution and the `(category, name)` keying (`request` a `variable`, `url_for` a `function`); [E01-architecture](../foundations/E01-architecture.md) — pure-read handlers.
- **Related:** [F12-folding-range](F12-folding-range.md) — another tree-driven feature; [F04-user-hints](F04-user-hints.md) — the `user` modifier's source; [F06-hover](F06-hover.md) — the docs behind a resolved token; [F01-diagnostics](F01-diagnostics.md) — squiggling what tokens color `unknown`.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-25** — v0.2 (review fixes): removed the `keyword` token type — the editor grammar owns statement keywords and re-colouring them violates §2 (REQ-SEM-01; index 7 retired/tombstoned). Added REQ-SEM-06 (append-only, tombstone-on-retire legend). Defined the bare-call resolution order macro-in-index → registry-function → `variable {unknown}` (§5.3.1) and reconciled `request` as a `variable`, `url_for` as a `function`, with no F02 user-function category (the `{user}` function modifier comes from F04 hints). Cited E07 REQ-DATA-12/01 for the parameter-vs-variable derivation (§5.3.2). Reworded `range` from "strict subset" to a decoded-tuple subset re-encoded from the range start (REQ-SEM-03), asserting subset on decoded tuples. Added a §3 note framing `unknown` as F01's quiet complement and the `builtin`/`user`/`macro` axes as the real value. Removed `function` from the `unknown` modifier's appliers (unresolved calls fall to `variable {unknown}`). Updated the §6.1 legend mockup, §11.2 tests, §11.4 coverage (added REQ-SEM-06), and E2E-01/02/03 to match.
- **2026-06-25** — Expanded §11.2 test plan and §12.2 E2E scenarios to cover every combination: each §5.1 token type, each §5.2 modifier in both `defined`/`unknown` and `builtin`/`user` polarities, the parameter/block "no resolution modifier" cases, all §10 edges (disabled pack, hint-over-builtin, broken template, range-split, inline, host-bytes, legend evolution), and the §6 legend rows / §6.2 line. Updated §11.3 fixtures and §11.4 requirement-coverage table; added E2E-05/06/07.
- **2026-06-26** — v0.3 (spec-review fixes): stated normatively that `parameter` and `block` tokens carry zero modifiers (REQ-SEM-02; jinja-lsp-0zs). Added §5.3.3 deriving the `block` token from a `BlockDefinition` name span (E07 REQ-DATA-02), parallel to §5.3.2 (jinja-lsp-4jb). Clarified `range` inclusion-by-overlap and that the first emitted token anchors at its absolute start so no delta is negative (REQ-SEM-03; jinja-lsp-3g3). Added a `function {user,defined}` row to the §6.1 legend mockup (jinja-lsp-sxz). Realigned the §6.2 colored-line so each underline sits under its token, with the modifier annotation inline (jinja-lsp-1rl). Broadened §12.1 E2E coverage wording to match all seven §12.2 scenarios (jinja-lsp-a2w). Added the required §13.1 "Access & authorization" and "Baseline" bullets (jinja-lsp-9ix).
