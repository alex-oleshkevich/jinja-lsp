// Tree-sitter wrapper: loads block/inline grammars, compiles .scm queries,
// exposes a typed cursor over parse results (REQ-FOLD-02).

mod extractor;
pub use extractor::extract;

pub fn layer_name() -> &'static str {
    "parsing"
}
