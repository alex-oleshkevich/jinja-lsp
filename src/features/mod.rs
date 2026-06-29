// One handler module per LSP feature (REQ-FOLD-06).
// Each is a pure-read handler dispatched from server.rs.
pub mod call_hierarchy;
pub mod code_actions;
pub mod code_lens;
pub mod completions;
pub mod definition;
pub mod document_highlight;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod inlay_hints;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbols;
pub mod wrap;

pub fn layer_name() -> &'static str {
    "features"
}

/// Extract the Jinja identifier word centered at `byte` in `source`.
pub(super) fn word_at_byte(source: &str, byte: usize) -> &str {
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(source.len());
    &source[start..end]
}
