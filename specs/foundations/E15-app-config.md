# E15 — App Configuration

> **Status:** Draft
>
> **Version:** 0.2   ·   **Last updated:** 2026-06-26
>
> **Purpose:** How jinja-lsp finds and reads its configuration — TOML-only file discovery, configuration delivered over the LSP protocol, the zero-config fallback, every config key, the validation rules, and live reload without restarting the server.

> **Depends on:** [constitution](../constitution.md), [E01-architecture](E01-architecture.md)   ·   **Related:** [E16-conventions](E16-conventions.md), [E30-extraction-and-indexing](E30-extraction-and-indexing.md), [F03-extension-packs](../features/F03-extension-packs.md), [F04-user-hints](../features/F04-user-hints.md), [F20-editor-integrations](../features/F20-editor-integrations.md)

> Requirement tag: **CFG**

---

## 1. Purpose & Scope

This spec defines jinja-lsp's configuration: where the config file lives, what it can say, what happens when there's no config at all, and how a change to it is applied without restarting the LSP. It is the single source of truth for `templates` and `extensions` resolution — [E30](E30-extraction-and-indexing.md) consumes the answer rather than recomputing it.

This spec covers:

- Config file discovery (TOML only).
- Configuration delivered over the LSP protocol (`initializationOptions` / `didChangeConfiguration`) and how it ranks against the file.
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
- **`initializationOptions`** — the JSON config object an editor sends in the LSP `initialize` request (and updates via `workspace/didChangeConfiguration`) to configure the server when there is no config file. Its fields mirror the file key set (§5.7).

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

Only directories that actually exist on disk are added; missing ones are silently skipped. A config file, when present, overrides this fallback — the fallback applies *only* when no config file is found. Editor-supplied configuration (`initializationOptions`, §5.7) is then layered on top of whichever base applies — the file, or this fallback — overriding the keys it sets (REQ-CFG-11).

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

The same diffed re-application runs when the editor pushes new settings via `workspace/didChangeConfiguration` (REQ-CFG-11): the pushed `initializationOptions` are re-applied as an overlay on top of the current file/defaults base, overriding the keys they set.

### 5.7 Configuration over the LSP protocol

A config file is not the only configuration channel, and for many editors it is not the primary one. An editor with no `jinja.toml` — or a user who prefers editor/GUI settings — configures the server **over the LSP protocol** instead of a file. jinja-lsp accepts the same key set across both channels and merges them under one precedence.

**REQ-CFG-11 — `initializationOptions` and `didChangeConfiguration` deliver config over the protocol; the file wins.**

- The client may send an **`initializationOptions`** object in the `initialize` request. Its fields mirror the §5.4 key set one-to-one — same names, same types, **no protocol-only keys**: one schema, two delivery channels. VS Code settings, Zed `initialization_options`, and Neovim `init_options` all serialize into this single object ([F20](../features/F20-editor-integrations.md) REQ-EDIT-10).
- **Precedence is fixed: zero-config defaults ‹ config file ‹ `initializationOptions`.** The server takes the discovered config file (REQ-CFG-01), or the zero-config defaults (REQ-CFG-02) when there is none, as the **base**, then applies `initializationOptions` as an **overlay on top**, overriding every key they set. A key *absent* from `initializationOptions` keeps the file's (or default's) value — explicit editor settings win per-key, but never erase keys they don't mention. An absent `templates` (in both file and options) still defaults to `["..."]` (REQ-CFG-03), so auto-discovery (REQ-CFG-02) runs and the `"..."` sentinel still merges discovered dirs into whichever `templates` list is in effect.
- The client may push updated settings at any time via **`workspace/didChangeConfiguration`**; the server re-applies the overlay on top of the current file/defaults base and re-resolves with the same diffed invalidation as REQ-CFG-10. The pushed settings override the file per-key, exactly as `initializationOptions` do at startup.
- `initializationOptions` / `didChangeConfiguration` payloads are **untrusted** and may be absent or partial; the server deserializes tolerantly and validates per REQ-CFG-07, never panicking on bad input (P3, [E16](E16-conventions.md)).

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

The same config delivered over the protocol — the `initializationOptions` object an editor sends in `initialize` when there is no file (REQ-CFG-11). Field names and types mirror the TOML keys exactly:

```json
{
  "templates": ["templates"],
  "extras": ["starlette"],
  "hints": ["jinja-hints"],
  "lint": { "select": ["JINJA-E", "JINJA-W2"], "ignore": ["JINJA-W203"] }
}
```

## 9. Examples & Use Cases

A developer opens `starlette-blog` with **no config file**. jinja-lsp finds `templates/` on disk, adds it (REQ-CFG-02), and resolves `base.html`, `blog/post.html`, and the rest. Later they add a `jinja.toml` with `extras = ["starlette"]`; the watcher fires, the registry reloads `request`/`url_for`, and the previously-flagged `request` in `email/digest.html` stops being an undefined variable — all without restarting the editor.

## 10. Edge Cases & Failure Modes

