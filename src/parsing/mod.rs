// Tree-sitter wrapper: loads block/inline grammars, compiles .scm queries,
// exposes a typed cursor over parse results (REQ-FOLD-02).

pub fn layer_name() -> &'static str {
    "parsing"
}
