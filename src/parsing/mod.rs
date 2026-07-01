// Tree-sitter wrapper: loads block/inline grammars, compiles .scm queries,
// exposes a typed cursor over parse results (REQ-FOLD-02).

mod extractor;
pub use extractor::extract;
pub use extractor::query_pattern_counts;

mod path_resolver;
pub use path_resolver::resolve_path;

mod discovery;
pub use discovery::discover_templates;

pub mod inline;

pub fn layer_name() -> &'static str {
    "parsing"
}
