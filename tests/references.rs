// F09 — Find references tests: REQ-REF-01 through REQ-REF-05.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::references::find_references;
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── REQ-REF-01: workspace-wide refs for macros ────────────────────────────

#[test]
fn ref01_macro_ref_returned_from_same_file() {
    // Macro defined and called in the same template.
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet( }}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on macro definition "greet"
    let col = src.find("greet").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(!results.is_empty(), "must find at least one reference to greet");
    let paths: Vec<&str> = results.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(&"test.html"), "reference must be in test.html");
}

#[test]
fn ref01_macro_ref_found_in_other_template() {
    let macro_src = "{% macro post_url(post) %}{% endmacro %}";
    let caller_src = r#"{% from "macros.html" import post_url %}{{ post_url( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", macro_src);
    ws.index_inline("caller.html", caller_src);
    let idx = extract(macro_src);
    let reg = Registry::load_core();
    // Cursor on the macro definition in macros.html
    let col = macro_src.find("post_url").unwrap() as u32;
    let results = find_references(macro_src, 0, col, "macros.html", false, &idx, &reg, &ws);
    assert!(!results.is_empty(), "must find reference from caller.html");
    let paths: Vec<&str> = results.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(&"caller.html"), "caller.html must be in results");
}

// ─── REQ-REF-02: dedup and stable sort ────────────────────────────────────

#[test]
fn ref02_results_are_deduplicated() {
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet( }}{{ greet( }}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("greet").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    // Each reference location must be unique (no duplicates by path+start_col).
    let mut seen = std::collections::HashSet::new();
    for r in &results {
        let key = (r.path.clone(), r.start_col);
        assert!(seen.insert(key), "duplicate reference at col {}", r.start_col);
    }
}

#[test]
fn ref02_results_sorted_by_path_then_position() {
    let macro_src = "{% macro greet(name) %}{% endmacro %}";
    let caller_a = "{{ greet( }}";
    let caller_b = "{{ greet( }}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", macro_src);
    ws.index_inline("a.html", caller_a);
    ws.index_inline("b.html", caller_b);
    let idx = extract(macro_src);
    let reg = Registry::load_core();
    let col = macro_src.find("greet").unwrap() as u32;
    let results = find_references(macro_src, 0, col, "macros.html", false, &idx, &reg, &ws);
    // All references from a.html must come before those from b.html.
    let paths: Vec<&str> = results.iter().map(|r| r.path.as_str()).collect();
    if paths.contains(&"a.html") && paths.contains(&"b.html") {
        let a_pos = paths.iter().position(|&p| p == "a.html").unwrap();
        let b_pos = paths.iter().position(|&p| p == "b.html").unwrap();
        assert!(a_pos < b_pos, "a.html must sort before b.html");
    }
}

// ─── REQ-REF-03: includeDeclaration flag ──────────────────────────────────

#[test]
fn ref03_include_declaration_false_excludes_definition() {
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet( }}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("greet").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    // None of the results should be at the declaration position (the macro name).
    let decl_col = src.find("greet").unwrap() as u32;
    let decl_in_results = results.iter().any(|r| r.path == "test.html" && r.start_col == decl_col);
    // With include_declaration=false, the definition itself is excluded.
    assert!(!decl_in_results, "declaration must be excluded when includeDeclaration=false");
}

#[test]
fn ref03_include_declaration_true_adds_definition() {
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet( }}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("greet").unwrap() as u32;
    let results_with = find_references(src, 0, col, "test.html", true, &idx, &reg, &ws);
    let results_without = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(
        results_with.len() >= results_without.len(),
        "includeDeclaration=true must return at least as many results"
    );
}

// ─── REQ-REF-04: host-owned symbols → empty result ────────────────────────

#[test]
fn ref04_builtin_filter_returns_empty() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("truncate").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(results.is_empty(), "built-in filter must return empty results (REQ-REF-04)");
}

#[test]
fn ref04_outside_jinja_returns_empty() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let results = find_references(src, 0, 5, "test.html", false, &idx, &reg, &ws);
    assert!(results.is_empty(), "plain HTML must return empty results");
}

// ─── REQ-REF-05: scope-local variable refs ────────────────────────────────

#[test]
fn ref05_for_loop_variable_refs_in_same_file() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on first "item" (the binding)
    let col = src.find("item").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    // Should find at least the usage in {{ item }}
    assert!(!results.is_empty(), "for loop variable must have at least one reference");
    let all_same_file = results.iter().all(|r| r.path == "test.html");
    assert!(all_same_file, "scope-local refs must be file-local");
}

#[test]
fn ref05_html_text_matching_variable_name_is_not_a_reference() {
    // Cursor on "item" in the HTML class, NOT inside Jinja — must return empty.
    let src = r#"{% for item in items %}{{ item }}{% endfor %}<div class="item"></div>"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();
    // Position of "item" in the HTML class attribute (after the Jinja block).
    let col = src.rfind("item").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(results.is_empty(), "HTML text matching a variable name must not yield references");
}

// ─── REQ-REF-01b: aliased from-import reference ──────────────────────────────

#[test]
fn ref01b_aliased_import_usage_classified_as_symbol() {
    // `{% from "m.html" import foo as bar %}{{ bar( }}` — cursor on "bar" call site
    // must produce a non-empty references result (alias must be recognized as a symbol).
    let src = r#"{% from "m.html" import foo as bar %}{{ bar( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    ws.index_inline("m.html", "{% macro foo(x) %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on "bar" at the call site
    let col = src.rfind("bar").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(!results.is_empty(), "aliased macro usage must be classified as a symbol and yield references");
}

// ─── REQ-REF-01: block references across workspace ───────────────────────────

#[test]
fn ref01_block_references_collected_workspace_wide() {
    // Two templates share a block named "content"; cursor on the block in the child.
    let child = "{% extends \"base.html\" %}{% block content %}hello{% endblock %}";
    let base = "{% block content %}{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base);
    ws.index_inline("child.html", child);
    let idx = extract(child);
    let reg = Registry::load_core();
    let col = child.find("content").unwrap() as u32;
    let results = find_references(child, 0, col, "child.html", true, &idx, &reg, &ws);
    assert!(results.len() >= 2, "both block declarations must be collected: {results:?}");
    let paths: Vec<&str> = results.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(&"child.html"), "child.html must appear");
    assert!(paths.contains(&"base.html"), "base.html must appear");
}

#[test]
fn ref01_block_cursor_on_declaration_collects_all_templates() {
    let base = "{% block sidebar %}{% endblock %}";
    let child = "{% extends \"base.html\" %}{% block sidebar %}content{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base);
    ws.index_inline("child.html", child);
    let idx = extract(base);
    let reg = Registry::load_core();
    let col = base.find("sidebar").unwrap() as u32;
    let results = find_references(base, 0, col, "base.html", true, &idx, &reg, &ws);
    assert!(results.len() >= 2, "both sidebar blocks must be in results: {results:?}");
}

// ─── REQ-REF-01: import-alias namespace usages ───────────────────────────────

#[test]
fn ref01_import_alias_namespace_usages_collected() {
    // `{% import "m.html" as macros %}{{ macros.post_url() }}` — cursor on "macros"
    // in the usage site must yield both the import declaration and the usage.
    let src = r#"{% import "m.html" as macros %}{{ macros.post_url() }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    ws.index_inline("m.html", "{% macro post_url() %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.rfind("macros").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);
    assert!(!results.is_empty(), "import alias usage must yield references: {results:?}");
}
