# jinja-lsp — Specification Index

> **Status:** Living (continuously maintained)
>
> **Last updated:** 2026-06-24
>
> **Purpose:** The map of the whole specification suite — every spec, what it defines, when to load it, and how finished it is. Start here.

jinja-lsp is a specialist language server for Jinja2 templates: static, editor-agnostic, a companion to the host Python LSP. The suite is organized in tiers — meta rules first, then product framing, then the engineering foundations the features stand on, then the features themselves, then the decision records.

**Foundation specs (`E##`) describe _how_ the server is built. Feature specs (`F##`) describe _what_ each feature does** and own their own diagnostics, surfaces, and tests.

## Status legend

✅ Approved · 📝 In Review · ✏️ Draft · ♻️ Deprecated · ⛔ Rejected

## Tier 1 — Meta

| Spec | Purpose | Load this when | Status |
|---|---|---|---|
| [constitution](constitution.md) | Governing principles, the diagnostic code scheme, authoring conventions | Writing or reviewing any spec | ✅ |
| [glossary](glossary.md) | Canonical definition of every domain term | A term is unclear | ✅ |

## Tier 2 — Product

| Spec | Purpose | Load this when | Status |
|---|---|---|---|
| [01-overview](01-overview.md) | What jinja-lsp is, in plain language | Onboarding to the project | ✅ |
| [roadmap](roadmap.md) | Build order — M0 foundations → M5 delivery | Planning what to build next | ✅ |

## Tier 3 — Foundations

| Spec | Purpose | Load this when | Status |
|---|---|---|---|
| [E01-architecture](foundations/E01-architecture.md) | Three front-ends, two-pass pipeline, capabilities | Understanding how it all fits | ✏️ |
| [E02-folder-structure](foundations/E02-folder-structure.md) | `src/` module layout, downward-dependency rule | Placing new code | ✏️ |
| [E03-tech-stack](foundations/E03-tech-stack.md) | Dependencies, upstream tree-sitter grammar, Rust edition, test deps | Setting up the crate | ✏️ |
| [E07-data-model](foundations/E07-data-model.md) | Symbol types, scopes, TemplateIndex/WorkspaceIndex | Touching the index | ✏️ |
| [E15-app-config](foundations/E15-app-config.md) | Config discovery, zero-config fallback, live reload | Touching config | ✏️ |
| [E16-conventions](foundations/E16-conventions.md) | Error handling, parse recovery, tracing | Writing engine code | ✏️ |
| [E17-testing](foundations/E17-testing.md) | Coverage policy, fixtures registry, golden files | Writing any test plan | ✏️ |
| [E29-e2e-testing](foundations/E29-e2e-testing.md) | The two E2E branches (golden `check` + pytest-lsp) | Writing an E2E plan | ✏️ |
| [E30-extraction-and-indexing](foundations/E30-extraction-and-indexing.md) | Tree-sitter queries, discovery, relink | Touching extraction | ✏️ |
| [E31-inline-templates](foundations/E31-inline-templates.md) | Inline grammar, embedded-template detection | Supporting inline Jinja | ✏️ |

## Tier 4 — Features

Numbered in build order: the meaning layer (diagnostics, registry, packs, hints), then read features, then edit features, then delivery.

