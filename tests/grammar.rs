use tree_sitter::{Parser, StreamingIterator};

#[test]
fn block_grammar_parses_tag() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_jinja::language())
        .expect("failed to load block grammar");

    let src = "{% if x %}hello{% endif %}";
    let tree = parser.parse(src, None).expect("parse returned None");
    assert!(!tree.root_node().has_error(), "parse produced error nodes");
}

#[test]
fn block_grammar_parses_expression() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_jinja::language())
        .expect("failed to load block grammar");

    let src = "{{ post.title | upper }}";
    let tree = parser.parse(src, None).expect("parse returned None");
    assert!(!tree.root_node().has_error(), "parse produced error nodes");
}

// REQ-EXTR-09: {% set name %}...{% endset %} (block-set) is parsed by tree-sitter
// as an ERROR node because the grammar's set_statement rule requires `=`.
// The identifier is still reachable via an (ERROR "set" ...) query —
// but only for the FIRST block-set in a template (subsequent ones are absorbed
// into the first ERROR without child nodes).  Extraction uses a manual scanner.
#[test]
fn block_grammar_block_set_is_error_node_with_reachable_identifier() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_jinja::language())
        .expect("failed to load block grammar");

    let src = "{% set nav %}hello{% endset %}{{ nav }}";
    let tree = parser.parse(src, None).expect("parse returned None");

    let q = tree_sitter::Query::new(
        &tree_sitter_jinja::language(),
        r#"(ERROR "set" (expression (binary_expression (unary_expression (primary_expression (identifier) @name)))))"#,
    ).expect("set_block.scm query must compile");

    let mut qcur = tree_sitter::QueryCursor::new();
    let mut ms = qcur.matches(&q, tree.root_node(), src.as_bytes());
    let mut captured: Vec<String> = vec![];
    while let Some(m) = ms.next() {
        for cap in m.captures {
            captured.push(cap.node.utf8_text(src.as_bytes()).unwrap_or("").to_owned());
        }
    }
    assert_eq!(
        captured,
        vec!["nav"],
        "block-set query must capture the variable name"
    );
}

#[test]
fn inline_grammar_parses_set_statement() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_jinja_inline::language())
        .expect("failed to load inline grammar");

    // The inline grammar uses '#' as the line-statement prefix
    let src = "# set x = 1";
    let tree = parser.parse(src, None).expect("parse returned None");
    assert!(!tree.root_node().has_error(), "parse produced error nodes");
}
