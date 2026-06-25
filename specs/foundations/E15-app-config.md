# E15 — App Configuration

> **Status:** Draft
>
> **Version:** 0.1   ·   **Last updated:** 2026-06-24
>
> **Purpose:** How jinja-lsp finds and reads its configuration — TOML-only discovery, the zero-config fallback, every config key, the validation rules, and live reload without restarting the server.

> **Depends on:** [constitution](../constitution.md), [E01-architecture](E01-architecture.md)   ·   **Related:** [E16-conventions](E16-conventions.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), [F03-extension-packs](../features/F03-extension-packs.md), [F04-user-hints](../features/F04-user-hints.md)

> Requirement tag: **CFG**

---

## 1. Purpose & Scope

This spec defines jinja-lsp's configuration: where the config file lives, what it can say, what happens when there's no config at all, and how a change to it is applied without restarting the LSP. It is the single source of truth for `templates` and `extensions` resolution — [E30](E30-extraction-and-indexing.md) consumes the answer rather than recomputing it.

This spec covers:

- Config file discovery (TOML only).
- The zero-config fallback and template auto-discovery.
- The `"..."` sentinel in the `templates` list.
- Every config key and its default.
- Validation rules.
- Live config reload (`REQ-CFG-10`).

## 2. Non-Goals / Out of Scope

- The template scan and path resolution that consume `templates`/`extensions` — owned by [E30-extraction-and-indexing](E30-extraction-and-indexing.md).
- What the extension packs in `extras` contain — owned by [F03-extension-packs](../features/F03-extension-packs.md).
- What hint files in `hints` contain — owned by [F04-user-hints](../features/F04-user-hints.md).
- The diagnostic codes `lint.select`/`lint.ignore` filter — owned by [F01-diagnostics](../features/F01-diagnostics.md).

## 3. Background & Rationale

The best config is the one you never write. A new user should be able to point an editor at a Starlette project with a `templates/` directory and have everything resolve — no `jinja.toml` required. So jinja-lsp works zero-config, auto-discovering the usual template directories, and treats the config file as an *override*, not a prerequisite. When config does exist, it's TOML and only TOML (ADR-003 keeps one grammar; the same discipline applies here): either a dedicated `jinja.toml` or a `[tool.jinja]` table in `pyproject.toml`, so it sits naturally next to the Python project it serves.

## 4. Concepts & Definitions

- **Config file** — `jinja.toml` or `pyproject.toml`'s `[tool.jinja]` table. (Canonical definition in [glossary](../glossary.md).)
- **Zero-config fallback** — automatic template discovery when no config file is present. (Canonical definition in [glossary](../glossary.md).)
- **`"..."` sentinel** — the placeholder that expands to the discovered directories. (Canonical definition in [glossary](../glossary.md).)
- **Live reload** — re-applying config changes without restarting the server. (Canonical definition in [glossary](../glossary.md).)

## 5. Detailed Specification

### 5.1 Config file discovery

There are two places a config can live, checked in a fixed order from the workspace root upward.

**REQ-CFG-01 — Discover `jinja.toml`, then `pyproject.toml`.**

Starting at the workspace root and walking up to the filesystem root, jinja-lsp looks for a config in this order:

1. A **`jinja.toml`** file — if found, its top-level table is the config.
2. A **`pyproject.toml`** file with a **`[tool.jinja]`** table — if found, that table is the config.

The first match wins; discovery stops there. Config is **TOML only** — no YAML or JSON config files are recognized.

### 5.2 Zero-config fallback

When no config file is found anywhere, jinja-lsp still works by auto-discovering template directories.

**REQ-CFG-02 — Zero-config template discovery.**

With no config file present, jinja-lsp auto-discovers template directories, relative to the workspace root, in this order:

1. **`templates/`** — the most common layout.
2. **`<project-name>/templates/`** — where `<project-name>` is read from `pyproject.toml`'s `[project].name`, falling back to `[tool.poetry].name`.
3. **`jinja/`** and **`j2/`** — conventional default directories.

Only directories that actually exist on disk are added; missing ones are silently skipped. A config file, when present, always overrides this fallback — the fallback applies *only* when no config file is found.

### 5.3 The `templates` key and the `"..."` sentinel

The `templates` key lists where templates live. A special sentinel lets you add directories *on top of* the auto-discovered set instead of replacing it.

**REQ-CFG-03 — The `"..."` sentinel merges in the discovered dirs.**

The `templates` list may contain the literal string **`"..."`**, which expands in place to the directories the zero-config fallback (REQ-CFG-02) would have discovered. So:

- `templates = ["custom_dir", "..."]` means `custom_dir` **plus** the auto-discovered directories.
- `templates = ["custom_dir"]` (no sentinel) means *only* `custom_dir` — the list fully replaces the defaults.
- When the `templates` key is **absent**, the default is `["..."]` — i.e. pure auto-discovery, identical to zero-config.

### 5.4 All config keys

The full set of keys, their meaning, and their defaults. Each is optional.

**REQ-CFG-04 — The config key set.**

