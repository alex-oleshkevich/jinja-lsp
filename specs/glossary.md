# Glossary

> **Status:** Living (continuously maintained)
>
> **Last updated:** 2026-06-24
>
> **Purpose:** The canonical definition of every domain term used across the suite. Define a term once here; link to it everywhere else instead of redefining.

This is a living document — add a term the first time a spec needs it. Terms are grouped by domain.

---

## Jinja language

- **Template** — a single Jinja source file (or inline region) the LSP analyzes, identified by a workspace-relative path. For example, `templates/blog/post.html` in `starlette-blog`.
- **Block** — a `{% block name %}…{% endblock %}` region; carries optional `scoped`/`required` modifiers that drive inheritance diagnostics. Owned by [F01-diagnostics](features/F01-diagnostics.md).
- **Macro** — a reusable `{% macro name(params) %}…{% endmacro %}` definition, callable like a function and importable across templates, e.g. `post_url(post)`.
- **Filter** — a transform applied with `|`, e.g. `{{ name | upper }}`. Built-in, pack-provided, or user-hinted.
- **Test** — a boolean check used with `is`, e.g. `{% if post is defined %}`. The Jinja sense of "test," distinct from a unit test.
- **Function (global)** — a callable in template scope, e.g. `range()`, `url_for()`. From built-ins, packs, or hints.
- **Tag / statement** — a `{% … %}` construct (`for`, `if`, `set`, `include`, `extends`, `with`, `call`, `trans`, …).
- **Expression** — the sub-language inside `{{ … }}` and `{% … %}`, parsed by the inline grammar.
- **Attribute access** — `obj.attr` or `obj["attr"]` in an expression; validated against hinted `attributes` when available, e.g. `post.title`.
- **Whitespace control** — the `-` markers in `{%- … -%}` / `{{- … -}}` that trim surrounding whitespace.

## Inheritance & composition

- **`extends`** — declares a template's parent; the child overrides the parent's blocks.
- **`include`** — inlines another template's output; may carry `ignore missing`.
- **`import` / `from … import`** — pulls macros from another template, optionally aliased.
- **Template chain** — the ordered list of templates from a child through its `extends` links to the root.
- **`super()`** — inside an overriding block, renders the parent block's content; valid only where a parent block exists.
- **`ignore_missing`** — a flag on an `include` suppressing the missing-template diagnostic.
- **`is_dynamic`** — internal flag marking a template reference whose path is an expression, not a literal, so it can't be resolved statically.

## Server & analysis

- **TemplateIndex** — the per-file collection of facts extracted from one template's tree (symbols, references, syntax errors). Owned by [E07-data-model](foundations/E07-data-model.md).
- **WorkspaceIndex** — the cross-file index mapping template paths to their TemplateIndex and resolving inheritance/import graphs.
- **Pass 1 / Pass 2** — extraction (per-file, on every change) and relink (cross-file, debounced). See [E01-architecture](foundations/E01-architecture.md).
- **Symbol** — an extracted definition: macro, block, variable, import alias, or template reference.
- **Reference** — a usage site of a symbol (identifier, attribute, filter, function, or test reference).
- **Variable scope** — where a variable is visible: Template, Block, ForLoop, Macro, With, CallBlock, Trans, Filter, or Autoescape.
- **Built-in registry** — the unified in-memory store of all documentation (built-ins, packs, custom builtins, hints), keyed by `(category, name)`. See [F02-builtin-registry](features/F02-builtin-registry.md).
- **Inline template** — Jinja embedded in a host file (e.g. a Python `render_template_string("…")`), analyzed via the inline grammar. See [E31-inline-templates](foundations/E31-inline-templates.md).

## Configuration & extension

- **Config file** — `jinja.toml`, or `pyproject.toml`'s `[tool.jinja]` table. TOML only. Owned by [E15-app-config](foundations/E15-app-config.md).
- **Zero-config fallback** — automatic template discovery (`templates/`, `<project-name>/templates/`, `jinja/`, `j2/`) when no config file is present.
- **`"..."` sentinel** — a placeholder in the `templates` list that expands to the zero-config-discovered directories, so explicit dirs merge with discovered ones.
- **Extension pack (extra)** — a bundle of framework globals/filters activated via `extras`: `flask`, `starlette`, `starlette-babel`, `starlette-flash`. See [F03-extension-packs](features/F03-extension-packs.md).
- **Custom builtins** — user-supplied built-in-format `.md` docs loaded from `custom_builtins` directories.
- **Hint file** — a project-local markdown doc (sidecar `*.hints.md` or in a `hints` directory) documenting user macros, filters, and context variables. See [F04-user-hints](features/F04-user-hints.md).
- **Context variable** — a variable injected into a template's context by host code at render time; invisible to static analysis unless declared in a hint file, e.g. `post`.
- **Sidecar file** — a hint file placed next to a template (`post.html.hints.md` beside `post.html`).
- **Live reload** — re-applying config and hint changes without restarting the LSP, via `workspace/didChangeWatchedFiles`.

## Diagnostics & directives

- **Diagnostic code** — the `JINJA-<SEV><CLASS><NN>` identifier (e.g. `JINJA-E101`). See the constitution §4.2 scheme.
- **Slug** — the kebab-case label paired with a code (e.g. `undefined-variable`); an output label, not an input identifier.
- **Class prefix** — a partial code matching a whole class (`JINJA-E1` = all 1xx; `JINJA-W` = all warnings) for `select`/`ignore`/`noqa`.
- **`noqa` directive** — an inline suppression comment: `{# noqa #}`, `{# noqa: JINJA-E101 #}`, or `{# noqa-file #}`. See [F01-diagnostics](features/F01-diagnostics.md).

## Editor & protocol

- **Capability** — a feature the server advertises in its `initialize` response (e.g. `hoverProvider`).
- **Quick-fix / code action** — an editor-offered edit resolving a diagnostic or refactoring code. See [F17-code-actions](features/F17-code-actions.md).
- **Refactor** — a cursor- or selection-triggered transformation (extract-to-macro, wrap-in-block).
- **Folding range** — a collapsible region (block, loop, macro, comment). See [F12-folding-range](features/F12-folding-range.md).
- **Semantic token** — a classification attached to a span so the editor colors it by meaning (known macro vs unknown variable). See [F13-semantic-tokens](features/F13-semantic-tokens.md).
- **Inlay hint** — inline ghost text (a parameter name, an `endblock` echo). See [F14-inlay-hints](features/F14-inlay-hints.md).
- **Call hierarchy** — the incoming/outgoing call graph of a macro. See [F16-call-hierarchy](features/F16-call-hierarchy.md).

## Changelog

- **2026-06-24** — Initial glossary covering the Jinja language, inheritance, server/analysis, configuration, diagnostics, and editor/protocol domains.
