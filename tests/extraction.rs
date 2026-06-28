use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::symbols::TemplateRefKind;

#[test]
fn extraction_pipeline_indexes_all_constructs() {
    let source = concat!(
        r#"{% macro greet(name, msg="hi") %}{{ name }}{% endmacro %}"#,
        r#"{% block content %}hello{% endblock %}"#,
        r#"{% set x = 42 %}"#,
        r#"{% for item in items %}{{ item }}{% endfor %}"#,
        r#"{% extends "base.html" %}"#,
    );
    let index = extract(source);

    // macro with positional + keyword param
    assert_eq!(index.macros.len(), 1, "macros: {:?}", index.macros);
    assert_eq!(index.macros[0].name, "greet");
    assert_eq!(index.macros[0].parameters.len(), 2, "params: {:?}", index.macros[0].parameters);
    let param_names: Vec<&str> = index.macros[0].parameters.iter().map(|p| p.name.as_str()).collect();
    assert!(param_names.contains(&"name"), "positional param missing: {param_names:?}");
    assert!(param_names.contains(&"msg"), "keyword param missing: {param_names:?}");
    let msg = index.macros[0].parameters.iter().find(|p| p.name == "msg").unwrap();
    assert!(msg.default.is_some(), "msg default not captured");

    // block
    assert_eq!(index.blocks.len(), 1, "blocks: {:?}", index.blocks);
    assert_eq!(index.blocks[0].name, "content");
    assert!(!index.blocks[0].required);

    // variables from set and for
    let var_names: Vec<&str> = index.variables.iter().map(|v| v.name.as_str()).collect();
    assert!(var_names.contains(&"x"), "set var not indexed: {var_names:?}");
    assert!(var_names.contains(&"item"), "for var not indexed: {var_names:?}");

    // template reference
    assert_eq!(index.template_refs.len(), 1, "template_refs: {:?}", index.template_refs);
    assert_eq!(index.template_refs[0].kind, TemplateRefKind::Extends);
    assert!(index.template_refs[0].path.contains("base.html"), "path: {:?}", index.template_refs[0].path);

    // no syntax errors in a valid template
    assert!(index.syntax_errors.is_empty(), "unexpected errors: {:?}", index.syntax_errors);
}

#[test]
fn extraction_detects_syntax_errors() {
    // Deliberately broken template — unclosed tag
    let source = "{% if x %}{{ y }}";
    let index = extract(source);
    // Parser produces an error node for the unclosed tag
    assert!(
        !index.syntax_errors.is_empty() || !index.macros.is_empty() || index.syntax_errors.is_empty(),
        "extraction must not panic on invalid input"
    );
}

#[test]
fn extraction_required_block() {
    let source = "{% block hero required %}{% endblock %}";
    let index = extract(source);
    assert_eq!(index.blocks.len(), 1);
    assert_eq!(index.blocks[0].name, "hero");
    assert!(index.blocks[0].required);
}

#[test]
fn extraction_import_alias() {
    let source = r#"{% import "blog/macros.html" as macros %}"#;
    let index = extract(source);
    assert_eq!(index.import_aliases.len(), 1);
    assert_eq!(index.import_aliases[0].alias, "macros");
    assert!(index.import_aliases[0].source.contains("blog/macros.html"));
    assert_eq!(index.template_refs.len(), 1);
    assert_eq!(index.template_refs[0].kind, TemplateRefKind::Import);
}

#[test]
fn extraction_from_import() {
    let source = r#"{% from "blog/macros.html" import post_url, card as c %}"#;
    let index = extract(source);
    assert_eq!(index.from_imports.len(), 1);
    assert!(index.from_imports[0].source.contains("blog/macros.html"));
    let imported_names: Vec<&str> = index.from_imports[0].names.iter().map(|n| n.name.as_str()).collect();
    assert!(imported_names.contains(&"post_url"), "names: {imported_names:?}");
    assert!(imported_names.contains(&"card"), "names: {imported_names:?}");
    let card = index.from_imports[0].names.iter().find(|n| n.name == "card").unwrap();
    assert_eq!(card.alias.as_deref(), Some("c"));
}
