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
curl -fsSL https://raw.githubusercontent.com/alex-oleshkevich/jinja-lsp/master/install.sh | bash
```

This picks the right build for your machine, checks it against the published SHA-256, and puts the binary in `~/.local/bin`. It never asks for sudo and writes nothing outside that directory. If `~/.local/bin` is not on your `PATH`, the script says so and prints the line to add.

Two environment variables change what it does. Note that they go on `bash`, not on `curl`, because the two are separate processes and only `bash` runs the script:

```bash
# pin a version instead of taking the latest
curl -fsSL https://raw.githubusercontent.com/alex-oleshkevich/jinja-lsp/master/install.sh \
  | JINJA_LSP_VERSION=0.2.0 bash

# install somewhere other than ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/alex-oleshkevich/jinja-lsp/master/install.sh \
  | JINJA_LSP_INSTALL_DIR=~/bin bash
```

Intel Macs have no published build. Apple Silicon, Linux (x86_64 and ARM64), and Windows do.

If you would rather use a package manager:

```bash
uv tool install jinja-lsp
pip install jinja-lsp
yay -S jinja-lsp-plus-bin          # Arch Linux
```

The Python packages ship the same self-contained Rust binary, so neither needs a Rust toolchain, and nothing imports Python at runtime. On the AUR the package is `jinja-lsp-plus-bin`. The similarly named `jinja-lsp-bin` belongs to an unrelated project.

You can also grab an archive from the [releases page](https://github.com/alex-oleshkevich/jinja-lsp/releases) and put the binary wherever you like.

## Editor setup

The server is launched as a subprocess and speaks LSP over stdio (`jinja-lsp lsp`). There is no TCP/socket transport.

### Neovim

Using [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) — paste this into `~/.config/nvim/init.lua`:

```lua
local lspconfig = require("lspconfig")
local configs   = require("lspconfig.configs")

if not configs.jinja_lsp then
  configs.jinja_lsp = {
    default_config = {
      cmd       = { "jinja-lsp", "lsp" },
      filetypes = { "jinja", "jinja.html", "htmldjango" },
      root_dir  = lspconfig.util.root_pattern("jinja.toml", "pyproject.toml", ".git"),
      -- mirrors jinja.toml; all keys optional — overlay on top of any discovered config file
      init_options = {
        templates = { "templates", "..." },
        extras    = {},
        hints     = {},
        lint      = { select = {}, ignore = {} },
      },
    },
  }
end

