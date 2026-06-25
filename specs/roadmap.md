# jinja-lsp — Roadmap

> **Status:** Living (continuously maintained)
>
> **Last updated:** 2026-06-24
>
> **Purpose:** The order we build in — milestones, foundation-first, each linked to the spec that defines it.

---

## Sequencing principle

Foundation-first. The shared pipeline (architecture, data model, config, extraction, inline support, testing) comes before any feature. Within the feature tiers we build the *meaning* layer first (diagnostics, the doc registry, packs, hints), then the *read* features that consume it, then the *edit* features, then delivery. Each feature is a pure read or edit over the same index, so once the foundations land the features can largely proceed in parallel.

## M0 — Foundations

The groundwork everything stands on.

- [ ] [E01-architecture](foundations/E01-architecture.md) — process model, two-pass pipeline, transport, capabilities.
- [ ] [E02-folder-structure](foundations/E02-folder-structure.md) — module layout and the downward-dependency rule.
- [ ] [E03-tech-stack](foundations/E03-tech-stack.md) — dependencies, upstream tree-sitter grammar, Rust edition.
- [ ] [E07-data-model](foundations/E07-data-model.md) — symbol types, scopes, the indexes.
- [ ] [E15-app-config](foundations/E15-app-config.md) — config discovery, zero-config fallback, live reload.
- [ ] [E16-conventions](foundations/E16-conventions.md) — error handling, parse recovery, tracing.
- [ ] [E30-extraction-and-indexing](foundations/E30-extraction-and-indexing.md) — queries, discovery, relink.
- [ ] [E31-inline-templates](foundations/E31-inline-templates.md) — inline grammar and embedded-template detection.
- [ ] [E17-testing](foundations/E17-testing.md) — coverage policy, fixtures registry, golden files.
- [ ] [E29-e2e-testing](foundations/E29-e2e-testing.md) — the two E2E branches (golden `check` + pytest-lsp).

## M1 — Diagnostics & CLI

The first thing users feel: real errors in their editor and in CI.

- [ ] [F01-diagnostics](features/F01-diagnostics.md) — the 21-code catalog and `noqa` suppression.
- [ ] [F19-cli-linter](features/F19-cli-linter.md) — `check` with rich/compact/json output.

## M2 — Knowledge layer

The data the interactive features read from.

- [ ] [F02-builtin-registry](features/F02-builtin-registry.md) — the unified doc registry (113 embedded docs).
- [ ] [F03-extension-packs](features/F03-extension-packs.md) — Flask, Starlette, Starlette-Babel, Starlette-Flash.
- [ ] [F04-user-hints](features/F04-user-hints.md) — sidecar + configured hint files.

## M3 — Read features

Navigation and information, each a pure read of the index.

- [ ] [F05-completions](features/F05-completions.md) · [F06-hover](features/F06-hover.md) · [F07-signature-help](features/F07-signature-help.md)
- [ ] [F08-go-to-definition](features/F08-go-to-definition.md) · [F09-find-references](features/F09-find-references.md) · [F10-symbols](features/F10-symbols.md)
- [ ] [F11-document-highlight](features/F11-document-highlight.md) · [F12-folding-range](features/F12-folding-range.md)
- [ ] [F13-semantic-tokens](features/F13-semantic-tokens.md) · [F14-inlay-hints](features/F14-inlay-hints.md)
- [ ] [F15-code-lens](features/F15-code-lens.md) · [F16-call-hierarchy](features/F16-call-hierarchy.md)

## M4 — Edit features

Changing templates safely.

- [ ] [F17-code-actions](features/F17-code-actions.md) — quick-fixes from diagnostics + refactors.
- [ ] [F18-formatting](features/F18-formatting.md) — the Jinja-only formatter (LSP + `format` CLI).

## M5 — Delivery

- [ ] [F20-editor-integrations](features/F20-editor-integrations.md) — VS Code, Zed, Neovim, generic.
- [ ] [F21-release-ci](features/F21-release-ci.md) — CI, cross-compiled binaries, distribution.

## Cross-References

- **Related:** [01-overview](01-overview.md), [index](index.md), [constitution](constitution.md).

## Changelog

- **2026-06-24** — Initial roadmap (M0–M5).
