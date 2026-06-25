# ADR-006 — Dual hint-file discovery with zero-config fallback

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

Static analysis can't see the symbols a Python app injects into a template's context at render time — a `post` variable, a custom filter, a project macro. Without help, the server would flag every one of them as `JINJA-E101 undefined-variable`, which violates P4 (only flag what's positively wrong). The fix is user hint files that document those symbols ([F04](../features/F04-user-hints.md)). But hints are only useful if they're easy to place: a developer documenting one template's context wants the hint *right there*, while a team documenting project-wide globals wants one shared location. Forcing a single mechanism would make one of those cases awkward. The server also has to work the instant it's installed, before anyone has written a config file at all.

## Decision

We support two hint-discovery mechanisms simultaneously: **sidecar files** (`post.html.hints.md` beside `post.html`, auto-discovered when the template is indexed) and **configured hint directories** (the `hints` config key, discovered at startup and on reload). On top of that, template discovery has a zero-config fallback — `templates/`, `<project-name>/templates/` (read from `pyproject.toml`'s `[project].name`), and the conventional `jinja/` / `j2/` directories — and the `templates` list honors a `"..."` sentinel that expands to those discovered directories.

## Consequences

The server works out of the box: a project with a conventional `templates/` layout and no config file is analyzed correctly, and a sidecar hint dropped next to a template is picked up with zero configuration. Teams that want shared, global hints use the `hints` directory; developers who want local, per-template hints use sidecars — both are active at once, so neither case is second-class. The `"..."` sentinel lets a project add a custom template dir *without* losing the conventional ones (`templates = ["custom", "..."]`), so explicit config augments the defaults instead of silently replacing them. The cost is more discovery paths to document and reason about, and a precedence model to keep straight (config overrides fallback; only existing directories are added; explicit missing dirs warn while auto-discovered missing dirs are silently skipped — [E15](../foundations/E15-app-config.md)). That complexity buys the "work-by-default, override-explicitly" experience the tool needs.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Configured hint directories only | A developer can't just drop a hint next to one template; every hint needs a config entry. |
| Sidecar files only | No place for project-wide globals shared across many templates. |
| No zero-config fallback (require a config file) | The tool wouldn't work on a fresh install; fails the out-of-the-box experience. |
| Replace defaults when `templates` is set (no `"..."`) | Adding one custom dir would silently drop the conventional `templates/`, surprising users. |

## Changelog

- **2026-06-24** — Created.
