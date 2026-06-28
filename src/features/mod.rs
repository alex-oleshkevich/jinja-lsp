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
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbols;

pub fn layer_name() -> &'static str {
    "features"
}
