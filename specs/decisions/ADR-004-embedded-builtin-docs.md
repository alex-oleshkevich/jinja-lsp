# ADR-004 — Embed built-in docs at compile time

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The server ships documentation for the built-in Jinja filters, functions, tests, and variables, plus the extension packs — 113 hand-written markdown files ([F02](../features/F02-builtin-registry.md), [F03](../features/F03-extension-packs.md)), each with a YAML frontmatter header and a prose body. These docs back hover, completion-resolve, and signature help. The question was how the binary gets them at runtime. Reading them from disk at startup would mean shipping a docs directory alongside the binary, an install layout to get right, file I/O on the hot startup path, and a failure mode when the files are missing — all of which fight the single-binary, fast-startup posture ([ADR-001](../decisions/ADR-001-language-and-runtime.md), P6).

## Decision

We embed every built-in and pack doc into the binary at compile time with `include_str!()`. The registry parses the embedded strings at startup; nothing is read from disk for the built-ins. (User-supplied `custom_builtins` and `hints` are the deliberate exception — they load from disk because they're project-local — see [F02](../features/F02-builtin-registry.md) / [F04](../features/F04-user-hints.md).)

## Consequences

The binary is genuinely self-contained — one file, no sidecar docs directory, no install layout to get wrong. Startup does zero file I/O for built-ins and the docs can never be missing or out of sync with the binary. A doc fix is a recompile, which fits the release cadence ([F21](../features/F21-release-ci.md)). The costs are a slightly larger binary (113 markdown files is small — kilobytes, not megabytes) and that updating a built-in doc requires a rebuild rather than a file edit. That trade is the right one: built-in docs change rarely and version with the binary, whereas the things users *do* edit frequently (their own hints) correctly stay on disk.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Read docs from a directory shipped beside the binary | Breaks the single-file install; adds startup I/O and a missing-files failure mode. |
| Fetch docs over the network on demand | Violates the no-network posture ([ADR-009](../decisions/ADR-009-stdio-only-transport.md)) and adds latency to hover. |
| Generate docs from the grammar at runtime | The prose bodies are hand-written; there's nothing to generate, and parsing them at startup is the same cost without the embedding benefit. |

## Changelog

- **2026-06-24** — Created.