lspconfig.jinja_lsp.setup({})
```

Neovim 0.11+: you can also use the built-in `vim.lsp.config` API instead:

```lua
vim.lsp.config('jinja_lsp', {
  cmd = { 'jinja-lsp', 'lsp' },
  filetypes = { 'jinja', 'jinja.html', 'htmldjango' },
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
language-servers = ["jinja-lsp"]
```

### Zed

Install from the Zed extensions panel (`Cmd+Shift+X`) — search for **Jinja Plus** and click Install (extension id `jinja-plus` — `jinja-lsp` was already taken on Zed's marketplace). It activates automatically for Jinja and HTML templates.

To control server order alongside other language servers or pass initialization options, add to `~/.config/zed/settings.json` (the language-server id is `jinja2-lsp` and the language is `Jinja2 (HTML)`):

```jsonc
{
  "languages": { "Jinja2 (HTML)": { "language_servers": ["jinja2-lsp"] } },
  "lsp": { "jinja2-lsp": { "initialization_options": { "templates": ["templates"], "extras": ["starlette"] } } }
}
```

## Configuration

Most projects need no configuration. Template directories are found automatically by looking for `templates/`, `<project-name>/templates/`, `jinja/`, and `j2/`.

When you do want to configure something, the server walks up from the project root and takes the first of these it finds:

1. `jinja.toml`
2. `[tool.jinja]` in `pyproject.toml`

Your editor's `initializationOptions` are layered on top of whichever it found. The overlay replaces only the keys it sets, so an editor setting for `extras` will not wipe out the `templates` list in your `jinja.toml`. Clearing a setting in your editor falls back to the file value rather than leaving the old override in place. Both files are watched, so edits apply without restarting the server.

### General options

| Option | Default | |
|---|---|---|
| `templates` | _(auto-discovered)_ | template root directories. `"..."` expands to the auto-discovered set, so you can add a directory without losing the defaults |
| `extensions` | `["html", "jinja", "jinja2", "j2"]` | file extensions to scan |
| `extras` | `[]` | framework packs that teach the server about globals your framework injects: `flask`, `starlette`, `starlette-babel`, `starlette-flash` |
| `hints` | `[]` | directories of hint files describing your own context variables and macros |
| `custom_builtins` | `[]` | directories of `*.md` docs for third-party filters, functions, and tests |
| `inline_patterns` | `["render_template_string"]` | host function names whose string argument is parsed as an inline template |
| `lint.select` | _(all)_ | diagnostic codes or classes to enable, such as `JINJA-E1` or `JINJA-W` |
| `lint.ignore` | `[]` | diagnostic codes or classes to suppress |

```toml
# jinja.toml
templates = ["templates", "..."]
extras = ["starlette"]

[lint]
ignore = ["JINJA-W106"]
```

To suppress a finding in one place rather than project-wide, use a comment in the template: `{# noqa #}` for that line, `{# noqa: JINJA-W201 #}` for one code, or `{# noqa-file #}` for the whole file.

### Formatter options

These live under `[format]` and apply to both `jinja-lsp format` and formatting from your editor. The formatter only rewrites what is inside Jinja delimiters, so your HTML, YAML, or whatever else the file contains is reproduced byte for byte.

| Option | Default | |
|---|---|---|
| `indent_size` | `4` | spaces per indent level, ignored when `use_tabs` is set |
| `use_tabs` | `false` | indent with tabs instead of spaces |
| `space_around_pipe` | `true` | `x \| upper` rather than `x\|upper` |
| `space_around_operators` | `false` | `a + b` rather than `a+b`, for symbolic operators only |
| `space_after_comma` | `true` | `truncate(20, true)` rather than `truncate(20,true)`, in filter-call arguments |
| `space_inside_parens` | `false` | `truncate( 20, true )` rather than `truncate(20, true)`, in filter-call arguments |
| `space_inside_variable_delimiters` | `true` | `{{ x }}` rather than `{{x}}` |
| `space_inside_block_delimiters` | `true` | `{% if x %}` rather than `{%if x%}` |
| `blank_lines_after_block` | `0` | blank lines to leave after a top-level `{% endblock %}`, `{% endfor %}`, and so on |
| `trim_blocks` | `false` | drop the first newline after a `{% %}` tag, matching Jinja2's runtime option of the same name |
| `lstrip_blocks` | `false` | drop leading whitespace before a `{% %}` tag, likewise |
| `preferred_quote` | `"preserve"` | `"single"` or `"double"` to normalize string literals, `"preserve"` to leave them alone |
| `newline_at_eof` | `true` | end the file with exactly one newline |
| `trim_trailing_whitespace` | `true` | strip trailing whitespace from every line |

Two things the formatter deliberately leaves alone. Keyword operators (`and`, `or`, `is`, `in`, `not`) always keep their surrounding spaces, whatever `space_around_operators` says, because removing them would change how the expression tokenizes: `andb` is one identifier, not `and` followed by `b`. And the comma and paren options above cover filter-call arguments only. Commas you wrote in a macro call, a dict, or a list stay exactly as you typed them, so `{{ post_url(post,absolute=true) }}` is left untouched.

```toml
# jinja.toml
[format]
indent_size = 2
space_around_pipe = false
preferred_quote = "double"
```

## CLI

```
jinja-lsp lsp                                              # run the language server over stdio
jinja-lsp check PATH [--select CODES] [--ignore CODES] [--format rich|compact|json]
jinja-lsp format PATH [--check]
jinja-lsp doctor [--config PATH]                           # report what it discovers here
```

`doctor` answers "why is it not seeing my templates". It prints the config file it
found (or that it fell back to zero-config), each template directory with how many
files matched, the builtin sources that loaded and what each contributed, and any
`*.hints.md` sidecars. It reports directories you configured but that do not exist,
which the indexer skips silently, and exits 1 when it finds a problem.

`check`'s `json` output matches the format the test suite asserts against, so it diffs cleanly in CI. `format` rewrites the Jinja layer only and is round-trip safe.

## Development

Every routine task has a `just` recipe — run `just` to list them.

```bash
just build
just test        # cargo nextest run
just test-e2e    # Python LSP-protocol suite against the real binary
just check-all   # everything CI gates on
```

## License

MIT
