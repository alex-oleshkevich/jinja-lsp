# Changelog

All notable changes to jinja-lsp are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) — SemVer per REQ-REL-07.

## [Unreleased]

<!-- Add entries above this line when cutting a release -->

## [0.2.0] - 2026-08-19

### Added
- Zed extension now covers 24 Jinja2 language variants — a base `Jinja2` plus
  `jinja2-html` and 19 host-format combinations (YAML, JSON, Markdown, …), with
  the LSP still attaching to bare `.jinja`/`.j2` files.
- Server lifecycle observability: a version and build banner on startup, a
  fallback log filter, and `--no-color` log output.

### Fixed
- Workspace-wide macro lookup is now deterministic. `call_hierarchy` and the
  E103 "add import" quick-fix walked a `HashMap`, so with a macro defined in
  several templates they picked an arbitrary definer that changed between runs.
- Formatter: HTML lines are reindented using the combined HTML + Jinja tag
  depth, instead of the Jinja depth alone.
- Parsing: function calls chained off an attribute (`obj.method()`) are captured,
  not just bare calls.
- Parsing: the inline gettext shorthand `_('msg')` is captured, so it gets hover.
- Hover: shows the workspace-relative template path rather than the absolute
  workspace key.
- Code lens clicks did nothing — `Command.command` was always empty.
- Zed: added the `language_ids` mapping, without which the server rejected
  `didOpen`; added the missing syntax highlighting for `Jinja2 (HTML)`; and
  pointed the grammar at the `tree-sitter-jinja` monorepo subdirectory.
- Config reloads now report success through `window/logMessage`.

### Removed
- VS Code extension (`editors/vscode/`) and its VS Code Marketplace release step.
- crates.io publish step — the `jinja-lsp` name there belongs to an unrelated
  project, and `tree-sitter-jinja`/`tree-sitter-jinja-inline` are git
  dependencies (crates.io requires a version requirement), so it never
  actually published anything.

### Changed
- AUR publishing now uses plain `git`/`ssh` against `aur.archlinux.org`
  directly instead of a third-party GitHub Action.
- AUR package renamed `jinja-lsp-bin` → `jinja-lsp-plus-bin` — the
  `jinja-lsp-bin` name belongs to an unrelated project, same collision as
  crates.io.
- A `Justfile` is now the entry point for every routine task (`just --list`),
  so local commands and CI gates cannot drift apart. `just release X.Y.Z`
  checks the tag/version and CHANGELOG gates before tagging.

## [0.1.0] - 2026-07-03

First release. A Jinja2 language server (`jinja-lsp lsp`) and standalone linter/formatter
CLI (`jinja-lsp check` / `jinja-lsp format`), covering:

- **Diagnostics** — 21 checks: undefined variables/filters/functions/tests, unused
  macros/imports, duplicate & shadowed bindings, template-inheritance errors, wrong
  call arguments, missing templates; inline `noqa` suppression.
- **Navigation** — go-to-definition (macros, blocks, templates, imports), find
  references, document symbols and workspace symbol search, call hierarchy.
- **Editing support** — hover docs, completions, signature help, semantic tokens,
  folding ranges, inlay hints, code lenses.
- **Code actions** — quick-fixes from the diagnostic catalog, extract-to-macro,
  wrap-in-block/if/for, and rename.
- **Formatting** — a Jinja-aware formatter for the template layer, available via
  `textDocument/formatting`/`rangeFormatting` and `jinja-lsp format`.
- **Configuration** — zero-config template discovery, `jinja.toml`/`pyproject.toml`
  file config, editor `InitializationOptions` overlay, framework extras
  (Flask/Starlette), custom builtin/hint docs.
- **Editor integrations** — VS Code extension, Zed extension, and documented
  Neovim/Helix recipes.

### Added
- F20: VS Code extension with language client, activation events, settings schema, and tmLanguage grammars (REQ-EDIT-03/04/05/06)
- F20: Zed extension crate with grammar registration and release-binary download+checksum (REQ-EDIT-07/08/12)
- F20: `InitializationOptions` wiring — overlay from editor settings on top of discovered config (REQ-EDIT-10)
- F20: `did_change_configuration` handler re-applies overlay on settings changes (REQ-EDIT-02)
- F20: documented `nvim-lspconfig` recipe with `init_options` in README (REQ-EDIT-09)
- F20: canonical `languageId` filter in `did_open` — only `jinja` and `jinja-html` are indexed (REQ-EDIT-11)
- F19: `jinja-lsp format` CLI with `--check` and `--diff` modes (REQ-FMT-08/09)
- F18: `textDocument/formatting` and `textDocument/rangeFormatting` LSP handlers (REQ-FMT-07)
- F18: Block-body re-indentation pass in the Jinja formatter (REQ-FMT-02)
- F17: `code_action` LSP handler with WorkspaceEdit dispatch (REQ-ACT-09)
- F17: Extract selection to macro (REQ-ACT-07)
- F17: Wrap selection in block/if/for (REQ-ACT-08)
- F17: Rename symbol workspace-wide or scope-local (REQ-ACT-11)
- F16: Call hierarchy — `callHierarchy/prepare`, `incomingCalls`, `outgoingCalls`
- F15: Code lens with reference-count and inheritance lenses
- F14: Inlay hints — macro/filter parameter labels and `endblock` echoes
- F13: Semantic tokens with full legend and classification
- F12: Folding ranges

<!-- Add entries above this line when cutting a release -->
