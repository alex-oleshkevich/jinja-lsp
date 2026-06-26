# E17 — Testing

> **Status:** Approved
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** How jinja-lsp is tested — the coverage policy, the three test categories, and the shared fixtures registry every feature deep-links. Each feature's own plan lives in its spec and defers here.

> **Depends on:** [constitution](../constitution.md)   ·   **Related:** [E29-e2e-testing](E29-e2e-testing.md), [E03-tech-stack](E03-tech-stack.md)

> Requirement tag: **TEST**

---

## 1. Purpose & Scope

This spec defines how we test jinja-lsp and what "tested" means here. It is the authority every feature's **Testing** section (§11) defers to, and it owns the fixtures registry — the self-contained fixture workspaces, each with its own golden file, that the whole suite reuses.

This spec covers:

- The coverage policy every spec must meet.
- The three test categories (unit, integration, e2e) and when to use each.
- The fixtures registry, with a deep-link anchor per fixture.
- The `expected-diagnostics.json` golden-file shape.

Out of scope: the end-to-end harness and its two branches — that has its own foundation, [E29-e2e-testing](E29-e2e-testing.md).

## 2. Coverage policy

This is the non-negotiable bar. Features are written against it, and a spec with uncovered behavior is not done.

**REQ-TEST-01 — Every REQ maps to a test.**

Every spec ships a test plan (its §11) covering all of its behavior: each `REQ-<TAG>-NN` maps to at least one test, and every edge case and UI state (its §10 and §6) has a test. The per-spec §11.4 requirement-coverage table is the proof, not a line-coverage percentage.

**REQ-TEST-02 — Every diagnostic code has a triggering fixture.**

All **21 diagnostic codes** ([constitution §4.2](../constitution.md)) have at least one fixture that triggers them, with that finding recorded in the fixture's golden file. A code with no triggering fixture is an untested code, which fails the bar. The fixtures registry (§5) maps each code class to its fixture.

**REQ-TEST-03 — Coverage is traceable, not just numeric.**

Coverage is demonstrated by requirement-to-test mapping, so a reader can trace a rule to its proof and back. A green percentage with an untested requirement still fails.

## 3. Test categories

There are three categories, split by what they touch. Pick the lightest one that exercises the behavior.

| Category | Use it for | Speed / scope | Tooling |
|---|---|---|---|
| **Unit** | Pure functions — extraction of one construct, one check over an index, config parsing. No I/O. | Fast, isolated. | `cargo nextest`, `insta` for snapshots |
| **Integration** | The workspace index over a fixture workspace — discovery, relink, cross-file diagnostics. | Slower, wired to a fixture dir. | `cargo nextest` |
| **End-to-end** | The real binary — golden-file `check` (Branch A) and `pytest-lsp` protocol journeys (Branch B). | Slowest; the real stack. | See [E29-e2e-testing](E29-e2e-testing.md) |

The toolchain is pinned in [E03-tech-stack](E03-tech-stack.md): `cargo nextest` runs the Rust suites, `insta` handles snapshots, and the Python e2e deps (`pytest`, `pytest-lsp`, `lsprotocol`) are confined to `tests/e2e/`.

## 4. The golden-file shape

Every diagnostic fixture carries a golden file recording exactly what `check` should report. That one shape ties the fixtures, the CLI, and the e2e harness together.

**REQ-TEST-04 — `expected-diagnostics.json` is the canonical diagnostic shape.**

Each diagnostic fixture carries an `expected-diagnostics.json` — an array of objects, one per expected finding, with this exact shape:

```json
{
  "file": "blog/post.html",
  "line": 4,
  "col": 6,
  "code": "JINJA-E101",
  "slug": "undefined-variable",
  "severity": "error",
  "message": "'post' is not defined"
}
```

This shape is **identical to** `jinja-lsp check --format json` output ([F19](../features/F19-cli-linter.md)) and to what the server publishes. That identity is what makes the golden files a machine-checkable regression gate ([E29](E29-e2e-testing.md) Branch A): the CLI, the server, and the golden file all speak the same shape.

