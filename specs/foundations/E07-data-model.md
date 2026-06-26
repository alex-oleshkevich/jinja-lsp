# E07 — Data Model

> **Status:** Draft
>
> **Version:** 0.2   ·   **Last updated:** 2026-06-25
>
> **Purpose:** The symbols, scopes, and indexes that every feature reads — the precise shape of the facts extraction produces and diagnostics, navigation, and editing consume.

> **Depends on:** [constitution](../constitution.md), [E01-architecture](E01-architecture.md)   ·   **Related:** [E02-folder-structure](E02-folder-structure.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), [E31-inline-templates](E31-inline-templates.md)

> Requirement tag: **DATA**

---

## 1. Purpose & Scope

This spec defines the data jinja-lsp keeps in memory. Extraction ([E30](E30-extraction-and-indexing.md)) produces these types; every feature handler is a pure read of them. Because everyone reads the same facts, this spec is where precision matters most — a wrong field here is a wrong answer in every feature.

This spec covers:

- The six symbol kinds (macros, blocks, variables, import aliases, from-imports, template references).
- References — the usage sites those symbols are read at.
- The nine variable scopes.
- Reference resolution (a use → its binding) and the enclosing owner of a span.
- `TemplateIndex` (per-file) and `WorkspaceIndex` (cross-file).
- The template chain.

## 2. Non-Goals / Out of Scope

- How these types are filled in — owned by [E30-extraction-and-indexing](E30-extraction-and-indexing.md).
- How inline regions become indexes — owned by [E31-inline-templates](E31-inline-templates.md).
- Which checks read which fields — owned by [F01-diagnostics](../features/F01-diagnostics.md).
- The built-in registry (a separate store) — owned by [F02-builtin-registry](../features/F02-builtin-registry.md).

## 3. Background & Rationale

Think of it like a blog post and its comments. One `templates/blog/post.html` owns a handful of facts — the blocks it overrides, the macros it imports, the variables it references. That bundle of per-file facts is a `TemplateIndex`. Stitch every file's index together and resolve the `extends`/`include`/`import` links between them, and you get the `WorkspaceIndex` — the thing a "find references" or "go to definition" actually walks. The types below are jinja-lsp's own design for these per-file and cross-file fact stores.

## 4. Concepts & Definitions

- **Symbol** — an extracted definition: macro, block, variable, import, or template reference. (Canonical definition in [glossary](../glossary.md).)
- **Reference** — a usage site of a symbol. (Canonical definition in [glossary](../glossary.md).)
- **Variable scope** — where a variable is visible. (Canonical definition in [glossary](../glossary.md).)
- **TemplateIndex / WorkspaceIndex** — per-file and cross-file fact stores. (Canonical definitions in [glossary](../glossary.md).)
- **Span** — a byte/line-col range in a source file; every symbol and reference carries one.

## 5. Detailed Specification

### 5.1 Symbol types

