use tree_sitter::Parser;

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