## 5. Fixtures registry

This is the canonical home for the suite's test workspaces. Each fixture is a self-contained directory under `tests/fixtures/` with its own `jinja.toml` and (except `large-workspace`) its own `expected-diagnostics.json`. Each heading below is a stable anchor, so a feature spec can deep-link it — e.g. `[the cast workspace](../foundations/E17-testing.md#starlette-blog)`.

The shape on disk:

```
tests/fixtures/
  starlette-blog/
    jinja.toml                 # templates = ["templates"], extras = ["starlette"]
    templates/
      base.html
      blog/post.html
      blog/macros.html
      email/digest.html
    expected-diagnostics.json
  large-workspace/             # 500 templates — perf baseline, NOT golden-diffed
  syntax-errors/
  undefined-vars/
  unused-symbols/
  duplicates/
  inheritance/
  call-and-paths/
  user-hints/
  config-reload/
```

### starlette-blog

The recurring example cast ([constitution §5](../constitution.md)) as a working fixture. Its `jinja.toml` sets `templates = ["templates"]` and `extras = ["starlette"]`, so `request` resolves. Holds `base.html` (the `head`/`body`/`content`/`footer` blocks), `blog/post.html` (extends base, overrides `content`, renders `{{ post.title | truncate(60) }}`, loops `{% for c in post.comments %}` calling `comment_card(c, show_actions=true)`, and calls `post_url(post)`), `blog/macros.html` (defines `post_url(post)` and `comment_card(comment, show_actions=true)`), and `email/digest.html` (extends base, overrides `content`, imports `{% from "blog/macros.html" import post_url %}`, uses `request`). These constructs — a filter call, a keyword-argument macro call (`comment_card`), a comment loop, and a cross-file macro import — give the interactive features ([F05](../features/F05-completions.md)/[F06](../features/F06-hover.md)/[F07](../features/F07-signature-help.md)) realizable call sites. It is the clean baseline — its golden file is (near) empty — and the workspace every spec's examples draw from. Reused across the whole suite. Negative/edge probes that need throwaway constructs (`{% raw %}` bodies, `{# comment #}` text, list-form `{% include ["a","b"] %}`, `is divisibleby(...)`, an `{% import "x" as y %}` alias slot) use synthetic in-memory `didOpen` documents rather than living in this baseline.

### large-workspace

500 generated templates, the performance baseline for the < 2 s rebuild budget ([E30](E30-extraction-and-indexing.md), P6). This is the **only** fixture without an `expected-diagnostics.json` — it is timed, not golden-diffed. Reused by [E30-extraction-and-indexing](E30-extraction-and-indexing.md) and the multi-folder isolation test.

### syntax-errors

Intentionally broken templates — unclosed tags, dangling delimiters — that trigger **`JINJA-E001 syntax-error`** and exercise partial-parse recovery ([E16](E16-conventions.md)). Reused by [E16-conventions](E16-conventions.md).

### undefined-vars

Templates referencing names and filters/functions/tests that aren't in scope, triggering the **1xx class** — `JINJA-E101 undefined-variable`, `E102 undefined-filter`, `E103 undefined-function`, `E104 undefined-test`. Reused by [F01-diagnostics](../features/F01-diagnostics.md).

### unused-symbols

Templates that define variables, macros, and imports never used, triggering the **2xx class** — `JINJA-W201 unused-variable`, `W202 unused-macro`, `W203 unused-import`. Reused by [F01-diagnostics](../features/F01-diagnostics.md) and the F17 remove-quick-fixes.

### duplicates

Templates with repeated blocks, macros, import aliases, from-imports, and shadowed names, triggering the **3xx class** — `JINJA-W301 duplicate-block`, `W302 duplicate-macro`, `W303 duplicate-import-alias`, `W304 duplicate-from-import`, `W305 name-shadowing`. Reused by [F01-diagnostics](../features/F01-diagnostics.md).

### inheritance