| Spec | Purpose | Load this when | Status |
|---|---|---|---|
| [F00-template](features/F00-template.md) | Boilerplate for new feature specs | Starting a new feature | — |
| [F01-diagnostics](features/F01-diagnostics.md) | The 21-code catalog + `noqa` suppression | Adding/auditing a check | ✏️ |
| [F02-builtin-registry](features/F02-builtin-registry.md) | The unified doc registry (113 embedded docs) | Touching docs/hover data | ✏️ |
| [F03-extension-packs](features/F03-extension-packs.md) | Flask, Starlette, Starlette-Babel, Starlette-Flash packs | Adding a framework pack | ✏️ |
| [F04-user-hints](features/F04-user-hints.md) | Sidecar + configured hint files; `W106` | Supporting project symbols | ✏️ |
| [F05-completions](features/F05-completions.md) | Context-aware completions + resolve | Working on completion | ✏️ |
| [F06-hover](features/F06-hover.md) | Hover docs for symbols and attributes | Working on hover | ✏️ |
| [F07-signature-help](features/F07-signature-help.md) | Param hints in macro/filter calls | Working on signatures | ✏️ |
| [F08-go-to-definition](features/F08-go-to-definition.md) | Jump to macro/template/block defs | Working on navigation | ✏️ |
| [F09-find-references](features/F09-find-references.md) | Workspace-wide usages of a symbol | Working on references | ✏️ |
| [F10-symbols](features/F10-symbols.md) | Document outline + workspace symbol search | Working on symbols | ✏️ |
| [F11-document-highlight](features/F11-document-highlight.md) | In-file occurrence highlighting | Working on highlights | ✏️ |
| [F12-folding-range](features/F12-folding-range.md) | Fold blocks/loops/macros/comments | Working on folding | ✏️ |
| [F13-semantic-tokens](features/F13-semantic-tokens.md) | Semantic coloring beyond tree-sitter | Working on highlighting | ✏️ |
| [F14-inlay-hints](features/F14-inlay-hints.md) | Param-name + endblock-echo hints | Working on inlay hints | ✏️ |
| [F15-code-lens](features/F15-code-lens.md) | Reference/inheritance counts above symbols | Working on code lens | ✏️ |
| [F16-call-hierarchy](features/F16-call-hierarchy.md) | Incoming/outgoing macro calls | Working on call hierarchy | ✏️ |
| [F17-code-actions](features/F17-code-actions.md) | Quick-fixes (from F01) + refactors | Working on fixes/refactors | ✏️ |
| [F18-formatting](features/F18-formatting.md) | Jinja-only formatter (LSP + `format` CLI) | Working on formatting | ✏️ |
| [F19-cli-linter](features/F19-cli-linter.md) | `check` with rich/compact/json output | Working on the CLI | ✏️ |
| [F20-editor-integrations](features/F20-editor-integrations.md) | VS Code, Zed, Neovim, generic clients | Shipping an editor extension | ✏️ |
| [F21-release-ci](features/F21-release-ci.md) | CI, cross-compiled binaries, distribution | Working on releases | ✏️ |

## Decisions

Append-only. Never edit or delete a past ADR; supersede it with a new one.

| ADR | Decision | Date | Status |
|---|---|---|---|
| [ADR-000-template](decisions/ADR-000-template.md) | Template | — | — |
| [ADR-001-language-and-runtime](decisions/ADR-001-language-and-runtime.md) | Rust over Python | 2026-06-24 | ✅ Accepted |
| [ADR-002-tree-sitter-grammar](decisions/ADR-002-tree-sitter-grammar.md) | Use the upstream tree-sitter-jinja grammar | 2026-06-24 | ✅ Accepted |
| [ADR-003-diagnostic-code-scheme](decisions/ADR-003-diagnostic-code-scheme.md) | Numeric codes; slug is output-only | 2026-06-24 | ✅ Accepted |
| [ADR-004-embedded-builtin-docs](decisions/ADR-004-embedded-builtin-docs.md) | `include_str!()` docs at compile time | 2026-06-24 | ✅ Accepted |
| [ADR-005-live-config-reload](decisions/ADR-005-live-config-reload.md) | Diff-reload config without restart | 2026-06-24 | ✅ Accepted |
| [ADR-006-hint-file-discovery](decisions/ADR-006-hint-file-discovery.md) | Sidecar + configured dirs + fallback | 2026-06-24 | ✅ Accepted |
| [ADR-007-formatting-strategy](decisions/ADR-007-formatting-strategy.md) | Format the Jinja layer only | 2026-06-24 | ✅ Accepted |
| [ADR-008-code-action-strategy](decisions/ADR-008-code-action-strategy.md) | Code actions derived from diagnostics | 2026-06-24 | ✅ Accepted |
| [ADR-009-stdio-only-transport](decisions/ADR-009-stdio-only-transport.md) | stdio is the only transport | 2026-06-24 | ✅ Accepted |
| [ADR-010-pypi-distribution](decisions/ADR-010-pypi-distribution.md) | Ship to PyPI as maturin wheels | 2026-06-25 | ✅ Accepted |

## Deprecated

| Spec | Superseded by | Status |
|---|---|---|
| <none yet> | | |

## Rejected

| Spec | Why rejected | Status |
|---|---|---|
| <none yet> | | |

## Out of scope

The suite deliberately does not cover: full host-language/HTML formatting, the dedicated `textDocument/rename` protocol method (rename ships as an [F17](features/F17-code-actions.md) code-action command instead), template rendering/execution, or generic host-language intelligence (that's the companion Python LSP's job). See the constitution §4.7 Non-Goals.

## Maintenance rule

When you author or change a spec, update its row here in the same edit. When a spec is **deprecated**, move it to `deprecated/` and list it above. When a proposal is **rejected**, move it to `rejected/` and list it.

## Changelog

- **2026-06-24** — Initial index: 5 meta, 10 foundations, 22 features (incl. F00 template), 10 ADRs (incl. ADR-000 template). Suite drafted per the approved plan.
