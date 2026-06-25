# ADR-003 — Numeric diagnostic code scheme

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

A diagnostic could be identified by a bare kebab-case string (`undefined-variable`, `wrong-call-args`). That reads well in a report but is a poor *filter key*: there's no way to say "ignore all the undefined-* checks" or "select every warning" without listing each slug. The numeric `XXX-<SEV><CLASS><NN>` scheme used by the sibling sqlalchemy-lsp groups related checks under a class digit and makes class-level filtering trivial. jinja-lsp defines **21** diagnostic codes, and `lint.select`, `lint.ignore`, and the `noqa` directive all need to share one input grammar.

## Decision

We adopt the numeric scheme `JINJA-<SEV><CLASS><NN>` (`SEV` ∈ E/W/I/H, `CLASS` the hundreds digit). Input — `lint.select`, `lint.ignore`, and `noqa` — accepts a full code (`JINJA-E101`) or a class prefix (`JINJA-E1` = all 1xx, `JINJA-W` = all warnings) only. Each diagnostic also has a kebab-case slug retained as an **output label**, shown next to the code (`JINJA-E101 undefined-variable`), but never accepted as input.

## Consequences

Class-level filtering is now a single token: `--ignore JINJA-W2` drops every unused-* warning. Output stays readable because the human-friendly slug rides alongside the code everywhere a finding is shown ([F19](../features/F19-cli-linter.md)'s three formats, the editor hover, the json `slug` field). There is exactly one input grammar across config keys, CLI flags, and `noqa` — no redundant dual aliasing where a slug and a code mean the same thing. The cost is that users must learn the numeric codes to filter (the slug alone won't work as input); we mitigate this by always printing both, and by raising `JINJA-W107 invalid-noqa` when someone passes a slug or a non-existent code to a `noqa` directive ([F01](../features/F01-diagnostics.md)). A slug given to `--select`/`--ignore` is rejected ([F19](../features/F19-cli-linter.md)).

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Keep bare kebab slugs as the only identifier | No ergonomic class-level filtering; can't express "all warnings" without enumerating slugs. |
| Accept both slug and code as input | A redundant dual grammar — two names for one thing, more to validate and document, more ways to typo. |
| Numeric code only, drop the slug entirely | Loses readability; a bare `JINJA-E101` in a report tells the reader nothing without a lookup. |

## Changelog

- **2026-06-24** — Created.
- **2026-06-24** — Named both new codes (W106, W107) in Context; previously only W107 appeared.
