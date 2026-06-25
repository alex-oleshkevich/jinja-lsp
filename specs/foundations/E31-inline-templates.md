# E31 — Inline Templates

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** How jinja-lsp analyzes Jinja that lives inside host files — the inline/expression grammar inside the delimiters, embedded templates in host code, and the range mapping that keeps every position in host-file coordinates.

> **Depends on:** [constitution](../constitution.md), [E03-tech-stack](E03-tech-stack.md), [E07-data-model](E07-data-model.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md)   ·   **Related:** [E01-architecture](E01-architecture.md), [E15-app-config](E15-app-config.md)

> Requirement tag: **INLN**

---

## 1. Purpose & Scope

This spec defines how jinja-lsp parses and indexes Jinja that isn't a standalone template file — the capability the Python project's separate `jinja_inline` grammar provided. There are two shapes: the sub-language *inside* the delimiters, and whole templates *embedded* in host code like a Python `render_template_string("…")`. Both end up as ordinary `TemplateIndex` entries so every feature works in them unchanged.

This spec covers:

- The expression/inline grammar inside `{{ }}` / `{% %}`.
- Embedded templates detected by host patterns.
- Range mapping back to host-file coordinates.
- The scope boundary (a non-goal: no full host-AST parsing).
- Feature uniformity through ordinary `TemplateIndex` entries.

## 2. Non-Goals / Out of Scope

- The grammars themselves — owned by [E03-tech-stack](E03-tech-stack.md).
- The index types inline ranges produce — owned by [E07-data-model](E07-data-model.md).
- How produced ranges are indexed and relinked — owned by [E30-extraction-and-indexing](E30-extraction-and-indexing.md).
- **Full host-language (Python/HTML) AST parsing** — explicitly out of scope (REQ-INLN-04); a stretch goal, not v1.

## 3. Background & Rationale

Jinja rarely lives alone. The interesting analysis is *inside* the delimiters — `{{ post.title | upper }}` is only checkable if something parses `post.title` and `upper` as an attribute and a filter. And Jinja often lives inside other code entirely: a Starlette view calls `render_template_string("<h1>{{ post.title }}</h1>")`, and that string is a real template the user would love to get diagnostics on. This spec brings both into the same index the standalone `.html` files use, so there's no second code path and no per-feature special-casing.

## 4. Concepts & Definitions

- **Inline template** — Jinja embedded in a host file. (Canonical definition in [glossary](../glossary.md).)
- **Inline (expression) grammar** — the grammar parsing the sub-language inside the delimiters. (Canonical definition under [glossary](../glossary.md) "Expression".)
- **Embedded template** — a Jinja string literal inside host code, detected by a host pattern.
- **Range map** — the offset translation from an extracted substring back to host-file coordinates.

## 5. Detailed Specification

### 5.1 The two inline shapes

Inline analysis has two distinct shapes, and it helps to name them before the rules. One is *intra-delimiter* (the expression sub-language); the other is *embedded* (a whole template inside host code).

### 5.2 Shape 1 — the expression/inline grammar

The content between `{{ … }}` and `{% … %}` is its own sub-language. The upstream grammar parses it.

**REQ-INLN-01 — The inline grammar parses intra-delimiter content.**

The content inside `{{ … }}` and `{% … %}` is parsed by the inline (expression) grammar that the upstream grammar ships ([E03](E03-tech-stack.md)). This is what makes attribute access, filters, tests, and function calls inside delimiters analyzable down to the individual node. Without it, `{{ post.title | upper }}` would be opaque text; with it, `post`, `.title`, and `upper` are each a `Reference` ([E07](E07-data-model.md)) the diagnostics and navigation features can read.

### 5.3 Shape 2 — embedded templates

A whole Jinja template can sit inside a host-language string. jinja-lsp detects these by configurable patterns and treats each as its own template.

**REQ-INLN-02 — Embedded templates are detected by host patterns.**

Jinja strings inside host code — the canonical case is `render_template_string("…")` in Python — are detected by **configurable host patterns**. Each match becomes an inline `TemplateIndex` range fed into the discovery/extraction pipeline ([E30](E30-extraction-and-indexing.md)). The set of recognized render-function names is the `inline_patterns` config key ([E15](E15-app-config.md)), so a project can teach jinja-lsp about its own template-rendering helpers.

### 5.4 Range mapping

The Jinja inside a host file is at some offset within that file. Every position the LSP reports must be translated back to where the user actually sees it.

**REQ-INLN-03 — Inline ranges map back to host-file coordinates.**

Each inline range keeps a source-offset map. Diagnostics, hover, go-to-definition, and edits report positions in **host-file coordinates** — the line and column in the `.py` (or other host) file — never the offset within the extracted substring. A diagnostic on `post.title` inside `render_template_string("{{ post.title }}")` points at the column in the Python file, so the squiggle lands where the user is looking.

### 5.5 The scope boundary

v1 detects embedded templates cheaply, by pattern. It does not understand the host language.

**REQ-INLN-04 — Lightweight pattern detection, not host-AST parsing.**