A symbol is something a template *defines*. There are six symbol kinds, each with a span and the fields the diagnostics and navigation features need. The struct sketches are in [§8 Data Shapes](#8-data-shapes); this section states the rules that matter.

**REQ-DATA-01 — `MacroDefinition` carries name, params, and body span.**

A `MacroDefinition` records the macro's `name`, its ordered `parameters` (name + optional default), and the span of its body. Parameters drive signature help ([F07](../features/F07-signature-help.md)), inlay parameter hints ([F14](../features/F14-inlay-hints.md)), and the wrong-call-args check (`JINJA-E501`). For `post_url(post)` in `blog/macros.html`, the parameter list is `[post]`.

**REQ-DATA-02 — `BlockDefinition` carries `scoped` and `required` flags.**

A `BlockDefinition` records the block's `name`, its body span, and two booleans: **`scoped`** (the block sees enclosing loop variables) and **`required`** (a child template must override it). Both flags drive the 4xx inheritance checks — `required` powers `JINJA-E403 missing-required-block`. For the `content` block in `base.html`, `required` is `false` unless declared `{% block content required %}`.

**REQ-DATA-03 — `VariableDefinition` carries name, scope, and definition span.**

A `VariableDefinition` records a `name`, the `VariableScope` it lives in, the `span` where it was defined (a `{% set %}`, a `for` target, a `with` binding, a macro parameter, …), and its **`valid_range`** — the lexical region over which the binding is live: the loop body for a `for` target, the `with`/macro/block body for those scopes, the remainder of the template for a top-level `{% set %}`. The scope and `valid_range` determine visibility for the undefined-/unused-variable checks and let a reference resolve to the right binding under shadowing (REQ-DATA-11). `valid_range` is what go-to-definition ([F08](../features/F08-go-to-definition.md)), document-highlight ([F11](../features/F11-document-highlight.md)), and scope-local rename ([F17](../features/F17-code-actions.md)) bound their work to.

**REQ-DATA-04 — Imports are `ImportAlias` or `FromImport`.**

Two import shapes are recorded. An **`ImportAlias`** captures `{% import "blog/macros.html" as macros %}` — the `alias` and the `source` template path. A **`FromImport`** captures `{% from "blog/macros.html" import post_url, comment_card %}` — the `source` and the list of imported `names` (each with an optional alias). These drive `JINJA-W203 unused-import`, the duplicate-import checks, and go-to-definition into the source template.

**REQ-DATA-05 — `TemplateReference` carries `ignore_missing` and `is_dynamic` flags.**

A `TemplateReference` records a cross-template link: its `kind` (`Extends`, `Include`, `Import`, or `From`), the `path`, and two flags that gate the path check (`JINJA-E601 template-does-not-exist`):

- **`ignore_missing`** — set for `{% include "x" ignore missing %}`; the path check must not fire.
- **`is_dynamic`** — set when the path is an expression, not a string literal (e.g. `{% extends layout_var %}`); the path is not statically resolvable, so the path check must not fire.

Both flags are load-bearing for [F01](../features/F01-diagnostics.md)'s path-check edge cases.

**REQ-DATA-06 — `Reference` records every usage site.**

A `Reference` is a *use* of a name, not a definition. It records the referenced name, its `kind` (identifier, attribute access, filter, function call, or test), and its span. References are what `find-references` ([F09](../features/F09-find-references.md)), `document-highlight` ([F11](../features/F11-document-highlight.md)), and the undefined-* / unused-* checks consume. They are the single most-read field in the model. How a reference resolves to the binding it names is REQ-DATA-11.

### 5.2 Variable scopes and reference resolution

Jinja introduces variables in nine distinct ways, and each defines its own scope. A name is visible only within its scope's span, which is what lets the undefined- and unused-variable checks be precise rather than guessy.

**REQ-DATA-07 — Nine variable scopes.**

The `VariableScope` enum has exactly nine variants:

| Scope | Introduced by |
|---|---|
| `Template` | `{% set x = … %}` at template top level |
| `Block` | a `{% block %}` body (and its `scoped` semantics — REQ-DATA-02) |
| `ForLoop` | the loop target(s) of `{% for x in … %}`; also the `loop` object |
| `Macro` | a macro's parameters, inside its body |
| `With` | `{% with x = … %}` bindings |
| `CallBlock` | a `{% call %}` block's `caller()` args |
| `Trans` | `{% trans count=… %}` variables |
| `Filter` | a `{% filter %}` block's scope |
| `Autoescape` | an `{% autoescape %}` block's scope |

**REQ-DATA-11 — A reference resolves to the binding its name and position select.**

A `Reference` resolves to the definition it binds to:

- a **variable** use → the `VariableDefinition` of the same `name` whose `valid_range` contains the reference's span; the **innermost** such binding wins, so an inner `{% for post %}` shadows an outer `post`;
- a **call** → the local or imported `MacroDefinition`;
- an **attribute / filter / test / function** → the registry or hint entry ([F02](../features/F02-builtin-registry.md) / [F04](../features/F04-user-hints.md)).

A reference that resolves to **no** template-owned (or hint-backed) binding — a host-injected context variable, an un-hinted name — is host-owned: this is the single negative contract shared by go-to-definition ([F08](../features/F08-go-to-definition.md)), find-references ([F09](../features/F09-find-references.md)), document-highlight ([F11](../features/F11-document-highlight.md)), and rename ([F17](../features/F17-code-actions.md)). Resolution is computed from the `TemplateIndex` (and, for cross-file calls/imports, the `WorkspaceIndex`); Pass 2 may cache it. **The corollary those features rely on:** a loop / `{% set %}` / `{% with %}` / macro-param variable *is* template-owned and resolvable — it is **not** a host variable — so F08 jumps it, F09/F11 find its uses, and F17 renames it; only the un-hinted host context variable returns nothing.

**REQ-DATA-12 — Every reference and definition has an enclosing owner.**

The **enclosing owner** of a span is the innermost `MacroDefinition` or `BlockDefinition` whose `body` contains it, or the template top level when none does. It is computed by body-span containment (no stored field required). The owner powers call-hierarchy ([F16](../features/F16-call-hierarchy.md)) — grouping incoming calls by the caller's enclosing macro, and collecting a macro's outgoing calls from the references *its own body* contains (not its whole template) — and bounds scope-local rename ([F17](../features/F17-code-actions.md)).

### 5.3 `TemplateIndex` — per-file facts

The `TemplateIndex` is everything we know about one template, extracted from its parse tree. It is replaced atomically on every edit ([E01](E01-architecture.md#52-the-two-pass-pipeline)).

**REQ-DATA-08 — `TemplateIndex` holds one file's symbols and errors.**

A `TemplateIndex` belongs to one template path and holds: the file's `MacroDefinition`s, `BlockDefinition`s, `VariableDefinition`s, `ImportAlias`/`FromImport`s, `TemplateReference`s, and `Reference`s — plus any syntax errors recorded during parsing (`JINJA-E001`). It carries no cross-file knowledge; that is Pass 2's job.

### 5.4 `WorkspaceIndex` — cross-file facts

The `WorkspaceIndex` is the stitched-together view. It maps paths to per-file indexes and resolves the links between them.

**REQ-DATA-09 — `WorkspaceIndex` resolves the cross-file graph.**

A `WorkspaceIndex` maps each template `path` to its `TemplateIndex`, resolves `extends`/`include`/`import`/`from` references to concrete targets, and tracks the import graph (used by `JINJA-E404 recursive-import`). It is the structure every cross-file feature reads — find-references, call-hierarchy, workspace symbols, and the Pass 2 diagnostics. Each workspace folder owns its own `WorkspaceIndex`; cross-folder references are not resolved ([E30](E30-extraction-and-indexing.md)).

### 5.5 The template chain

Inheritance diagnostics and `super()` resolution need to walk from a child up to its root parent.

**REQ-DATA-10 — The template chain is the ordered `extends` lineage.**

The **template chain** for a template is the ordered list from that template, following each `extends` link, to the root template with no parent. For `blog/post.html`, the chain is `[blog/post.html, base.html]`. The chain powers `JINJA-E401 invalid-super`, `JINJA-W402 unreachable-content`, `JINJA-E403 missing-required-block`, and block go-to-definition.

## 8. Data Shapes

These are Rust struct sketches — illustrative shapes, not the final field-for-field source. Every symbol and reference carries a `span`; the sketches below elide the common `span: Span` field except where its meaning needs naming.

The six symbol kinds, the scope enum, and the reference kind:

```rust
// src/workspace/symbols.rs

pub struct MacroDefinition {
    pub name: String,
    pub parameters: Vec<Parameter>,   // ordered; drives F07, F14, E501
    pub body: Span,
    pub span: Span,
}

pub struct Parameter {
    pub name: String,
    pub default: Option<String>,      // present → optional argument
}

pub struct BlockDefinition {
    pub name: String,
    pub scoped: bool,                 // REQ-DATA-02
    pub required: bool,               // drives E403
    pub body: Span,
    pub span: Span,
}

pub struct VariableDefinition {
    pub name: String,
    pub scope: VariableScope,         // REQ-DATA-07
    pub span: Span,                   // the definition site
    pub valid_range: Span,            // REQ-DATA-03 — region the binding is live over
}

pub struct ImportAlias {
    pub alias: String,                // {% import "…" as alias %}
    pub source: String,               // template path
    pub span: Span,
}

pub struct FromImport {
    pub source: String,               // {% from "…" import … %}
    pub names: Vec<ImportedName>,      // each with optional alias
    pub span: Span,
}

pub struct ImportedName {
    pub name: String,
    pub alias: Option<String>,
}

pub struct TemplateReference {
    pub kind: TemplateRefKind,        // Extends | Include | Import | From
    pub path: String,
    pub ignore_missing: bool,         // REQ-DATA-05 — suppresses E601
    pub is_dynamic: bool,             // REQ-DATA-05 — path is an expression
    pub span: Span,
}

pub struct Reference {
    pub name: String,
    pub kind: ReferenceKind,          // Identifier | Attribute | Filter | Function | Test
    pub span: Span,
}

pub enum VariableScope {
    Template, Block, ForLoop, Macro, With, CallBlock, Trans, Filter, Autoescape,
}
```

The two indexes that hold those symbols:

```rust
// src/workspace/index.rs

pub struct TemplateIndex {
    pub path: String,
    pub macros: Vec<MacroDefinition>,
    pub blocks: Vec<BlockDefinition>,
    pub variables: Vec<VariableDefinition>,
    pub import_aliases: Vec<ImportAlias>,
    pub from_imports: Vec<FromImport>,
    pub template_refs: Vec<TemplateReference>,
    pub references: Vec<Reference>,
    pub syntax_errors: Vec<SyntaxError>,   // JINJA-E001
}

pub struct WorkspaceIndex {
    pub templates: HashMap<String, TemplateIndex>,  // path → per-file facts
    // resolved cross-file links + import graph, built in Pass 2
}
```

## 9. Examples & Use Cases

Take `blog/post.html` from the `starlette-blog` cast. It `extends "base.html"`, fills the `content` block, and calls `post_url(post)`. Its `TemplateIndex` holds one `BlockDefinition` (`content`), one `TemplateReference` (kind `Extends`, path `base.html`, neither flag set), and `Reference`s for `post`, `post.title`, and the `post_url` call. After Pass 2, the `WorkspaceIndex` resolves the `Extends` reference to `base.html`'s index, so the template chain `[blog/post.html, base.html]` is available — and a `super()` call in the `content` block can be validated against the parent's `content` block.

## 10. Edge Cases & Failure Modes

- **Dynamic extends path** (`{% extends layout_var %}`) → `TemplateReference.is_dynamic = true`; the template chain stops here and `JINJA-E601` does not fire.
- **`include … ignore missing`** → `ignore_missing = true`; a missing target is silent.
- **A symbol with no usages** → it simply has no matching `Reference`s; the unused-* checks read that absence, no special flag needed.
- **A reference to a name defined in no scope** → resolves to no `VariableDefinition` (REQ-DATA-11); it is host-owned (a hint may name it — [F04](../features/F04-user-hints.md)), and `JINJA-E101` fires unless a hint suppresses it.
- **A reference inside a shadowing inner scope** (`{% for post %}…{{ post }}` nested under an outer `post`) → resolves to the **innermost** binding whose `valid_range` contains it (REQ-DATA-11), never the outer same-named one.
- **A reference outside every binding's `valid_range`** (a loop variable used after `{% endfor %}`) → resolves to no scope-local binding; host-owned.

## 11. Testing

This foundation is verified by extraction tests that assert the produced symbols match expected shapes against fixtures.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior is covered.** Every `REQ-DATA-NN` maps to at least one test. See the policy in [E17-testing](E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| Macro params extracted in order with defaults | unit | [starlette-blog](E17-testing.md#starlette-blog) | REQ-DATA-01 |
| `scoped`/`required` flags set from block modifiers | unit | [inheritance](E17-testing.md#inheritance) | REQ-DATA-02 |
| Each of the nine scopes is assigned correctly | unit | [undefined-vars](E17-testing.md#undefined-vars) | REQ-DATA-07 |
| `valid_range` spans the binding's live region (loop body, `with`/macro body, rest-of-template for top-level `set`) | unit | [undefined-vars](E17-testing.md#undefined-vars) | REQ-DATA-03 |
| A variable reference resolves to the innermost binding whose `valid_range` contains it (shadowing); an un-hinted name resolves to nothing | unit | [undefined-vars](E17-testing.md#undefined-vars) | REQ-DATA-11 |
| A reference's/definition's enclosing owner is the innermost macro/block body containing it, else the template | unit | [starlette-blog](E17-testing.md#starlette-blog) | REQ-DATA-12 |
| `ignore_missing` and `is_dynamic` set on the right refs | unit | [call-and-paths](E17-testing.md#call-and-paths) | REQ-DATA-05 |
| WorkspaceIndex resolves extends/import targets | integration | [inheritance](E17-testing.md#inheritance) | REQ-DATA-09 |
| Template chain ordered child→root | integration | [inheritance](E17-testing.md#inheritance) | REQ-DATA-10 |

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-DATA-01 | macro-params extraction test |
| REQ-DATA-02 | block-flags extraction test |
| REQ-DATA-03 | variable-definition test |
| REQ-DATA-04 | import-shapes test |
| REQ-DATA-05 | template-ref-flags test |
| REQ-DATA-06 | reference-kinds test |
| REQ-DATA-07 | nine-scopes test |
| REQ-DATA-08 | per-file index test |
| REQ-DATA-09 | workspace-resolution test |
| REQ-DATA-10 | template-chain test |
| REQ-DATA-11 | reference-resolution + shadowing test |
| REQ-DATA-12 | enclosing-owner test |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — in-memory only; no trust boundary. The model holds only the user's own source spans and names.
- **Input & validation** — all fields originate from tree-sitter parsing of untrusted templates; parsing is memory-safe and never executes content (P1).
- **Data sensitivity** — none beyond the user's source.

### 13.4 Performance & Scale

- **Volume & scale** — one `TemplateIndex` per template; the `WorkspaceIndex` must hold 500 templates within the < 2 s rebuild budget ([E30](E30-extraction-and-indexing.md), P6). Symbols are stored, not recomputed per query.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — P1, P4; [E01-architecture](E01-architecture.md) — the passes that fill and read these types.
- **Related:** [E02-folder-structure](E02-folder-structure.md) — these types live in `workspace/`; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — how the types are populated; [E31-inline-templates](E31-inline-templates.md) — inline regions produce ordinary `TemplateIndex` entries; [F01-diagnostics](../features/F01-diagnostics.md) — the checks that read these fields; [F02-builtin-registry](../features/F02-builtin-registry.md) — the separate registry store.

## 17. Changelog

- **2026-06-25** — v0.2: added `valid_range` to `VariableDefinition` (REQ-DATA-03), and the reference-resolution (REQ-DATA-11) and enclosing-owner (REQ-DATA-12) rules — the shared backing that go-to-definition (F08), find-references (F09), document-highlight (F11), call-hierarchy (F16), and rename (F17) depend on, and that establishes scope-local variables as template-owned (not host-owned).
- **2026-06-24** — Initial draft: six symbol types with the `scoped`/`required`/`ignore_missing`/`is_dynamic` flags, the nine variable scopes, `TemplateIndex`/`WorkspaceIndex`, and the template chain.
- **2026-06-24** — §1 scope list now states the six symbol kinds correctly and lists references separately (a reference is a usage site, not a symbol — REQ-DATA-06).
