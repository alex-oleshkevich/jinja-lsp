# Constitution

> **Status:** Approved
>
> **Version:** 1.0   ·   **Last updated:** 2026-06-24
>
> **Purpose:** The governing rules for both the product and its specs — the principles jinja-lsp must honor, and the conventions every spec in this suite must follow.

---

## 1. Purpose & Scope

This document is the single source of authority for the jinja-lsp spec suite. It governs two things: the **product principles** the language server must honor no matter which feature you're building, and the **authoring conventions** every spec obeys so the suite stays a connected whole rather than a pile of docs.

Read this first. When a feature spec and the constitution disagree, the constitution wins — or the constitution is wrong and you change it here, deliberately, with a changelog entry.

## 2. Product Principles

These are the non-negotiable beliefs jinja-lsp is built on. A feature that violates one of them is wrong, however useful it seems. Specs cite them as "per P3".

| # | Principle | What it means |
|---|---|---|
| P1 | **Static analysis only** | We parse templates with tree-sitter; we never import, render, or execute them or the host Python. Every answer comes from the syntax tree and the workspace index, never from running code. |
| P2 | **Editor-agnostic, stdio only** | We speak standard LSP over stdio. No editor-specific coupling, and no alternate transport — stdio is the only listener (ADR-009). |
| P3 | **Never corrupt, never panic** | tree-sitter always yields a tree, even for half-typed templates; we degrade gracefully and never panic on partial input. Edits and formatting are round-trip safe — a formatted file re-formats to itself and re-parses to an equivalent tree. |
| P4 | **Only flag what is positively wrong** | No guessing. We raise a diagnostic only when we can prove something is wrong; anything we can't resolve stays silent. Every check is configurable via `lint.select`/`lint.ignore`. |
| P5 | **Companion, not replacement** | We run *alongside* the host Python LSP (Pyright/pylsp), never shadowing it. We own the Jinja layer end-to-end — diagnostics, navigation, edits, and Jinja-aware formatting — but we never format or analyze the host language (HTML/SQL/text); that stays with its own tools. |
| P6 | **Fast enough to forget** | Completions return in < 100 ms; a full workspace index of 500 templates rebuilds in < 2 s. The server is invisible because it's quick. |

## 3. Engineering Principles

These guide technical decisions without being rules you cite by number.

