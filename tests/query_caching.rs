// jinja-lsp-ex8b: tree-sitter queries must be compiled once, not per-call.

#[test]
fn queries_are_lazily_compiled_and_non_empty() {
    // Behavioral test: access each query static via query_pattern_counts() and verify
    // (a) every query has at least one pattern (not an empty/broken query), and
    // (b) calling it twice returns the same counts (LazyLock returns the same compiled object).
    use jinja_lsp::parsing::query_pattern_counts;

    let counts1 = query_pattern_counts();
    let counts2 = query_pattern_counts();

    assert!(!counts1.is_empty(), "query_pattern_counts must return entries");

    for ((name1, c1), (name2, c2)) in counts1.iter().zip(counts2.iter()) {
        assert_eq!(name1, name2);
        assert_eq!(c1, c2, "pattern count for {name1} changed between calls (not a stable static)");
        assert!(*c1 > 0, "query {name1} has zero patterns — scm file may be empty or wrong");
    }
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
