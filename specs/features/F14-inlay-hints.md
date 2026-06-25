# F14 — Inlay Hints

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** Inline ghost text that makes Jinja calls and blocks self-explanatory — parameter-name labels at macro calls, an `endblock` name echo, and an optional loop-variable type hint — all served lazily via `inlayHint/resolve`.

> **Depends on:** [constitution](../constitution.md), [E07-data-model](../foundations/E07-data-model.md), [E01-architecture](../foundations/E01-architecture.md)   ·   **Related:** [F02-builtin-registry](F02-builtin-registry.md), [F04-user-hints](F04-user-hints.md), [F07-signature-help](F07-signature-help.md)

> Requirement tag: **INLAY**

---

## 1. Purpose & Scope

Inlay hints are the small grey labels an editor paints *between* your tokens — they tell you what an argument means without you opening the definition. When you call `{{ post_url(post) }}`, the hint shows you that `post` fills the `post` parameter; when you close a long block with a bare `{% endblock %}`, the hint reminds you which block you just closed.

This spec covers:

- **Parameter-name hints** at macro call sites, drawn from each macro's declared parameters ([E07](../foundations/E07-data-model.md)).
- **`endblock` name echo** when a `{% endblock %}` omits its block name.
- **Loop-variable type hints** when a loop iterates a hinted iterable (off by default).
- The three categories as independent, client-toggleable settings.
- Lazy tooltip computation via `inlayHint/resolve`.

## 2. Non-Goals / Out of Scope

- The full call-site signature popup — owned by [F07-signature-help](F07-signature-help.md).
- The documentation bodies that hints can link to — owned by [F02-builtin-registry](F02-builtin-registry.md) and [F04-user-hints](F04-user-hints.md).
- Inferring types by executing Python or rendering templates — forbidden by P1; we only echo what hints already declare.
- Inlay hints inside the host language (HTML/SQL/text) — we paint hints only inside Jinja delimiters (P5).

## 3. Background & Rationale

Jinja calls are positional, and a macro like `comment_card(comment, show_actions)` reads as `comment_card(c, true)` at the call site — the second argument is a mystery. Inlay hints close that gap the way they do in Rust or TypeScript: a faint `show_actions:` label before `true`. The `endblock` echo earns its place because base layouts often nest blocks hundreds of lines deep, and a bare `{% endblock %}` tells you nothing. Each category is independently toggleable because hints are noise to some readers and a lifeline to others; per P4-adjacent restraint, the noisiest one (loop types) ships off.

## 4. Concepts & Definitions

- **Inlay hint** — inline ghost text the editor renders between tokens. (Canonical definition in [glossary](../glossary.md).)
- **Parameter-name hint** — a `param:` label painted before a positional argument at a call site.
- **`endblock` echo** — the block name painted after a name-less `{% endblock %}`.
- **Resolve** — the lazy second round-trip (`inlayHint/resolve`) that attaches a hint's tooltip only when the user hovers it.

## 5. Detailed Specification

The server advertises `inlayHintProvider` with `resolveProvider: true` ([E01](../foundations/E01-architecture.md)). On `textDocument/inlayHint` for a visible range, the handler reads the `TemplateIndex` for that file and emits hints purely from extracted facts — no parsing, no execution.

### 5.1 Parameter-name hints at macro calls

When you call a macro with positional arguments, the hint labels each argument with the parameter it fills.

**REQ-INLAY-01 — Label positional macro arguments with their parameter name.**

For a call to a known macro, emit one `InlayHint` of kind `Parameter` immediately before each positional argument, with label `<param>:` and position at the argument's start. The parameter names come from the macro's `parameters` list ([E07](../foundations/E07-data-model.md)), matched left-to-right. So `{{ post_url(post) }}` renders as `{{ post_url(post: post) }}` and `{{ comment_card(comment, true) }}` renders as `{{ comment_card(comment: comment, show_actions: true) }}`.

