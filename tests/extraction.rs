use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::symbols::{ReferenceKind, TemplateRefKind};

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
    assert_eq!(
        index.macros[0].parameters.len(),
        2,
        "params: {:?}",
        index.macros[0].parameters
    );
    let param_names: Vec<&str> = index.macros[0]
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(
        param_names.contains(&"name"),
        "positional param missing: {param_names:?}"
    );
    assert!(
        param_names.contains(&"msg"),
        "keyword param missing: {param_names:?}"
    );
    let msg = index.macros[0]
        .parameters
        .iter()
        .find(|p| p.name == "msg")
        .unwrap();
    assert!(msg.default.is_some(), "msg default not captured");

    // block
    assert_eq!(index.blocks.len(), 1, "blocks: {:?}", index.blocks);
    assert_eq!(index.blocks[0].name, "content");
    assert!(!index.blocks[0].required);

    // variables from set and for
    let var_names: Vec<&str> = index.variables.iter().map(|v| v.name.as_str()).collect();
    assert!(
        var_names.contains(&"x"),
        "set var not indexed: {var_names:?}"
    );
    assert!(
        var_names.contains(&"item"),
        "for var not indexed: {var_names:?}"
    );

    // template reference
    assert_eq!(
        index.template_refs.len(),
        1,
        "template_refs: {:?}",
        index.template_refs
    );
    assert_eq!(index.template_refs[0].kind, TemplateRefKind::Extends);
    assert!(
        index.template_refs[0].path.contains("base.html"),
        "path: {:?}",
        index.template_refs[0].path
    );

    // no syntax errors in a valid template
    assert!(
        index.syntax_errors.is_empty(),
        "unexpected errors: {:?}",
        index.syntax_errors
    );
}

#[test]
fn extraction_detects_syntax_errors() {
    // Deliberately malformed template — truncated tag delimiter produces an ERROR node.
    // Tree-sitter's Jinja grammar marks "{%" without a closing "%}" as a syntax error.
    let source = "{%";
    let index = extract(source);
    assert!(
        !index.syntax_errors.is_empty(),
        "extraction must record a syntax error for truncated '{{%%' delimiter"
    );
}