A template chain exercising the **4xx class** — `JINJA-E401 invalid-super`, `W402 unreachable-content`, `E403 missing-required-block`, `E404 recursive-import` — plus the template-chain and block go-to-definition behavior. Reused by [E07-data-model](E07-data-model.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), and [F01-diagnostics](../features/F01-diagnostics.md).

### call-and-paths

Macro calls with wrong arguments and template paths that don't exist, triggering **`JINJA-E501 wrong-call-args`** and **`E601 template-does-not-exist`** — including the `{% include "x" ignore missing %}` and dynamic-path cases that must **not** fire ([E07](E07-data-model.md) flags). Also holds the inline/embedded-template cases ([E31](E31-inline-templates.md)). Reused by [E07-data-model](E07-data-model.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), [E31-inline-templates](E31-inline-templates.md), and [F01-diagnostics](../features/F01-diagnostics.md).

### user-hints

A template with a hint sidecar (`*.hints.md`) and a configured `hints` directory, declaring `post` as a `context_variable` of `type: Post` with an `attributes` list (`title`, `slug`, `body`, `author`) — the same `post` the [F05](../features/F05-completions.md)/[F06](../features/F06-hover.md) attribute and hover examples draw on. Triggers **`JINJA-W106 unknown-attribute`** (the hint-gated, off-by-default code) on an undeclared attribute, and suppresses `JINJA-E101` for the hinted variable. Reused by [F04-user-hints](../features/F04-user-hints.md).

### config-reload

Two `jinja.toml` variants for live-reload testing — one before, one after a change to `extras`/`templates`/`lint.*` — plus an invalid variant for the retain-previous-config path. Reused by [E15-app-config](E15-app-config.md).

> **Note:** `JINJA-W107 invalid-noqa` is triggered by a `noqa` directive referencing a nonexistent code; the `undefined-vars` fixture carries a template with a bad `noqa` for this. See [F01-diagnostics](../features/F01-diagnostics.md).

## 6. Conventions

The rules that keep tests consistent and linkable across the suite.

**REQ-TEST-05 — Requirement traceability.**

Every load-bearing requirement (`REQ-<TAG>-NN`) is named in the test that verifies it, so a rule traces to its proof. The per-feature §11.4 tables are the index of this mapping.

- **Naming:** tests are named for the behavior and the requirement they verify, e.g. `req_diag_01_undefined_variable`.
- **Structure:** arrange / act / assert; one behavior per test.
- **Fakes over mocks:** prefer real fixture workspaces over mocks; the pipeline is pure enough that fakes are rarely needed.
- **Where feature tests link:** every feature's §11 links to §2 (coverage policy) and §5 (fixtures registry) rather than restating them.

## 7. Running tests & CI

`cargo nextest run` runs the unit and integration suites and the Branch A golden-file tests; `insta` snapshots update with `cargo insta review`. The Python e2e suite (Branch B) runs under `pytest` in `tests/e2e/`. CI ([F21](../features/F21-release-ci.md)) gates merge on all three plus clippy and rustfmt. The coverage bar (§2) is enforced by review of the §11.4 tables, not by a line-coverage threshold alone.

## 8. Cross-References

- **Depends on:** [constitution](../constitution.md) — the quality principles (P3, P4) and the 21-code scheme this enforces.
- **Related:** [E29-e2e-testing](E29-e2e-testing.md) — the two e2e branches that consume the golden files; [E03-tech-stack](E03-tech-stack.md) — pinned test tooling; [F01-diagnostics](../features/F01-diagnostics.md) — the codes each fixture triggers; [F19-cli-linter](../features/F19-cli-linter.md) — the `--format json` shape the golden file mirrors.

## 9. Changelog
- **2026-06-26** — Status: Draft → Approved.

- **2026-06-24** — Initial draft: the coverage policy, three test categories, the `expected-diagnostics.json` golden shape, and the fixtures registry with a deep-link anchor per fixture.
- **2026-06-24** — `starlette-blog` fixture description reconciled with the constitution cast: `base.html` owns `content`; `post.html` calls `post_url` and `comment_card`; `digest.html` extends base and imports `post_url`.
