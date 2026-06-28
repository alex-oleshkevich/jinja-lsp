// Jinja-only formatter engine — called by both the LSP formatting handler
// and the `jinja-lsp format` CLI front-end (REQ-FOLD-07).

pub fn layer_name() -> &'static str {
    "format"
}