| Key | Type | Default | Meaning |
|---|---|---|---|
| `templates` | list of strings | `["..."]` | Template directories; supports the `"..."` sentinel (REQ-CFG-03) |
| `extensions` | list of strings | `["html", "jinja", "jinja2", "j2"]` | File extensions to scan ([E30](E30-extraction-and-indexing.md)) |
| `lint.select` | list of codes/prefixes | all enabled | Diagnostic codes to enable ([F01](../features/F01-diagnostics.md)) |
| `lint.ignore` | list of codes/prefixes | empty | Diagnostic codes to disable ([F01](../features/F01-diagnostics.md)) |
| `extras` | list of strings | empty | Extension packs: `flask`, `starlette`, `starlette-babel`, `starlette-flash` ([F03](../features/F03-extension-packs.md)) |
| `custom_builtins` | list of strings | empty | Directories of built-in-format `.md` docs ([F02](../features/F02-builtin-registry.md)) |
| `hints` | list of strings | empty | Directories of user hint files ([F04](../features/F04-user-hints.md)) |
| `inline_patterns` | list of strings | `["render_template_string"]` | Host render-function names whose string argument is parsed as an inline Jinja template ([E31](E31-inline-templates.md)) |

**REQ-CFG-05 — `lint.select`/`lint.ignore` accept code or class-prefix only.**

The lint filters take a full code (`JINJA-E101`) or a class prefix (`JINJA-E1`, `JINJA-W`) — **never a slug**. This is the same input grammar as `noqa` directives and the constitution's §4.2 scheme. A slug in a lint filter is a validation error.

**REQ-CFG-06 — `hints` is separate from `custom_builtins`.**

`custom_builtins` and `hints` are distinct keys with distinct meaning. `custom_builtins` loads third-party *built-ins* (filters/functions/tests in the core doc format). `hints` loads *project-local* docs describing the project's own macros, filters, and context variables ([F04](../features/F04-user-hints.md)). Keeping them separate lets a project vendor a builtin pack and document its own symbols without conflating the two.

### 5.5 Validation

A bad config should fail loudly where it's wrong and quietly where it's merely incomplete.

**REQ-CFG-07 — Validation rules.**

- An **unknown name in `extras`** is a **config error** — packs are a closed set ([F03](../features/F03-extension-packs.md)).
- A code appearing in **both `lint.select` and `lint.ignore`** is a **warning**; `ignore` wins for that code.
- A **non-existent explicit `templates` directory** (one named in the list, not discovered) is a **warning**. Auto-discovered directories that don't exist are silently skipped (REQ-CFG-02), never warned.
- A **slug** used where a code/prefix is required (REQ-CFG-05) is a **config error**.

Per [E16](E16-conventions.md), a fatal config error surfaces as a `ConfigError` and the server degrades gracefully — it never panics.

### 5.6 Live config reload

Editing config should not mean restarting the editor. The server watches its own config file and re-applies changes in place.

**REQ-CFG-10 — Live config reload via `didChangeWatchedFiles`.**