A hint is emitted only when the macro is resolvable in the `WorkspaceIndex`. Unresolvable calls get no hint — we never guess (P4). Keyword arguments (`comment_card(comment=c)`) already name their parameter, so they get no hint. Arguments past the last declared parameter (or covered by a `*args`-style trailing param) get no hint.

This category is **on by default**.

### 5.2 `endblock` name echo

A long block closed with a bare `{% endblock %}` gives the reader no anchor; the echo supplies one.

**REQ-INLAY-02 — Echo the block name after a name-less `endblock`.**

When a `{% endblock %}` omits the block name, emit one `InlayHint` of kind `Type` positioned just inside the tag, carrying the owning block's name. A `{% endblock content %}` that already names the block gets no hint — it would be redundant. The block name is read from the enclosing `BlockDefinition` ([E07](../foundations/E07-data-model.md)).

This category is **on by default**.

### 5.3 Loop-variable type hints

When a loop iterates a variable whose element type is known from a hint, the loop variable can carry that type.

**REQ-INLAY-03 — Hint the loop variable's element type when the iterable is hinted.**

For `{% for <var> in <iterable> %}`, if `<iterable>` is a hinted `context_variable` ([F04](F04-user-hints.md)) whose declared element type is known, emit a `Type`-kind hint `: <ElementType>` after `<var>`. So in `digest.html`, `{% for post in posts %}` can render as `{% for post: Post in posts %}` when `posts` is hinted as a list of `Post`. With no element-type information, no hint is emitted (P4).

This category is **off by default** — element types are coarse and the hint is the noisiest of the three.

### 5.4 Toggles

Each category is an independent on/off switch the client controls.

**REQ-INLAY-04 — Each category toggles independently.**

