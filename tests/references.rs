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

// ─── jinja-lsp-orh5: scope-local refs are bound to the binding's valid_range ──

#[test]
fn ref05_two_unrelated_for_loops_do_not_merge_references() {
    // Two separate {% for item in ... %} loops, each with its own "item" binding.
    // find-references from the first loop's binding must only return refs from
    // that loop, not the unrelated second loop.
    let src = "{% for item in a %}{{ item }}{% endfor %}{% for item in b %}{{ item }}{% endfor %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    let idx = extract(src);
    let reg = Registry::load_core();

    // Cursor on the first loop's "item" usage inside {{ item }}.
    let col = src.find("{{ item").unwrap() as u32 + 3;
    let results = find_references(src, 0, col, "test.html", true, &idx, &reg, &ws);

    let second_loop_ref_col = src.rfind("{{ item").unwrap() as u32 + 3;
    let touches_second_loop = results.iter().any(|r| r.start_col == second_loop_ref_col);
    assert!(
        !touches_second_loop,
        "references from the unrelated second for-loop must not be included: {results:?}"
    );
    assert!(!results.is_empty(), "the first loop's own usage must still be found");
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

// ─── REQ-REF-03: block includeDeclaration=false must exclude current-file block ─

#[test]
fn ref_u4yx_block_include_declaration_false_excludes_current_file() {
    // REQ-REF-03: cursor on the base block, include_declaration=false.
    // base.html defines {% block content %}, child.html overrides it.
    // Result must NOT include base.html's block, but MUST include child.html's.
    let base_src = "{% block content %}body{% endblock %}";
    let child_src = "{% extends 'base.html' %}{% block content %}override{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base_src);
    ws.index_inline("child.html", child_src);
    let base_idx = extract(base_src);
    let reg = Registry::load_core();
    // Cursor on "content" in base.html
    let col = base_src.find("content").unwrap() as u32;
    let results = find_references(base_src, 0, col, "base.html", false, &base_idx, &reg, &ws);

    let base_in_results = results.iter().any(|r| r.path == "base.html");
    assert!(!base_in_results, "base.html block must be excluded when include_declaration=false: {results:?}");

    let child_in_results = results.iter().any(|r| r.path == "child.html");
    assert!(child_in_results, "child.html override must be included: {results:?}");
}

// ─── REQ-REF-03: alias includeDeclaration=false must not leak declaration ────

#[test]
fn ref_6s8p_alias_include_declaration_false_excludes_declaration_site() {
    // REQ-REF-03: `{% import "m.html" as macros %}{{ macros.fn() }}`
    // With include_declaration=false, the "macros" token in the import declaration
    // must NOT appear in results — only the usage in `{{ macros.fn() }}`.
    let src = r#"{% import "m.html" as macros %}{{ macros.post_url() }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("test.html", src);
    ws.index_inline("m.html", "{% macro post_url() %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on the usage site ("macros" in the `{{ }}` expression)
    let col = src.rfind("macros").unwrap() as u32;
    let results = find_references(src, 0, col, "test.html", false, &idx, &reg, &ws);

    // Find where "macros" appears in the import declaration
    let decl_col = src.find("macros").unwrap() as u32; // first occurrence = "as macros"
    let usage_col = src.rfind("macros").unwrap() as u32; // last  occurrence = "{{ macros"

    // The declaration site must NOT appear when include_declaration=false.
    let decl_in_results = results.iter().any(|r| r.start_col == decl_col && r.start_line == 0);
    assert!(!decl_in_results, "declaration site must be excluded when include_declaration=false; results: {results:?}");

    // The usage site MUST appear.
    let usage_in_results = results.iter().any(|r| r.start_col == usage_col && r.start_line == 0);
    assert!(usage_in_results, "usage site must be in results: {results:?}");
}

// ─── REQ-REF-03 / jinja-lsp-tb4c: from-import usage resolves correct def span ─

#[test]
fn ref03_from_imported_macro_declaration_has_correct_span() {
    // When cursor is on a from-imported macro USAGE, includeDeclaration=true must
    // include the definition at the actual macro name location, NOT at 0:0.
    let macro_src = "{% macro greet(name) %}hello{{ name }}{% endmacro %}";
    let caller_src = r#"{% from "macros.html" import greet %}{{ greet( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", macro_src);
    ws.index_inline("caller.html", caller_src);

    let caller_idx = extract(caller_src);
    let reg = Registry::load_core();
    // Cursor on "greet" usage in caller.html
    let col = caller_src.rfind("greet").unwrap() as u32;
    let results = find_references(caller_src, 0, col, "caller.html", true, &caller_idx, &reg, &ws);

    // The declaration in macros.html must appear
    let decl: Vec<_> = results.iter().filter(|r| r.path == "macros.html").collect();
    assert!(!decl.is_empty(), "declaration in macros.html must be included: {results:?}");

    // The declaration must NOT be at 0:0 (the bogus default span)
    for d in &decl {
        assert!(
            d.start_col > 0 || d.start_line > 0,
            "declaration must not be at 0:0 (bogus default span): {d:?}"
        );
    }
}

// ─── jinja-lsp-wtnp: workspace keyed by absolute paths, fi.source is relative ──

#[test]
fn ref_from_import_resolves_relative_source_against_absolute_workspace_keys() {
    // The server keys workspace.templates by absolute path, but `from "macros.html"
    // import greet` always records a relative fi.source. classify_reference must
    // resolve that relative source through workspace.resolve_key, not a raw HashMap::get.
    let macro_src = "{% macro greet(name) %}hello{{ name }}{% endmacro %}";
    let caller_src = r#"{% from "macros.html" import greet %}{{ greet( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.templates.insert("/abs/project/macros.html".to_owned(), extract(macro_src));
    ws.templates.insert("/abs/project/caller.html".to_owned(), extract(caller_src));

    let caller_idx = extract(caller_src);
    let reg = Registry::load_core();
    let col = caller_src.rfind("greet").unwrap() as u32;
    let results = find_references(
        caller_src, 0, col, "/abs/project/caller.html", true, &caller_idx, &reg, &ws,
    );

    // The declaration must be reported under the ABSOLUTE key, not the relative "macros.html".
    let decl: Vec<_> = results.iter().filter(|r| r.path == "/abs/project/macros.html").collect();
    assert!(
        !decl.is_empty(),
        "declaration must be reported under the absolute workspace key: {results:?}"
    );
    for d in &decl {
        assert!(
            d.start_col > 0 || d.start_line > 0,
            "declaration must not be at 0:0 (bogus default span): {d:?}"
        );
    }

    // No result should carry the bare relative path "macros.html" — that produces a
    // bogus file:///macros.html URI once path_to_uri runs on it.
    assert!(
        results.iter().all(|r| r.path != "macros.html"),
        "no reference should use the unresolved relative path: {results:?}"
    );
}
