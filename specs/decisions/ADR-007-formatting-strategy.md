# ADR-007 — Format the Jinja layer only

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

Templates are a host language (HTML, SQL, plain text) with Jinja woven through it. We wanted to offer formatting ([F18](../features/F18-formatting.md)) — normalize `{{x}}` to `{{ x }}`, fix delimiter and pipe spacing, re-indent block bodies — because inconsistent Jinja spacing is a real annoyance and no other tool owns it well. But P5 says we're a companion, not a replacement: we never analyze or format the host language, which has its own mature formatters (Prettier, djLint). A formatter that reflowed HTML would compete with those tools and, worse, risk corrupting markup we don't fully parse. And P3 makes corruption unforgivable — an edit that breaks a template is the one thing a template tool must never do.

## Decision

The formatter normalizes the Jinja layer only and never touches host-language bytes — HTML, SQL, and text pass through verbatim. Round-trip safety is mandatory: formatting a formatted file is a no-op, and the output re-parses to an equivalent tree.

## Consequences

This softens P5 in a controlled way — we now own Jinja edits end to end (diagnostics, code actions, *and* formatting), which is a coherent ownership story, while still leaving the host language entirely to its own tools. Users get consistent Jinja spacing without us competing with Prettier/djLint, and they can run both: format Jinja with us, HTML with djLint. Because we only rewrite spans we positively recognize as Jinja, the blast radius of a formatting bug is contained to the Jinja layer. The cost is that we can't fix host-language indentation that *interacts* with Jinja (a `{% for %}` body whose HTML indentation looks wrong is left for djLint), and round-trip safety demands extensive before/after golden tests ([F18](../features/F18-formatting.md)) — a no-op-on-formatted-input invariant that every change must preserve. We accept that cost; the alternative (touching host bytes) trades it for the far worse risk of corrupting files.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Full template formatting (Jinja + HTML) | Competes with Prettier/djLint and risks corrupting markup we don't fully parse — violates P5 and endangers P3. |
| No formatting at all | Leaves a real gap — no tool owns Jinja-layer spacing well; we're best placed to. |
| Best-effort formatting without a round-trip guarantee | A formatter that can subtly change semantics is worse than none; P3 forbids it. |

## Changelog

- **2026-06-24** — Created.