#[test]
fn extraction_does_not_panic_on_unclosed_tag() {
    // Unclosed {% if %} may or may not produce a syntax_error node (grammar-dependent)
    // but must not panic or lose data.
    let source = "{% if x %}{{ y }}";
    let index = extract(source);
    // Must not panic and must at least extract the variable reference.
    let ref_names: Vec<&str> = index.references.iter().map(|r| r.name.as_str()).collect();
    assert!(
        ref_names.contains(&"x") || ref_names.contains(&"y"),
        "extractor must still index references even with unclosed tags: {ref_names:?}"
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

// ── jinja-lsp-k8oy: filter references after attribute chains ───────────────

#[test]
fn extr10_filter_after_attr_captured_as_filter() {
    // {{ post.title | upper }} — 'upper' must be captured as ReferenceKind::Filter
    let src = "{{ post.title | upper }}";
    let idx = extract(src);
    let filter_refs: Vec<_> = idx
        .references
        .iter()
        .filter(|r| r.name == "upper" && r.kind == ReferenceKind::Filter)
        .collect();
    assert!(
        !filter_refs.is_empty(),
        "upper must be captured as Filter in '{{ post.title | upper }}';\n  references = {:?}",
        idx.references
    );
}

#[test]
fn extr10_filter_with_args_after_attr_captured_as_function() {
    // {{ post.title | truncate(60) }} — treesitter promotes filter(args) to function_call,
    // so 'truncate' must be captured as ReferenceKind::Function (enabling the hover fallback).
    let src = "{{ post.title | truncate(60) }}";
    let idx = extract(src);
    let fn_refs: Vec<_> = idx
        .references
        .iter()
        .filter(|r| r.name == "truncate" && r.kind == ReferenceKind::Function)
        .collect();
    assert!(
        !fn_refs.is_empty(),
        "truncate must be captured as Function in '{{ post.title | truncate(60) }}';\n  references = {:?}",
        idx.references
    );
}

#[test]
fn extr10_inline_gettext_underscore_captured_as_function() {
    // {{ _('Upload signed PDF') }} — the grammar parses this as a dedicated
    // inline_trans node (seq('_', '(', expression, ')')), not a generic
    // function_call, so it needs its own query pattern or it's invisible to
    // hover/goto-definition despite `_` being a real, documented builtin
    // (starlette_babel/func__.md).
    let src = "{{ _('Upload signed PDF') }}";
    let idx = extract(src);
    let fn_refs: Vec<_> = idx
        .references
        .iter()
        .filter(|r| r.name == "_" && r.kind == ReferenceKind::Function)
        .collect();
    assert!(
        !fn_refs.is_empty(),
        "_ must be captured as Function in '{{ _('Upload signed PDF') }}';\n  references = {:?}",
        idx.references
    );
}

#[test]
fn extr10_deep_attr_chain_filter_captured() {
    // {{ post.author.name | truncate(60) }} — two-level attribute chain before filter
    let src = "{{ post.author.name | truncate(60) }}";
    let idx = extract(src);
    let fn_refs: Vec<_> = idx
        .references
        .iter()
        .filter(|r| r.name == "truncate" && r.kind == ReferenceKind::Function)
        .collect();
    assert!(
        !fn_refs.is_empty(),
        "truncate must be captured after deep attr chain;\n  references = {:?}",
        idx.references
    );
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
    let imported_names: Vec<&str> = index.from_imports[0]
        .names
        .iter()
        .map(|n| n.name.as_str())
        .collect();
    assert!(
        imported_names.contains(&"post_url"),
        "names: {imported_names:?}"
    );
    assert!(
        imported_names.contains(&"card"),
        "names: {imported_names:?}"
    );
    let card = index.from_imports[0]
        .names
        .iter()
        .find(|n| n.name == "card")
        .unwrap();
    assert_eq!(card.alias.as_deref(), Some("c"));
}

#[test]
fn jinja_lsp_fx8f_unclosed_block_does_not_leak_scoped_from_later_source() {
    // jinja-lsp-fx8f: the scoped-keyword scan used unwrap_or(after.len()) when no `%}`
    // was found, so an unclosed `{% block %}` tag scanned all the way to EOF looking
    // for "scoped" — a later, unrelated line containing that word would wrongly mark
    // the block as scoped. The scan must stop at the next newline/tag opener instead.
    let source = "{% block foo\nplain text mentions scoped later\n";
    let index = extract(source);
    assert_eq!(index.blocks.len(), 1, "blocks: {:?}", index.blocks);
    assert!(
        !index.blocks[0].scoped,
        "unclosed block must not pick up 'scoped' from a later, unrelated line: {:?}",
        index.blocks[0]
    );
}

#[test]
fn jinja_lsp_8my3_unclosed_with_falls_back_to_end_of_source() {
    // jinja-lsp-8my3: run_with fell back to an empty valid_range
    // (byte_span(with_ctrl_end, with_ctrl_end)) when no {% endwith %} exists (e.g.
    // mid-edit), so a use of the with-bound name anywhere after the tag fell outside
    // its valid_range. run_for/run_set fall back to end-of-source for the same
    // incomplete-template case; run_with must match.
    let source = "{% with x = 1 %}\n{{ x }}";
    let index = extract(source);
    let x = index
        .variables
        .iter()
        .find(|v| v.name == "x")
        .expect("with-bound variable must be indexed");
    assert!(
        x.valid_range.end_byte >= source.len(),
        "unclosed with must fall back to end-of-source, not an empty range: {:?}",
        x.valid_range
    );
}

#[test]
fn jinja_lsp_smvv_set_block_in_comment_is_not_extracted() {
    // jinja-lsp-smvv: run_set_block scanned raw bytes for {% set NAME %} with no
    // awareness of {# ... #} comment regions, so a commented-out block-set tag
    // created a phantom VariableDefinition — suppressing legitimate
    // undefined-variable diagnostics for that name.
    let source = "{# {% set nav %} #}{{ nav }}";
    let index = extract(source);
    assert!(
        index.variables.iter().all(|v| v.name != "nav"),
        "a block-set tag written inside a comment must not be extracted: {:?}",
        index.variables
    );
}

#[test]
fn jinja_lsp_smvv_set_block_in_raw_is_not_extracted() {
    // jinja-lsp-smvv: same bug for {% raw %}...{% endraw %} bodies — a block-set
    // tag escaped as literal text must not be extracted as a real binding.
    let source = "{% raw %}{% set nav %}{% endraw %}{{ nav }}";
    let index = extract(source);
    assert!(
        index.variables.iter().all(|v| v.name != "nav"),
        "a block-set tag written inside {{% raw %}} must not be extracted: {:?}",
        index.variables
    );
}

#[test]
fn jinja_lsp_smvv_real_set_block_still_extracted() {
    // Sanity: a genuine block-set outside any raw/comment region must still work.
    let source = "{% set nav %}hello{% endset %}{{ nav }}";
    let index = extract(source);
    assert!(
        index.variables.iter().any(|v| v.name == "nav"),
        "a real block-set must still be extracted: {:?}",
        index.variables
    );
}

#[test]
fn jinja_lsp_mojm_include_template_refs_are_in_document_order() {
    // jinja-lsp-mojm: include TemplateReferences were accumulated in a HashMap and
    // appended via into_values(), so their order in idx.template_refs varied run to
    // run — any consumer iterating template_refs (e.g. missing-template checks)
    // could produce diagnostics in unstable order. Includes must come out sorted by
    // where they appear in the source.
    let source = concat!(
        r#"{% include "a.html" %}"#,
        r#"{% include "b.html" %}"#,
        r#"{% include "c.html" %}"#,
        r#"{% include "d.html" %}"#,
        r#"{% include "e.html" %}"#,
    );
    let index = extract(source);
    let include_paths: Vec<&str> = index
        .template_refs
        .iter()
        .filter(|r| r.kind == TemplateRefKind::Include)
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(
        include_paths,
        vec!["a.html", "b.html", "c.html", "d.html", "e.html"],
        "include template_refs must be in document order: {include_paths:?}"
    );
}
