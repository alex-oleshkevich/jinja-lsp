// jinja-lsp-ex8b: tree-sitter queries must be compiled once, not per-call.

#[test]
fn queries_compiled_lazily_not_per_call() {
    let src = include_str!("../src/parsing/extractor.rs");
    // qry() was the per-call compiler that ran Query::new on every extract().
    // It must no longer exist.
    assert!(
        !src.contains("fn qry("),
        "per-call query compiler fn qry() must be removed — queries must use lazy statics"
    );
    // LazyLock<Query> is the expected replacement.
    assert!(
        src.contains("LazyLock"),
        "queries must be compiled once via LazyLock"
    );
}

#[test]
fn extract_is_idempotent_for_same_source() {
    // Basic sanity: extract() can be called multiple times without panicking.
    use jinja_lsp::parsing::extract;
    let src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let idx1 = extract(src);
    let idx2 = extract(src);
    assert_eq!(idx1.macros.len(), idx2.macros.len(), "repeated extract must give same result");
}
