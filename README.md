# jinja-lsp

[![CI](https://github.com/alex-oleshkevich/jinja-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/alex-oleshkevich/jinja-lsp/actions/workflows/ci.yml)
[![Release](https://github.com/alex-oleshkevich/jinja-lsp/actions/workflows/release.yml/badge.svg)](https://github.com/alex-oleshkevich/jinja-lsp/releases)

Language server for Jinja templates — diagnostics, navigation, completions, hover, and Jinja-aware formatting. One Rust binary, any LSP-capable editor. Static analysis only — it never imports, renders, or executes your templates or host Python.

It runs *alongside* your Python and HTML language servers, owning the Jinja layer end to end and staying silent everywhere else.

## Features

| | |
|---|---|
| **Diagnostics** | 21 checks — undefined variables/filters/functions/tests, unused macros/imports, duplicate & shadowed bindings, inheritance errors, wrong call args, missing templates; inline `noqa` suppression |
| **Navigation** | go-to-definition (macros, blocks, templates, imports), find references, document & call hierarchy |
| **Hover** | built-in docs for filters/tests/functions, macro signatures, variable scope and definition site |
| **Completions** | variables, attributes, filters, tests, statement keywords, template paths, imported macro names |
| **Signature help** | macro and filter call signatures, with the active argument highlighted |
| **Symbols & lenses** | document symbols, semantic tokens, folding, inlay hints, reference/override code lenses |
| **Code actions** | quick-fixes from the diagnostic catalog, extract-to-macro, wrap-in-block/if/for, and **rename** |
| **Formatting** | Jinja-aware formatting of the template layer — `jinja-lsp format` |
| **`check` CLI** | the same diagnostics as a linter — `jinja-lsp check .` with `rich` / `compact` / `json` output |

## Installation

```bash
uv tool install jinja-lsp
```

Or with pip:

```bash
pip install jinja-lsp
```

Both install a self-contained Rust binary — no Rust toolchain, no Python runtime dependency. Or download a pre-built binary from the [releases page](https://github.com/alex-oleshkevich/jinja-lsp/releases), or `cargo install jinja-lsp`.

## Editor setup

The server is launched as a subprocess and speaks LSP over stdio (`jinja-lsp lsp`). There is no TCP/socket transport.

### Neovim

```lua
vim.lsp.config('jinja_lsp', {
  cmd = { 'jinja-lsp', 'lsp' },
  filetypes = { 'jinja', 'jinja2', 'html', 'htmldjango' },
  root_markers = { 'jinja.toml', 'pyproject.toml', '.git' },
})
vim.lsp.enable('jinja_lsp')
```

### Helix

```toml
# ~/.config/helix/languages.toml
[language-server.jinja-lsp]
command = "jinja-lsp"
args = ["lsp"]

[[language]]
name = "jinja"
language-servers = ["jinja-lsp"]

[[language]]
name = "html"
language-servers = ["vscode-html-language-server", "jinja-lsp"]
```

### Zed

Install from the Zed extensions panel (`Cmd+Shift+X`) — search for **jinja-lsp** and click Install. It activates automatically for Jinja and HTML templates.

To control server order alongside other language servers or pass initialization options, add to `~/.config/zed/settings.json` (the language-server id is `jinja2-lsp` and the language is `Jinja2 (HTML)`):

```jsonc
{
  "languages": { "Jinja2 (HTML)": { "language_servers": ["jinja2-lsp"] } },
  "lsp": { "jinja2-lsp": { "initialization_options": { "templates": ["templates"], "extras": ["starlette"] } } }
}
```

### VS Code

Install the **jinja-lsp** extension from the marketplace. It spawns `jinja-lsp lsp` over stdio; set `jinja-lsp.server.path` if the binary isn't on `PATH`.

## Configuration

Zero config for standard projects — template directories are discovered automatically (`templates/`, `<project-name>/templates/`, `jinja/`, `j2/`). A discovered config file (`jinja.toml`, then `[tool.jinja]` in `pyproject.toml`) — or the zero-config defaults when there's none — is the base; the editor's `InitializationOptions` are then overlaid on top, **overriding only the keys they set** while leaving the rest of the file intact.

| Option | Default | |
|---|---|---|
| `templates` | _(auto-discovered)_ | template root directories; `"..."` expands to the discovered set |
| `extensions` | `["html", "jinja", "jinja2", "j2"]` | file extensions to scan |
| `extras` | `[]` | framework packs: `flask`, `starlette`, `starlette-babel`, `starlette-flash` |
| `hints` | `[]` | directories of hint files describing your project's context variables/macros |
| `custom_builtins` | `[]` | directories of built-in-format `*.md` docs for third-party filters/functions/tests |
| `inline_patterns` | `["render_template_string"]` | host render-function names whose string argument is parsed as an inline template |
| `lint.select` | _(all)_ | diagnostic codes/classes to enable (`JINJA-E1`, `JINJA-W`, …) |
| `lint.ignore` | `[]` | diagnostic codes/classes to suppress |

```toml
# jinja.toml
templates = ["templates"]
extras = ["starlette"]

[lint]
ignore = ["JINJA-W106"]
```

## CLI

```
jinja-lsp lsp                                              # run the language server over stdio
jinja-lsp check PATH [--select CODES] [--ignore CODES] [--format rich|compact|json]
jinja-lsp format PATH [--check]
```

`check`'s `json` output matches the format the test suite asserts against, so it diffs cleanly in CI. `format` rewrites the Jinja layer only and is round-trip safe.

## Development

```bash
cargo build
cargo nextest run
uv run --group dev pytest tests/e2e/ -v
```

## License

MIT
