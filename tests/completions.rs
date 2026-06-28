// F05 — Completions tests: REQ-CMP-01 through REQ-CMP-12.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::completions::{complete, resolve_doc};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── REQ-CMP-02: each cursor context maps to the right candidates ─────────────

#[test]
fn cmp02_filter_context_offers_filters() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "filter context must offer items");
    // Must include built-in filters
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"truncate"), "truncate must be offered: {labels:?}");
    assert!(labels.contains(&"upper"), "upper must be offered: {labels:?}");
    assert!(labels.contains(&"lower"), "lower must be offered: {labels:?}");
}

#[test]
fn cmp02_filter_context_does_not_offer_variables() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    // Should not offer non-filter items; all items should be filters
    for item in &items {
        assert!(
            item.detail.as_deref().unwrap_or("").contains("filter")
                || item.data.as_deref().unwrap_or("").contains("filter"),
            "item {:?} in filter context must be a filter",
            item.label
        );
    }
}

#[test]
fn cmp02_expression_context_offers_variables_and_functions() {
    let src = "{% set my_var = 1 %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "expression context must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // my_var should be in scope
    assert!(labels.contains(&"my_var"), "set variable must be offered: {labels:?}");
    // range() is a built-in function
    assert!(labels.contains(&"range"), "global function 'range' must be offered: {labels:?}");
}

#[test]
fn cmp02_statement_context_offers_keywords() {
    let src = "{% ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "statement context must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"for"), "keyword 'for' must be offered: {labels:?}");
    assert!(labels.contains(&"if"), "keyword 'if' must be offered: {labels:?}");
    assert!(labels.contains(&"block"), "keyword 'block' must be offered: {labels:?}");
    assert!(labels.contains(&"macro"), "keyword 'macro' must be offered: {labels:?}");
}

#[test]
fn cmp02_attribute_context_offers_attrs_for_known_receiver() {
    let src = "{% for i in items %}{{ loop.";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(!items.is_empty(), "attribute context for 'loop' must offer items");
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"index"), "loop.index must be offered: {labels:?}");
    assert!(labels.contains(&"first"), "loop.first must be offered: {labels:?}");
}

// ─── REQ-CMP-03: attribute completions only for resolvable receivers ──────────

#[test]
fn cmp03_unknown_receiver_returns_empty() {
    let src = "{{ unknown_obj.";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    assert!(items.is_empty(), "unknown receiver must yield no completions: {items:?}");
}

// ─── REQ-CMP-05: lazy documentation resolve ───────────────────────────────────

#[test]
fn cmp05_resolve_returns_markdown_body() {
    let reg = Registry::load_core();
    // "filter:truncate" is the data key
    let doc = resolve_doc("filter:truncate", &reg);
    assert!(doc.is_some(), "resolve must return doc for truncate");
    let d = doc.unwrap();
    assert!(d.contains("truncate"), "resolved doc must mention truncate");
}

#[test]
fn cmp05_resolve_returns_none_for_unknown() {
    let reg = Registry::load_core();
    let doc = resolve_doc("filter:no_such_filter_xyz", &reg);
    assert!(doc.is_none(), "resolve must return None for unknown symbol");
}

// ─── REQ-CMP-06: nothing outside Jinja delimiters ────────────────────────────

#[test]
fn cmp06_outside_delimiter_returns_empty() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor in the middle of plain HTML
    let items = complete(src, 0, 5, &idx, &reg, &ws);
    assert!(items.is_empty(), "plain HTML must return empty completions");
}

#[test]
fn cmp06_inside_comment_returns_empty() {
    let src = "{# some comment text #}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, 10, &idx, &reg, &ws);
    assert!(items.is_empty(), "comment interior must return empty completions");
}

// ─── REQ-CMP-07: item fields ─────────────────────────────────────────────────

#[test]
fn cmp07_filter_item_has_required_fields() {
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let truncate = items.iter().find(|i| i.label == "truncate").unwrap();
    assert!(!truncate.label.is_empty(), "label must be set");
    // detail should contain filter information
    assert!(truncate.detail.is_some(), "detail must be set");
    // data must be set for lazy resolve
    assert!(truncate.data.is_some(), "data must be set for resolve");
    let data = truncate.data.as_deref().unwrap();
    assert!(data.contains("truncate"), "data must contain the symbol name");
}

#[test]
fn cmp07_item_has_no_documentation_initially() {
    // Items ship without documentation; resolve adds it (REQ-CMP-05).
    let src = "{{ x | ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let any_with_doc = items.iter().any(|i| i.documentation.is_some());
    assert!(!any_with_doc, "items must not have documentation until resolved");
}

// ─── REQ-CMP-11: scope-local variables in expression position ────────────────

#[test]
fn cmp11_for_loop_var_offered_in_expression() {
    // The loop variable "item" should be offered inside {{ }} within the loop.
    let src = "{% for item in items %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"item"), "for loop variable must be offered: {labels:?}");
}

#[test]
fn cmp11_set_var_offered_in_expression() {
    let src = "{% set title = 'Hello' %}{{ ";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"title"), "set variable must be offered: {labels:?}");
}

// ─── REQ-CMP-12: template paths one directory at a time ──────────────────────

#[test]
fn cmp12_path_context_offers_workspace_templates() {
    let src = r#"{% extends ""#;
    let idx = extract(src);
    let reg = Registry::load_core();
    // Workspace with one template
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", "{% block content %}{% endblock %}");
    let items = complete(src, 0, src.len() as u32, &idx, &reg, &ws);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"base.html"), "workspace template must be offered: {labels:?}");
}
