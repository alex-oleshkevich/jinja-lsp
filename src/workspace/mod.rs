// TemplateIndex, WorkspaceIndex, symbol types, and discovery (REQ-FOLD-03).
// Reads parsing/; never reads features/ (REQ-FOLD-08).
pub mod index;
pub mod symbols;

pub fn layer_name() -> &'static str {
    "workspace"
}