- **One engine, three front-ends.** Extraction, indexing, and diagnostics live in one pipeline. The `lsp` server, the `check` linter, and the `format` command are thin I/O layers over it — they never grow their own analysis logic, so they can't drift.
- **Extract facts, then read them.** Pass 1 extracts facts from one file's tree; Pass 2 links them into a workspace index. Every feature handler is a *pure read* of that index — no parsing, no shared mutable state in handlers.
- **Dependencies flow downward.** Features depend on foundations; foundations never depend on features. `features/` may read `workspace/`, `builtins/`, `edit/`, `format/`; nothing imports `features/`.
- **One source of truth.** Each fact (a config key's default, a diagnostic's slug, a template's symbols) is defined in exactly one place and linked from everywhere else.
- **Degrade, don't fail.** A broken config, a missing template, an unparseable hint — each is logged and worked around, never fatal.

## 4. Authoring Conventions

### 4.1 Document template

Every spec follows `features/F00-template.md`: the metadata header, then numbered sections. Required everywhere: **Purpose & Scope**, **Detailed Specification**, **Cross-References**, **Changelog**. Features additionally require **Testing** and **§13.1 Security & Privacy**. Specs with an editor surface require a **UI Mockups** section.

### 4.2 Naming & ID schemes

- **Files:** prefix + number + kebab slug. `E##` engineering foundations, `F##` features. Meta-docs are `index.md`, `constitution.md`, `glossary.md`, `01-overview.md`, `roadmap.md`.
- **Reserved names:** foundation and meta names follow the shared reserved-names registry (`references/reserved-names.md`). `E31-inline-templates` is a new shared name appended to that registry for this project.
- **Requirement IDs:** each spec declares a short uppercase tag (e.g. `DIAG`); load-bearing rules are `REQ-DIAG-01`, open questions `OQ-DIAG-01`.
- **Diagnostic codes:** the scheme below.

#### Diagnostic code scheme

Every diagnostic has a numeric code `JINJA-<SEV><CLASS><NN>` and a kebab-case **slug**:

- **`<SEV>`** ∈ `E` (error), `W` (warning), `I` (info), `H` (hint).
- **`<CLASS>`** is the hundreds digit, grouping related checks.
- The slug is a stable kebab-case label shown next to the code in output, e.g. `JINJA-E101 undefined-variable`.

**Output shows both; input accepts code or class-prefix only.** `lint.select`, `lint.ignore`, and `noqa` directives take a full code (`JINJA-E101`) or a class prefix (`JINJA-E1` = all 1xx, `JINJA-W` = all warnings). The slug is a human-readable output label, **not** an input identifier — one input grammar, no redundant aliasing (ADR-003).

The 21 codes below make up the diagnostic catalog.

| Code | Slug | Class |
|---|---|---|
| `JINJA-E001` | `syntax-error` | 0xx syntax |
| `JINJA-E101` | `undefined-variable` | 1xx undefined/unknown |
| `JINJA-E102` | `undefined-filter` | |
| `JINJA-E103` | `undefined-function` | |
| `JINJA-E104` | `undefined-test` | |
| `JINJA-W106` | `unknown-attribute` | hint-gated, off by default |
| `JINJA-W107` | `invalid-noqa` | bad code/prefix in a `noqa` |
| `JINJA-W201` | `unused-variable` | 2xx unused |
| `JINJA-W202` | `unused-macro` | |
| `JINJA-W203` | `unused-import` | |
| `JINJA-W301` | `duplicate-block` | 3xx duplicate/shadow |
| `JINJA-W302` | `duplicate-macro` | |
| `JINJA-W303` | `duplicate-import-alias` | |
| `JINJA-W304` | `duplicate-from-import` | |
| `JINJA-W305` | `name-shadowing` | |
| `JINJA-E401` | `invalid-super` | 4xx inheritance |
| `JINJA-W402` | `unreachable-content` | |
| `JINJA-E403` | `missing-required-block` | |
| `JINJA-E404` | `recursive-import` | |
| `JINJA-E501` | `wrong-call-args` | 5xx call/arg |
| `JINJA-E601` | `template-does-not-exist` | 6xx path |

That is **21 codes total**. Each slug is a stable kebab-case label, fixed once and never reused. The full catalog with detection rules lives in [F01-diagnostics](features/F01-diagnostics.md).

### 4.3 Crosslinking & the index

Specs link to each other inline and list every connection in their Cross-References section. [`index.md`](index.md) is updated in the same edit as any spec change.

### 4.4 Testing & coverage

Every feature ships a test plan covering **100% of its scope** — each `REQ-<TAG>-NN` maps to a test, every screen state and edge case is covered. Shared rules, tools, and the fixtures registry live in [E17-testing](foundations/E17-testing.md); end-to-end rules and the harness in [E29-e2e-testing](foundations/E29-e2e-testing.md). Two E2E branches: **golden-file `check` fixtures** (Rust) for diagnostics, and **`pytest-lsp`** for LSP-protocol journeys. Feature specs link to these foundations rather than restating them.

### 4.5 Status lifecycle & changelog

A spec moves `Draft → In Review → Approved`, ending in one of two terminal states:

- **Deprecated** — was Approved, now superseded. Set the status and move the file to `deprecated/`.
- **Rejected** — considered and turned down. Set the status and move the file to `rejected/`.

Archived specs keep their name; the index lists them in its Deprecated and Rejected sections. Every change gets a dated changelog entry; versions bump on meaningful change.

### 4.6 Non-functional & operational scope

Decided once here; every feature spec obeys it. The kickoff interview covered features, extras, and stack; these non-functional choices are reasoned defaults for a latency-sensitive CLI/LSP dev tool.

| Concern | Spec section | Status |
|---|---|---|
| Security & Privacy | §13.1 | **Required** |
| Accessibility | §13.2 | **N/A** — no GUI; the editor renders all UI |
| Permissions & Roles | §13.3 | **N/A** — single-user developer tool |
| Performance & Scale | §13.4 | **Enabled** — latency budgets per P6 (owned by E01/E30) |
| Observability | §13.5 | **Enabled (lightweight)** — `tracing` spans on slow paths (E16); no metrics backend |
| Rollout & Migration | §14 | **N/A** — greenfield, versioned releases (F21) |
| Acceptance criteria & DoD | §12.3 | **N/A** — the E2E scenarios serve as the contract |

### 4.7 Non-Goals (suite-wide)

Recorded so they read as decisions, not omissions:

- **Full host-language / HTML formatting** — we format Jinja only ([F18](features/F18-formatting.md)); recommend djLint/Prettier for HTML.
- **The `textDocument/rename` / `prepareRename` protocol method** — not implemented as a dedicated method; workspace rename is delivered instead as an [F17](features/F17-code-actions.md) code-action command, built on [F09](features/F09-find-references.md)'s reference graph.
- **`inlineValue`, `moniker`, `selectionRange`, `typeHierarchy`, `declaration`, `implementation`** — low value for templates.
- **Executing or rendering templates** — forbidden by P1.

## 5. The Recurring Example Cast

To keep examples concrete and consistent, every spec draws from the same world: **`starlette-blog`**, a small Starlette blog. It is also the canonical [E17-testing](foundations/E17-testing.md) fixture; its `jinja.toml` sets `templates = ["templates"]` and `extras = ["starlette"]` so `request` resolves.

- **`templates/base.html`** — the base layout, defining the `head`, `body`, `content`, and `footer` blocks.
- **`templates/blog/post.html`** — extends `base.html`, fills the `content` block, imports `post_url` and `comment_card` from `blog/macros.html`, and calls them.
- **`templates/blog/macros.html`** — defines the `post_url(post)` and `comment_card(comment, show_actions)` macros.
- **`templates/email/digest.html`** — extends `base.html`, imports `post_url` from `blog/macros.html`, and uses the Starlette `request` global.
- **`post`** — the recurring context variable: a blog post with `.title`, `.slug`, `.body`, `.author`.

## 6. Visualization Style Guide

- **ASCII mockups** (~78 columns) for **every editor surface** a spec introduces — completion popup, hover card, signature-help popup, code-action menu, inlay-hint rendering, codelens line, folding, document-highlight, semantic-token legend. They live in each spec's UI Mockups section. The CLI `check` report is mocked for **every** `--format` value ([F19](features/F19-cli-linter.md)). Follow the skill's `references/ascii-mockups.md`.
- **Mermaid** for flows, lifecycles, and state machines (the two-pass pipeline, the relink flow) — follow `references/mermaid.md`. These live in Visualizations, not UI Mockups.
- **Tables** for matrices, catalogs, and decision matrices (the code catalog, the capability matrix).

## 7. Cross-References

- **Related:** [index](index.md), [glossary](glossary.md), [roadmap](roadmap.md), [01-overview](01-overview.md).

## 8. Changelog

- **2026-06-24** — Initial constitution: P1–P6, the 21-code diagnostic scheme (code + class-prefix input, slug output label), §4.6 non-functional scope, suite-wide Non-Goals, and the `starlette-blog` example cast.
- **2026-06-24** — Cast reconciliation: `base.html` now lists the `content` block it owns; `post.html` imports and calls both `post_url` and `comment_card`; `comment_card` takes `(comment, show_actions)`; `digest.html` extends `base.html` and imports `post_url` (was loosely "includes"), matching the feature specs.
