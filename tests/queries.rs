// REQ-EXTR-01 + REQ-EXTR-02: verify that all 16 .scm query files compile
// against the upstream grammar and capture expected constructs from fixtures.

use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

const QUERY_NAMES: &[&str] = &[
    "macros",
    "params",
    "blocks",
    "set",
    "set_unpacking",
    "for",
    "for_unpacking",
    "with",
    "trans",
    "extends",
    "includes",
    "imports",
    "from_imports",
    "import_names",
    "caller_args",
    "references",
];

fn query_src(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/parsing/queries")
        .join(format!("{name}.scm"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing query file: {}", path.display()))
}

fn parse_block(src: &str) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_jinja::language()).unwrap();
    parser.parse(src, None).unwrap()
}

// REQ-EXTR-02: every .scm query must compile against the block grammar
// (node-type drift surfaces here as a query error, not silent empty captures)
#[test]
fn all_queries_compile_against_block_grammar() {
    let lang = tree_sitter_jinja::language();
    for name in QUERY_NAMES {
        let src = query_src(name);
        Query::new(&lang, &src)
            .unwrap_or_else(|e| panic!("query '{name}.scm' failed to compile: {e}"));
    }
}

// ── Fixture tests: each query must capture its construct ─────────────────────

fn captures(query_name: &str, template: &str) -> Vec<String> {
    let lang = tree_sitter_jinja::language();
    let src = query_src(query_name);
    let query = Query::new(&lang, &src).expect("query compile");
    let tree = parse_block(template);
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), template.as_bytes());
    let mut out = Vec::new();
    while let Some(m) = matches.next() {
        for c in m.captures {
            out.push(c.node.utf8_text(template.as_bytes()).unwrap().to_owned());
        }
    }
    out
}

#[test]
fn macros_captures_macro_name() {
    let caps = captures("macros", "{% macro post_url(post) %}{% endmacro %}");
    assert!(caps.contains(&"post_url".to_owned()), "macro name not captured: {caps:?}");
}

#[test]
fn blocks_captures_block_name() {
    let caps = captures("blocks", "{% block content %}{% endblock %}");
    assert!(caps.contains(&"content".to_owned()), "block name not captured: {caps:?}");
}

#[test]
fn blocks_captures_required_flag() {
    let caps = captures("blocks", "{% block content required %}{% endblock %}");
    assert!(caps.contains(&"required".to_owned()), "required flag not captured: {caps:?}");
}

#[test]
fn set_captures_variable_name() {
    let caps = captures("set", "{% set my_var = 42 %}");
    assert!(caps.iter().any(|c| c.contains("my_var")), "variable name not captured: {caps:?}");
}

#[test]
fn for_captures_loop_variable() {
    let caps = captures("for", "{% for item in items %}{% endfor %}");
    assert!(caps.contains(&"item".to_owned()), "loop variable not captured: {caps:?}");
}

#[test]
fn extends_captures_path() {
    let caps = captures("extends", r#"{% extends "base.html" %}"#);
    assert!(caps.iter().any(|c| c.contains("base.html")), "extends path not captured: {caps:?}");
}

#[test]
fn includes_captures_path() {
    let caps = captures("includes", r#"{% include "header.html" %}"#);
    assert!(caps.iter().any(|c| c.contains("header.html")), "include path not captured: {caps:?}");
}

#[test]
fn includes_captures_ignore_missing() {
    let caps = captures("includes", r#"{% include "missing.html" ignore missing %}"#);
    assert!(
        caps.iter().any(|c| c.contains("ignore") || c.contains("missing")),
        "ignore missing not captured: {caps:?}"
    );
}

#[test]
fn imports_captures_source_and_alias() {
    let caps = captures("imports", r#"{% import "blog/macros.html" as macros %}"#);
    assert!(caps.iter().any(|c| c.contains("blog/macros.html")), "import source not captured: {caps:?}");
    assert!(caps.contains(&"macros".to_owned()), "import alias not captured: {caps:?}");
}

#[test]
fn from_imports_captures_source() {
    let caps = captures("from_imports", r#"{% from "blog/macros.html" import post_url %}"#);
    assert!(caps.iter().any(|c| c.contains("blog/macros.html")), "from-import source not captured: {caps:?}");
}

#[test]
fn params_captures_positional_param() {
    let caps = captures("params", "{% macro greet(name) %}{% endmacro %}");
    assert!(caps.contains(&"name".to_owned()), "positional param not captured: {caps:?}");
}

#[test]
fn params_captures_keyword_param_with_default() {
    let caps = captures("params", r#"{% macro greet(msg="hi") %}{% endmacro %}"#);
    assert!(caps.contains(&"msg".to_owned()), "keyword param not captured: {caps:?}");
    assert!(caps.iter().any(|c| c.contains("hi")), "param default not captured: {caps:?}");
}

#[test]
fn with_captures_variable() {
    let caps = captures("with", "{% with x = 5 %}{% endwith %}");
    assert!(caps.contains(&"x".to_owned()), "with variable not captured: {caps:?}");
}

#[test]
fn trans_captures_plural_variable() {
    let caps = captures("trans", "{% trans count %}{% endtrans %}");
    assert!(caps.contains(&"count".to_owned()), "trans plural var not captured: {caps:?}");
}

#[test]
fn import_names_captures_names_and_aliases() {
    let caps = captures("import_names", r#"{% from "m.html" import foo, bar as baz %}"#);
    assert!(caps.contains(&"foo".to_owned()), "imported name not captured: {caps:?}");
    assert!(caps.contains(&"baz".to_owned()), "import alias not captured: {caps:?}");
}

#[test]
fn set_unpacking_captures_both_names() {
    let caps = captures("set_unpacking", "{% set x, y = (1, 2) %}");
    assert!(caps.iter().any(|c| c == "x"), "first unpacked name not captured: {caps:?}");
    assert!(caps.iter().any(|c| c == "y"), "second unpacked name not captured: {caps:?}");
}

#[test]
fn for_unpacking_captures_both_names() {
    let caps = captures("for_unpacking", "{% for key, value in items %}{% endfor %}");
    assert!(caps.iter().any(|c| c == "key"), "first loop name not captured: {caps:?}");
    assert!(caps.iter().any(|c| c == "value"), "second loop name not captured: {caps:?}");
}

#[test]
fn caller_args_captures_caller_variable() {
    let caps = captures("caller_args", "{% call (c) render_dialog() %}{% endcall %}");
    assert!(caps.contains(&"c".to_owned()), "caller var not captured: {caps:?}");
}

#[test]
fn references_captures_identifier() {
    let caps = captures("references", "{{ post.title }}");
    assert!(caps.iter().any(|c| c == "post" || c == "title"), "reference not captured: {caps:?}");
}