The three categories map to three client-side settings: `parameterNames` (default on), `endblockNames` (default on), `loopVariableTypes` (default off). A disabled category emits no hints of that kind; the others are unaffected. Clients that pass these as `InitializationOptions` (or the editor extension's settings — [F20](F20-editor-integrations.md)) get the configured defaults.

### 5.5 Lazy tooltips via resolve

The initial response stays cheap; tooltips are computed only when needed.

**REQ-INLAY-05 — Attach tooltips lazily on resolve.**

The `textDocument/inlayHint` response ships each hint with its label, kind, and position only — no `tooltip`. When the user hovers a hint, the client calls `inlayHint/resolve`, and only then does the handler attach a Markdown tooltip: a parameter hint resolves to the parameter's doc (its declared type/default, or the registry doc — [F02](F02-builtin-registry.md)/[F04](F04-user-hints.md)); an `endblock` echo resolves to the block's definition location. Hints carry an opaque `data` payload so resolve can find the source symbol without re-deriving it.

## 6. UI Mockups

### 6.1 Parameter-name hints + `endblock` echo (editor)

How the two default categories render together in `blog/post.html`. The faint labels (shown here in `‹…›`) are the painted inlay hints, not source text.

```
templates/blog/post.html
 ┌──────────────────────────────────────────────────────────────────────┐
 │  3 │ {% block content %}                                              │
 │  4 │   <a href="{{ post_url(‹post:› post) }}">{{ post.title }}</a>    │
 │  5 │   {{ comment_card(‹comment:› comment, ‹show_actions:› true) }}   │
 │ 18 │ {% endblock ‹content› %}                                         │
 └──────────────────────────────────────────────────────────────────────┘
   ‹ … › = inlay hint (grey ghost text — not part of the file)
```

### 6.2 Loop-variable type hint (off by default)

When `loopVariableTypes` is enabled and `posts` is a hinted list of `Post` (`email/digest.html`):

```
 7 │ {% for post‹: Post› in posts %}
 8 │   {{ post.title }}
 9 │ {% endfor %}
```

### 6.3 Resolved tooltip on hover

Hovering the `show_actions:` parameter hint triggers `inlayHint/resolve` and shows its doc:

```
   …{{ comment_card(comment: comment, show_actions: true) }}
                                       │
                                       ▼
        ╭───────────────────────────────────────────╮
        │ show_actions: bool = true                  │
        │                                            │
        │ Whether to render the reply / edit links.  │
        ╰───────────────────────────────────────────╯
```

## 7. Visualizations

The request/resolve lifecycle for a single hint.

```mermaid
stateDiagram-v2
    [*] --> Computed: textDocument/inlayHint (range)
    Computed --> Painted: label + kind + position only
    Painted --> Resolved: inlayHint/resolve (on hover)
    Resolved --> [*]: tooltip attached
```

## 9. Examples & Use Cases

In `starlette-blog`, `templates/blog/post.html` calls `{{ comment_card(comment, true) }}` where `comment_card(comment, show_actions)` is defined in `blog/macros.html`. With `parameterNames` on, the reader sees `comment_card(comment: comment, show_actions: true)` and instantly understands the bare `true`. The base layout in `templates/base.html` closes its `body` block 40 lines after opening it; with `endblockNames` on, the bare `{% endblock %}` echoes `body`, so the reader doesn't scroll back to check. A maintainer who finds the labels noisy turns `parameterNames` off in their editor settings; everyone else keeps them.

## 10. Edge Cases & Failure Modes

- **Unresolvable macro call** → no parameter hints (we never guess a parameter name — P4).
- **More arguments than parameters** → label up to the last declared parameter; extras get no hint.
- **Keyword argument** (`comment_card(comment=c)`) → already named, so no hint.
- **`{% endblock content %}`** (already named) → no echo; redundant.
- **Loop over a non-hinted iterable** → no type hint even when the category is on.
- **Half-typed call** `{{ post_url( }}` → tree-sitter recovers; no hint until the argument node exists (P3).
- **Resolve for a stale hint** (the document changed underneath) → return the hint unchanged with no tooltip; never throw.

## 11. Testing

Each category and the resolve round-trip are unit-tested against the `starlette-blog` fixture; toggles are tested for independence.

### 11.1 Scope & coverage

Target: **100% of this feature's behavior.** Every `REQ-INLAY-NN` maps to at least one test; every category state (§6) and edge case (§10) has a test. See [E17-testing](../foundations/E17-testing.md#2-coverage-policy).

### 11.2 Test plan

Each row names the concrete call/construct, the fixture (or a synthetic `didOpen` doc where no fixture carries the construct — [E17 §5](../foundations/E17-testing.md#starlette-blog)), and the exact expected hint (label + position) or its absence. Rows pair happy (hint emitted) with negative (no hint) polarities for every branch.

| # | Behavior / scenario | Call / construct · fixture | Expected hint (label · position) or absence | Type | Verifies |
|---|---|---|---|---|---|
| T-01 | Single positional arg → one `param:` label | `{{ post_url(post) }}` in `blog/post.html` · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | `post:` of kind `Parameter`, immediately before the `post` argument | unit | REQ-INLAY-01 |
| T-02 | Multiple positional args → labels in left-to-right order | `comment_card(c, show_actions=true)` rewritten positionally `comment_card(c, true)` · synthetic doc importing `blog/macros.html` | `comment:` before `c`, then `show_actions:` before `true`, both `Parameter` | unit | REQ-INLAY-01 |
| T-03 | Keyword arg already names its parameter → no hint (§10) | `{{ comment_card(c, show_actions=true) }}` in `blog/post.html` · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | `comment:` before `c`; no hint on `show_actions=true` (already named) | unit | REQ-INLAY-01 |
| T-04 | Unresolvable / unknown macro callee → no hint (§10) | `{{ nonexistent_macro(x) }}` · synthetic doc | empty hint list — never guess a parameter name (P4) | unit | REQ-INLAY-01 |
| T-05 | Over-arity call → label up to last param, extras get none (§10) | `{{ post_url(post, extra) }}` (one-param macro, two args) · synthetic doc importing `blog/macros.html` | `post:` before `post`; no hint before `extra` | unit | REQ-INLAY-01 |
| T-06 | Half-typed call → no hint until the argument node exists (§10) | `{{ post_url( }}` · synthetic doc | empty hint list; tree-sitter recovers, no crash (P3) | unit | REQ-INLAY-01 |
| T-07 | Bare `{% endblock %}` → block-name echo | long `content` block closed `{% endblock %}` in `blog/post.html` · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | `content` of kind `Type`, just inside the closing tag | unit | REQ-INLAY-02 |
| T-08 | Bare `{% endblock %}` on the base layout → echoes `body` | `body` block closed bare in `base.html` · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | `body` of kind `Type`, just inside the closing tag | unit | REQ-INLAY-02 |
| T-09 | Named `{% endblock content %}` → no echo (redundant, §10) | named endblock in a synthetic doc | no `Type` echo hint on that tag | unit | REQ-INLAY-02 |
| T-10 | Loop over a hinted iterable → element-type hint | `{% for post in posts %}` where `posts` is hinted list of `Post` · [user-hints](../foundations/E17-testing.md#user-hints) | `: Post` of kind `Type`, after `post` | unit | REQ-INLAY-03 |
| T-11 | Loop over a non-hinted iterable → no type hint even when on (§10) | `{% for c in post.comments %}` (no element-type hint) · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | no `Type` loop hint | unit | REQ-INLAY-03 |
| T-12 | `parameterNames` off → suppresses only `param:` labels | `post.html` content block with `endblockNames` left on · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | no `Parameter` hints; `content` endblock echo still present | unit | REQ-INLAY-04 |
| T-13 | `endblockNames` off → suppresses only the echo | `post.html` content block with `parameterNames` left on · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | no `Type` endblock echo; `post:` parameter hint still present | unit | REQ-INLAY-04 |
| T-14 | `loopVariableTypes` on → enables only loop type hints | `{% for post in posts %}` with the other two toggles off · [user-hints](../foundations/E17-testing.md#user-hints) | `: Post` loop hint present; no parameter/endblock hints | unit | REQ-INLAY-04 |
| T-15 | `loopVariableTypes` off (default) → no loop type hints, others unaffected | hinted-`posts` loop plus a macro call, defaults applied · [user-hints](../foundations/E17-testing.md#user-hints) | no `: Post` loop hint; parameter hint on the call still present | unit | REQ-INLAY-04 |
| T-16 | Initial response carries label/kind/position only, no tooltip | `{{ post_url(post) }}` parameter hint · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | hint has no `tooltip` field; opaque `data` payload present | unit | REQ-INLAY-05 |
| T-17 | Resolve a parameter hint → Markdown param doc tooltip | resolve the `show_actions:` hint · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | tooltip = param doc (`show_actions: bool = true` + body) | unit | REQ-INLAY-05 |
| T-18 | Resolve an `endblock` echo → block definition location | resolve the `content` echo hint · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | tooltip points at the `{% block content %}` definition | unit | REQ-INLAY-05 |
| T-19 | Resolve a stale hint (doc changed underneath) → unchanged, no throw (§10) | resolve a hint after editing the doc · synthetic doc | hint returned unchanged, no tooltip, no exception | unit | REQ-INLAY-05 |

### 11.3 Fixtures

- Reuses [starlette-blog](../foundations/E17-testing.md#5-fixtures-registry) for macro calls and `endblock` echoes, and [user-hints](../foundations/E17-testing.md#5-fixtures-registry) for the loop-variable type case (a hinted `posts` list).

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-INLAY-01 | T-01–T-06 (positional order, keyword-named no-hint, unknown-callee no-hint, over-arity, half-typed); E2E-01, E2E-06 |
| REQ-INLAY-02 | T-07–T-09 (bare echo ×2, named no-echo); E2E-01 |
| REQ-INLAY-03 | T-10, T-11 (hinted element type, non-hinted none); E2E-03 |
| REQ-INLAY-04 | T-12–T-15 (each toggle isolated, loop default off); E2E-03 |
| REQ-INLAY-05 | T-16–T-19 (no initial tooltip, param resolve, endblock resolve, stale resolve); E2E-02, E2E-04 |

## 12. End-to-End Test Plan

### 12.1 Coverage target

**100% of the feature's user-visible scope** through the `pytest-lsp` LSP-protocol branch ([E29](../foundations/E29-e2e-testing.md#2-coverage-policy)): request hints, assert labels, then resolve and assert tooltips.

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | `inlayHint` over `post.html` content block (`post_url(post)`, `comment_card(c, show_actions=true)`, bare `{% endblock %}`) · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | happy | response includes `post:`, `comment:`, and the `content` endblock echo at the right ranges; no hint on the `show_actions=true` keyword arg |
| E2E-02 | `inlayHint/resolve` on the `show_actions:` parameter hint · [starlette-blog](../foundations/E17-testing.md#starlette-blog) | happy | resolved hint carries the parameter's Markdown tooltip (`show_actions: bool = true` + body) |
| E2E-03 | `inlayHint` with `loopVariableTypes` off (default) over the `posts` loop · [user-hints](../foundations/E17-testing.md#user-hints) | happy | no `Type` loop hints; parameter and endblock hints still present |
| E2E-04 | `inlayHint/resolve` on a stale hint after a `didChange` · synthetic doc | happy | hint returned unchanged with no tooltip; the server never throws |
| E2E-05 | `inlayHint` at a position outside any Jinja delimiter (host HTML) | error | empty hint list (host language untouched — P5) |
| E2E-06 | `inlayHint` over a call to an unresolvable macro · synthetic doc | error | empty hint list — no guessed parameter names (P4) |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Input & validation** — hints read the syntax tree and the registry only; no template is executed (P1).
- **Data sensitivity** — labels and tooltips quote only the user's own source and their own hint docs; nothing leaves the machine.

### 13.2 Accessibility

- **N/A** — the editor renders all inlay-hint UI; jinja-lsp emits protocol data only (constitution §4.6).

### 13.4 Performance & Scale

- **Latency** — the `inlayHint` response covers only the requested (visible) range and reads the in-memory `TemplateIndex`, so it returns well inside the interactive budget; tooltip work is deferred to `resolve` (REQ-INLAY-05).

## 15. Open Questions & Decisions

- **Decided** — loop-variable type hints ship off by default; the other two on. Tooltips are resolve-lazy. No host-language hints (P5).
- **OQ-INLAY-1** — should an over-arity call (more args than params) surface a subtle hint, or stay silent and leave it to `JINJA-E501`? Currently silent.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — the mockup and P1/P5 rules; [E07-data-model](../foundations/E07-data-model.md) — macro `parameters` and `BlockDefinition`; [E01-architecture](../foundations/E01-architecture.md) — the `inlayHintProvider` capability.
- **Related:** [F02-builtin-registry](F02-builtin-registry.md) and [F04-user-hints](F04-user-hints.md) — the docs tooltips resolve to; [F07-signature-help](F07-signature-help.md) — the richer call-site surface; [F20-editor-integrations](F20-editor-integrations.md) — where the toggles are configured.

## 17. Changelog

- **2026-06-24** — Initial draft.
- **2026-06-25** — Expanded §11.2 test plan to concrete per-branch rows (T-01–T-19) covering every hint kind and polarity — positional single/multi-param labels, keyword-named no-hint, unknown-callee no-hint, over-arity, half-typed, bare/named endblock echo, hinted/non-hinted loop type, per-toggle isolation, no-initial-tooltip, param/endblock/stale resolve — and grew §12.2 to E2E-01–E2E-06 (added stale-resolve and unresolvable-macro journeys). Rewrote §11.4 to trace each REQ to its T-/E2E ids.
