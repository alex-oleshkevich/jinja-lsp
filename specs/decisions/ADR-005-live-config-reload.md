# ADR-005 — Live config reload via watched files

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The config file (`jinja.toml` or `pyproject.toml`'s `[tool.jinja]`) drives a lot of behavior: which template directories are scanned, which extension packs are active, which hint directories load, and which lint rules run ([E15](../foundations/E15-app-config.md)). While iterating on a project, a developer changes `extras` or `hints` often. A restart-to-reload model would force every editor to disconnect on each change, re-index the workspace from cold, and break the developer's flow. That restart cost is exactly the kind of friction P6 ("fast enough to forget") rules out for an everyday tool.

## Decision

The server subscribes to the discovered config file via LSP `workspace/didChangeWatchedFiles`. On a change it re-parses the config, diffs it against the previous state, and invalidates only the sections that changed — `extras` change reloads the registry, `templates` change re-scans the workspace, `lint` change re-runs diagnostics — with no LSP restart.

## Consequences

A developer edits `extras = ["starlette"]` and the `request` global resolves moments later, without reconnecting the editor or losing the warm index. The diff-reload keeps the cost proportional to the change: editing a lint rule doesn't trigger a full workspace re-scan. A reload completes within ~500 ms for a typical project ([E15](../foundations/E15-app-config.md) REQ-CFG-10). The trade-offs are real: the reload logic is more complex than a restart, and we must handle a config that becomes invalid mid-session. We do — a parse error during reload is surfaced as a workspace diagnostic and the previous valid config is retained, so a half-typed edit never leaves the server unconfigured (degrade, don't fail). Hint files reload through the same watched-files path ([F04](../features/F04-user-hints.md)).

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Require an LSP restart on config change | Disconnects the editor, throws away the warm index, breaks flow — fails P6's friction posture. |
| Reload everything on any config change | Simpler, but a one-line lint edit would trigger a full workspace re-scan; the diff keeps cost proportional. |
| Poll the config file on a timer | Wasteful and laggy; the editor already tells us via `didChangeWatchedFiles`. |

## Changelog

- **2026-06-24** — Created.