v1 detects embedded templates with **lightweight patterns** — string literals passed to known render functions — **not** by fully parsing the host language. Full host-AST analysis (following a variable that holds a template string, tracking f-string interpolation, resolving imports) is **out of scope** for v1 and documented as a stretch goal. This keeps the feature small and predictable; a string that isn't a direct literal argument to a recognized call is simply not detected.

### 5.6 Feature uniformity

The payoff of routing inline ranges through the ordinary index is that no feature has to know they exist.

**REQ-INLN-05 — Inline ranges are ordinary `TemplateIndex` entries.**

Because an inline range produces an ordinary `TemplateIndex` entry ([E07](E07-data-model.md)), every feature (F01–F18) works inside it with **no per-feature special-casing**. Completions, hover, diagnostics, and the rest read the index the same way whether the source was `base.html` or a Python string. The only inline-specific code is detection (REQ-INLN-02) and range mapping (REQ-INLN-03); everything downstream is shared.

## 8. Data Shapes

An inline range records where it came from and how to translate positions back. This is the offset map that REQ-INLN-03 relies on:

```rust
// src/workspace/inline.rs
pub struct InlineRange {
    pub host_path: String,     // the .py (or other host) file
    pub host_offset: usize,    // byte offset of the inline content in the host file
    pub host_line: u32,        // starting line of the inline content
    pub host_col: u32,         // starting column of the inline content
    // a TemplateIndex is built over the extracted substring; positions in it
    // are translated to host coordinates using the offsets above (REQ-INLN-03).
}
```

## 9. Examples & Use Cases

In `starlette-blog`, a quick endpoint renders a banner inline:

```python
# app/views.py
return render_template_string("<h1>{{ post.titel }}</h1>")
```

jinja-lsp's host pattern matches `render_template_string("…")`, extracts the string as an inline template, and runs the same `references` query over it. `post.titel` (a typo for `title`) becomes a `Reference`; if `post` is hinted with a `title` attribute, `JINJA-W106 unknown-attribute` fires — and thanks to range mapping, the diagnostic points at the column inside `views.py`, not at offset 4 of the substring.

## 10. Edge Cases & Failure Modes

- **A template string held in a variable**, not passed directly to the render call → not detected in v1 (REQ-INLN-04); no analysis, no false diagnostic.
- **An embedded template with a syntax error** → `JINJA-E001` recorded, reported at host-file coordinates ([E16](E16-conventions.md), REQ-INLN-03).
- **An f-string / concatenated template** → not a single literal; not detected in v1.
- **A custom render helper** → not analyzed unless its pattern is added to the configured host patterns (REQ-INLN-02).

## 11. Testing

This foundation is verified by detection unit tests, range-mapping tests asserting host coordinates, and a feature-uniformity test.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior is covered.** Every `REQ-INLN-NN` maps to at least one test. See the policy in [E17-testing](E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Intra-delimiter content parses to references | unit | [starlette-blog](E17-testing.md#starlette-blog) | REQ-INLN-01 |
| `render_template_string("…")` is detected | unit | [call-and-paths](E17-testing.md#call-and-paths) | REQ-INLN-02 |
| Diagnostic position is in host-file coordinates | unit | [call-and-paths](E17-testing.md#call-and-paths) | REQ-INLN-03 |
| A non-literal template arg is not detected | unit | [call-and-paths](E17-testing.md#call-and-paths) | REQ-INLN-04 |
| A feature (e.g. hover) works unchanged in an inline range | integration | [call-and-paths](E17-testing.md#call-and-paths) | REQ-INLN-05 |

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-INLN-01 | inline-grammar parse test |
| REQ-INLN-02 | embedded-detection test |
| REQ-INLN-03 | range-mapping test |
| REQ-INLN-04 | non-literal non-detection test |
| REQ-INLN-05 | feature-uniformity test |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — host files are read from the workspace only; detection never executes host code (P1). No network access.
- **Input & validation** — embedded template strings are untrusted; they are parsed, never run. Pattern detection is purely textual.
- **Data sensitivity** — none beyond the user's source.

### 13.4 Performance & Scale

- **Latency** — inline detection is a lightweight pattern scan; it adds negligible cost to the per-file extraction budget ([E30](E30-extraction-and-indexing.md)).

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P5; [E03-tech-stack](E03-tech-stack.md) — the inline grammar; [E07-data-model](E07-data-model.md) — the `TemplateIndex` entries inline ranges produce; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — how those entries are indexed.
- **Related:** [E01-architecture](E01-architecture.md) — the lifecycle that triggers host-file extraction; [E15-app-config](E15-app-config.md) — configurable host patterns.

## 17. Changelog

- **2026-06-24** — Initial draft: the inline/expression grammar, embedded-template detection by host pattern, host-coordinate range mapping, the no-host-AST scope boundary, and feature uniformity via ordinary `TemplateIndex` entries.
- **2026-06-24** — Bound the configurable host patterns to E15's `inline_patterns` config key.