- **Both `jinja.toml` and a `[tool.jinja]` table exist** → `jinja.toml` wins (REQ-CFG-01 order); the `pyproject.toml` table is ignored.
- **`templates` omitted entirely** → defaults to `["..."]`, i.e. pure auto-discovery.
- **Config edited to invalid TOML during a session** → workspace diagnostic raised, previous config retained (REQ-CFG-10).
- **An `extras` name is misspelled** → config error; the misspelled pack is not loaded, and the rest of the config still applies.
- **A `templates` entry escapes the workspace with `../`** → rejected during path resolution ([E30](E30-extraction-and-indexing.md)).
- **No config file, but the editor sent `initializationOptions`** → the protocol settings configure the server; an absent/`"..."` `templates` still triggers auto-discovery (REQ-CFG-11, §5.7).
- **Editor settings and a `jinja.toml` are both present** → the file is the base and the editor settings (`initializationOptions` / a `didChangeConfiguration` push) override the keys they set; keys they don't mention keep the file's values (REQ-CFG-11).
- **Malformed/partial `initializationOptions`** → deserialized tolerantly; unknown values validated per REQ-CFG-07, never a panic (REQ-CFG-11, P3).

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
| `initializationOptions` (no file present) configures the server; fields map to the key set | integration | [config-reload](E17-testing.md#config-reload) | REQ-CFG-11 |
| `initializationOptions` override a discovered `jinja.toml` per-key; keys they omit keep the file value | integration | [starlette-blog](E17-testing.md#starlette-blog) | REQ-CFG-11 |
| `didChangeConfiguration` push re-applies the overlay and re-resolves, overriding the file per-key | e2e (pytest-lsp) | [config-reload](E17-testing.md#config-reload) | REQ-CFG-11 |
| Malformed/partial `initializationOptions` deserializes tolerantly, no panic | unit | — | REQ-CFG-11 |

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
| REQ-CFG-11 | initializationOptions-configures + initializationOptions-override-file-per-key + didChangeConfiguration-reapply + tolerant-deserialize tests |

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
| E2E-04 | No file; client sends `initializationOptions = {extras=["starlette"]}` in `initialize` | happy | server loads the Starlette pack; `request` resolves with no config file |
| E2E-05 | A `jinja.toml` exists; client also sends `initializationOptions` overriding `extras` | precedence | the file is the base; the sent `extras` wins, keys not in the options keep the file's values (REQ-CFG-11) |
| E2E-06 | No file; client pushes `workspace/didChangeConfiguration` mid-session | happy | config re-resolves and re-indexes without a restart |

## 13. Non-Functional Requirements

### 13.1 Security & Privacy

- **Access & authorization** — config is read from the workspace tree only; `custom_builtins`/`hints` directories are read only as configured. No network access (P1).
- **Input & validation** — the config file is untrusted input; unknown keys/values are validated (REQ-CFG-07) and never executed.
- **Data sensitivity** — none; config holds paths and code filters, not secrets.

### 13.4 Performance & Scale

- **Latency** — a config reload completes in < 500 ms for a typical project (REQ-CFG-10); only changed sections are re-applied.

## 16. Cross-References

- **Depends on:** [constitution](../constitution.md) — "degrade, don't fail"; [E01-architecture](E01-architecture.md) — watched-files dispatch.
- **Related:** [E16-conventions](E16-conventions.md) — `ConfigError` handling; [E30-extraction-and-indexing](E30-extraction-and-indexing.md) — consumes `templates`/`extensions`; [F01-diagnostics](../features/F01-diagnostics.md) — the `lint.*` codes; [F02-builtin-registry](../features/F02-builtin-registry.md) — `custom_builtins`; [F03-extension-packs](../features/F03-extension-packs.md) — `extras`; [F04-user-hints](../features/F04-user-hints.md) — `hints`; [F20-editor-integrations](../features/F20-editor-integrations.md) — forwards editor settings as the `initializationOptions` this spec defines (REQ-CFG-11 ↔ REQ-EDIT-10).

## 17. Changelog

- **2026-06-24** — Initial draft: TOML-only discovery, zero-config fallback (incl. `<project-name>/templates`), the `"..."` sentinel, the full key set, validation rules, and live reload (REQ-CFG-10).
- **2026-06-24** — Added the `inline_patterns` key (default `["render_template_string"]`) so E31's configurable host patterns have a defined home, including its live-reload behavior.
- **2026-06-26** — **Added configuration over the LSP protocol (v0.2).** New §5.7 / REQ-CFG-11 defines `initializationOptions` (in `initialize`) and `workspace/didChangeConfiguration` as config delivery channels whose schema mirrors the §5.4 key set one-to-one. Precedence is **defaults ‹ config file ‹ `initializationOptions`**: the file (or zero-config defaults) is the base and protocol-supplied settings are layered on top, overriding per-key while leaving unmentioned keys intact — matching the legacy server's behavior of letting explicit editor settings override the file. Reconciled with the `"..."` sentinel and auto-discovery. Extended REQ-CFG-02 with a precedence pointer and REQ-CFG-10 with a `didChangeConfiguration` overlay-reapply arm. Added the §4 `initializationOptions` term, the §8 JSON shape, three §10 edges, four §11.2 test rows + REQ-CFG-11 coverage, and §12.2 E2E-04/05/06. Added F20 to Related (REQ-CFG-11 ↔ F20 REQ-EDIT-10). Closes the gap that the protocol config channel F20 depends on was unspecified here.
