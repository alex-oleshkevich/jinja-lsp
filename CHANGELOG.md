# Changelog

All notable changes to jinja-lsp are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) — SemVer per REQ-REL-07.

## [Unreleased]

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