The server registers the discovered config file with `workspace/didChangeWatchedFiles` ([E01](E01-architecture.md#53-document-lifecycle--watched-files)). On a change, the server:

1. Re-parses the config.
2. **Diffs** it against the previous state and invalidates only what changed:
   - `extras` changed → reload the built-in registry ([F02](../features/F02-builtin-registry.md), [F03](../features/F03-extension-packs.md)).
   - `templates`/`extensions` changed → re-scan the workspace ([E30](E30-extraction-and-indexing.md)).
   - `lint.*` changed → re-run diagnostics with the new filters.
   - `custom_builtins`/`hints` changed → reload those loaders.
   - `inline_patterns` changed → re-scan host files for embedded templates ([E31](E31-inline-templates.md)).
3. Completes within **500 ms** for a typical project.

If the new config fails to parse, the server reports the failure as a **workspace diagnostic** and **retains the previous valid config** — a typo never breaks an active session.

## 8. Data Shapes

A worked `jinja.toml` for the `starlette-blog` cast. It points at `templates/`, turns on the Starlette pack so `request` resolves, adds a project hints directory, and tunes the lint set:

```toml
# jinja.toml — at the workspace root
templates = ["templates"]               # explicit; no "..." → replaces defaults
extensions = ["html", "jinja"]          # scan .html and .jinja only
extras = ["starlette"]                  # request / url_for globals (F03)
custom_builtins = ["vendor/jinja-docs"] # third-party builtin docs (F02)
hints = ["jinja-hints"]                 # project context-var docs (F04)

[lint]
select = ["JINJA-E", "JINJA-W2"]        # all errors + the unused-* class
ignore = ["JINJA-W203"]                 # but not unused-import
```

The equivalent under `pyproject.toml` is the same keys nested under `[tool.jinja]`:

```toml
# pyproject.toml
[project]
name = "starlette-blog"                 # used by the <project-name>/templates fallback

[tool.jinja]
templates = ["templates"]
extras = ["starlette"]
```

## 9. Examples & Use Cases

A developer opens `starlette-blog` with **no config file**. jinja-lsp finds `templates/` on disk, adds it (REQ-CFG-02), and resolves `base.html`, `blog/post.html`, and the rest. Later they add a `jinja.toml` with `extras = ["starlette"]`; the watcher fires, the registry reloads `request`/`url_for`, and the previously-flagged `request` in `email/digest.html` stops being an undefined variable — all without restarting the editor.

## 10. Edge Cases & Failure Modes

- **Both `jinja.toml` and a `[tool.jinja]` table exist** → `jinja.toml` wins (REQ-CFG-01 order); the `pyproject.toml` table is ignored.
- **`templates` omitted entirely** → defaults to `["..."]`, i.e. pure auto-discovery.
- **Config edited to invalid TOML during a session** → workspace diagnostic raised, previous config retained (REQ-CFG-10).
- **An `extras` name is misspelled** → config error; the misspelled pack is not loaded, and the rest of the config still applies.
- **A `templates` entry escapes the workspace with `../`** → rejected during path resolution ([E30](E30-extraction-and-indexing.md)).

## 11. Testing

This foundation is verified by config-parsing unit tests plus a live-reload e2e against the `config-reload` fixture.

### 11.1 Scope & coverage

Target: **100% of this spec's behavior is covered.** Every `REQ-CFG-NN` maps to at least one test. See the policy in [E17-testing](E17-testing.md#2-coverage-policy).

### 11.2 Test plan

| Behavior / scenario | Type | Fixtures | Verifies |
|---|---|---|---|
| `jinja.toml` found before `pyproject.toml` | unit | [config-reload](E17-testing.md#config-reload) | REQ-CFG-01 |
| Zero-config discovers `templates/` and `<name>/templates` | integration | [starlette-blog](E17-testing.md#starlette-blog) | REQ-CFG-02 |
| `"..."` sentinel merges discovered dirs | unit | [config-reload](E17-testing.md#config-reload) | REQ-CFG-03 |
| Each key parses to its default when absent | unit | — | REQ-CFG-04 |
| A slug in `lint.select` is a config error | unit | — | REQ-CFG-05 |
| Unknown `extras` name is a config error | unit | — | REQ-CFG-07 |
| Overlapping select/ignore warns; ignore wins | unit | — | REQ-CFG-07 |
| Editing config reloads only the changed section in < 500 ms | e2e (pytest-lsp) | [config-reload](E17-testing.md#config-reload) | REQ-CFG-10 |
| Invalid config on reload retains the previous config | e2e (pytest-lsp) | [config-reload](E17-testing.md#config-reload) | REQ-CFG-10 |

### 11.4 Requirement coverage

| Requirement | Covered by |
|---|---|
| REQ-CFG-01 | discovery-order test |
| REQ-CFG-02 | zero-config discovery test |
| REQ-CFG-03 | sentinel-merge test |
| REQ-CFG-04 | key-defaults test |
| REQ-CFG-05 | slug-rejection test |
| REQ-CFG-06 | hints-vs-builtins separation test |
| REQ-CFG-07 | validation-rules tests |
| REQ-CFG-10 | live-reload + retain-on-error e2e |

## 12. End-to-End Test Plan

Live reload is exercised end to end through the running server against the `config-reload` fixture.

### 12.1 Coverage target

**100% of the reload behavior** — diffed invalidation on the happy path, and the retain-previous-config error path.

### 12.2 Scenarios

| # | Journey | Path | Expected outcome |
|---|---|---|---|
| E2E-01 | Add `extras = ["starlette"]` to config | happy | registry reloads; `request` stops being undefined |
| E2E-02 | Change `templates` dirs | happy | workspace re-scans; new templates resolve |
| E2E-03 | Save invalid TOML | error | workspace diagnostic; previous config still active |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — config is read from the workspace tree only; `custom_builtins`/`hints` directories are read only as configured. No network access (P1).
- **Input & validation** — the config file is untrusted input; unknown keys/values are validated (REQ-CFG-07) and never executed.
- **Data sensitivity** — none; config holds paths and code filters, not secrets.

### 13.4 Performance & Scale

- **Latency** — a config reload completes in < 500 ms for a typical project (REQ-CFG-10); only changed sections are re-applied.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — "degrade, don't fail"; [E01-architecture](E01-architecture.md) — watched-files dispatch.
- **Related:** [E16-conventions](E16-conventions.md) — `ConfigError` handling; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — consumes `templates`/`extensions`; [F01-diagnostics](../features/F01-diagnostics.md) — the `lint.*` codes; [F02-builtin-registry](../features/F02-builtin-registry.md) — `custom_builtins`; [F03-extension-packs](../features/F03-extension-packs.md) — `extras`; [F04-user-hints](../features/F04-user-hints.md) — `hints`.

## 17. Changelog

- **2026-06-24** — Initial draft: TOML-only discovery, zero-config fallback (incl. `<project-name>/templates`), the `"..."` sentinel, the full key set, validation rules, and live reload (REQ-CFG-10).
- **2026-06-24** — Added the `inline_patterns` key (default `["render_template_string"]`) so E31's configurable host patterns have a defined home, including its live-reload behavior.
