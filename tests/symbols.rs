// F10 — Symbols tests: REQ-SYM-01 through REQ-SYM-05.

use jinja_lsp::features::symbols::{document_symbols, workspace_symbols, SymbolKind};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn find_sym<'a>(
    syms: &'a [jinja_lsp::features::symbols::DocumentSymbol],
    name: &str,
) -> Option<&'a jinja_lsp::features::symbols::DocumentSymbol> {
    syms.iter().find(|s| s.name == name)
}

// ─── name_span_in word-boundary fix ──────────────────────────────────────────

#[test]
fn sym_q634_block_named_lock_selection_points_to_name_not_keyword() {
    // "lock" is a substring of the keyword "block" — name_span_in must find the
    // standalone identifier, not the occurrence inside "block".
    let src = "{% block lock %}body{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "lock").expect("block 'lock' must appear in symbols");
    // In "{% block lock %}", "lock" as the block name starts at byte 9.
    // If name_span_in returns byte 4 (inside "block"), that's the bug.
    let lock_byte = src.find(" lock ").map(|p| p + 1).unwrap() as u32;
    assert_eq!(
        s.selection_range.start_col, lock_byte,
        "selection_range must point to the block name, not inside the keyword; got col {}",
        s.selection_range.start_col
    );
}

#[test]
fn sym_q634_macro_named_acro_selection_points_to_name_not_keyword() {
    // "acro" is a substring of "macro" — same class of bug.
    let src = "{% macro acro() %}{% endmacro %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "acro").expect("macro 'acro' must appear in symbols");
    let acro_byte = src.find(" acro(").map(|p| p + 1).unwrap() as u32;
    assert_eq!(
        s.selection_range.start_col, acro_byte,
        "selection_range must point to the macro name, not inside the keyword; got col {}",
        s.selection_range.start_col
    );
}

// ─── jinja-lsp-wbsh: name that is a PREFIX of its keyword ────────────────────

#[test]
fn sym_wbsh_macro_name_prefix_of_keyword_selects_name_not_keyword() {
    // "ma" is a prefix of "macro" — without the trailing guard, name_span_in
    // would stop at the "ma" inside "macro" rather than the actual name.
    let src = "{% macro ma() %}{% endmacro %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "ma").expect("macro 'ma' must appear in symbols");
    let ma_byte = src.find(" ma(").map(|p| p + 1).unwrap() as u32;
    assert_eq!(
        s.selection_range.start_col, ma_byte,
        "selection_range must point to the standalone name, not the prefix inside the keyword; got col {}",
        s.selection_range.start_col
    );
}

#[test]
fn sym_wbsh_block_name_prefix_of_keyword_selects_name_not_keyword() {
    // "bl" is a prefix of "block" — the trailing guard must catch it.
    let src = "{% block bl %}{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "bl").expect("block 'bl' must appear in symbols");
    let bl_byte = src.find(" bl ").map(|p| p + 1).unwrap() as u32;
    assert_eq!(
        s.selection_range.start_col, bl_byte,
        "selection_range must point to the standalone name; got col {}",
        s.selection_range.start_col
    );
}

// ─── REQ-SYM-01: SymbolKind mapping ──────────────────────────────────────────

#[test]
fn sym01_block_maps_to_class() {
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "content").expect("block 'content' must appear");
    assert_eq!(s.kind, SymbolKind::Class, "block → Class");
}

#[test]
fn sym01_macro_maps_to_function_with_param_detail() {
    let src = "{% macro greet(name, title='Dr') %}{% endmacro %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "greet").expect("macro 'greet' must appear");
    assert_eq!(s.kind, SymbolKind::Function, "macro → Function");
    let detail = s.detail.as_deref().unwrap_or("");
    assert!(detail.contains("name"), "detail must include param name");
    assert!(detail.contains("title"), "detail must include param title");
}

#[test]
fn sym01_toplevel_set_maps_to_variable() {
    let src = "{% set page_title = 'Home' %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "page_title").expect("top-level set must appear");
    assert_eq!(s.kind, SymbolKind::Variable, "top-level set → Variable");
}

#[test]
fn sym01_extends_maps_to_module() {
    let src = r#"{% extends "base.html" %}"#;
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "base.html").expect("extends must appear");
    assert_eq!(s.kind, SymbolKind::Module, "extends → Module");
    assert_eq!(s.detail.as_deref(), Some("base.html"), "detail is parent path");
}

#[test]
fn sym01_include_maps_to_module() {
    let src = r#"{% include "sidebar.html" %}"#;
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "sidebar.html").expect("include must appear");
    assert_eq!(s.kind, SymbolKind::Module, "include → Module");
}

#[test]
fn sym01_loop_variable_not_in_outline() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    assert!(find_sym(&syms, "item").is_none(), "loop variable must not appear");
}

#[test]
fn sym01_in_block_set_not_in_outline() {
    let src = "{% block foo %}{% set inner = 1 %}{{ inner }}{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    assert!(find_sym(&syms, "inner").is_none(), "in-block set must not appear");
}

// ─── REQ-SYM-05: import / extends / include shape ────────────────────────────

#[test]
fn sym05_alias_import_shape() {
    let src = r#"{% import "macros.html" as macros %}"#;
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "macros").expect("alias import must appear as 'macros'");
    assert_eq!(s.kind, SymbolKind::Namespace, "alias import → Namespace");
    assert_eq!(s.detail.as_deref(), Some("macros.html"), "detail is source path");
}

#[test]
fn sym05_from_import_named_by_source_path() {
    let src = r#"{% from "macros.html" import post_url %}"#;
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let s = find_sym(&syms, "macros.html").expect("from-import must appear as source path");
    assert_eq!(s.kind, SymbolKind::Namespace, "from-import → Namespace");
}

#[test]
fn sym05_from_import_names_are_not_child_symbols() {
    let src = r#"{% from "macros.html" import post_url, comment_card %}"#;
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    // The imported names must not appear as top-level or child symbols
    let all_names: Vec<&str> = syms
        .iter()
        .flat_map(|s| {
            let mut names = vec![s.name.as_str()];
            names.extend(s.children.iter().map(|c| c.name.as_str()));
            names
        })
        .collect();
    assert!(!all_names.contains(&"post_url"), "from-import imported names must not appear");
    assert!(!all_names.contains(&"comment_card"), "from-import imported names must not appear");
}

// ─── REQ-SYM-02: nesting ─────────────────────────────────────────────────────

#[test]
fn sym02_macro_inside_block_is_child() {
    // Block wraps the macro entirely
    let src = "{% block outer %}{% macro inner() %}{% endmacro %}{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let block = find_sym(&syms, "outer").expect("block 'outer' must appear");
    let child = block
        .children
        .iter()
        .find(|c| c.name == "inner")
        .expect("macro 'inner' must be a child of 'outer'");
    assert_eq!(child.kind, SymbolKind::Function, "nested macro → Function");
}

#[test]
fn sym02_sibling_macros_do_not_nest() {
    let src = "{% macro a() %}{% endmacro %}{% macro b() %}{% endmacro %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let a = find_sym(&syms, "a").expect("macro 'a' must appear");
    let b = find_sym(&syms, "b").expect("macro 'b' must appear");
    assert!(a.children.is_empty(), "'a' must not have children");
    assert!(b.children.is_empty(), "'b' must not have children");
}

#[test]
fn jinja_lsp_lrcm_two_same_named_macros_are_both_top_level_siblings() {
    // jinja-lsp-lrcm: full_tag_span matched by name alone, so it always found the
    // FIRST tag named "dup" — both index entries mapped to the identical span,
    // and build_tree's containment fold nested one duplicate inside the other
    // instead of keeping them as two sibling top-level symbols.
    let src = "{% macro dup() %}a{% endmacro %}{% macro dup() %}b{% endmacro %}";
    let idx = extract(src);
    assert_eq!(idx.macros.len(), 2, "extractor must index both occurrences: {:?}", idx.macros);
    let syms = document_symbols(src, &idx);
    let dups: Vec<_> = syms.iter().filter(|s| s.name == "dup").collect();
    assert_eq!(dups.len(), 2, "both same-named macros must appear as top-level symbols: {syms:?}");
    assert_ne!(
        dups[0].range.start_byte, dups[1].range.start_byte,
        "the two occurrences must have distinct spans: {syms:?}"
    );
}

#[test]
fn jinja_lsp_lrcm_two_same_named_blocks_are_both_top_level_siblings() {
    let src = "{% block dup %}a{% endblock %}{% block dup %}b{% endblock %}";
    let idx = extract(src);
    assert_eq!(idx.blocks.len(), 2, "extractor must index both occurrences: {:?}", idx.blocks);
    let syms = document_symbols(src, &idx);
    let dups: Vec<_> = syms.iter().filter(|s| s.name == "dup").collect();
    assert_eq!(dups.len(), 2, "both same-named blocks must appear as top-level symbols: {syms:?}");
    assert_ne!(
        dups[0].range.start_byte, dups[1].range.start_byte,
        "the two occurrences must have distinct spans: {syms:?}"
    );
}

#[test]
fn sym02_deeply_nested_block_macro_block() {
    let src = "{% block outer %}{% macro mid() %}{% block inner %}x{% endblock %}{% endmacro %}{% endblock %}";
    let idx = extract(src);
    let syms = document_symbols(src, &idx);
    let outer = find_sym(&syms, "outer").expect("block 'outer' must appear");
    let mid = outer.children.iter().find(|c| c.name == "mid").expect("macro 'mid' must be child of 'outer'");
    let _inner = mid.children.iter().find(|c| c.name == "inner").expect("block 'inner' must be child of 'mid'");
}

// ─── REQ-SYM-03: workspace symbol search ─────────────────────────────────────

#[test]
fn sym03_empty_query_returns_all_macros_and_blocks() {
    let src = "{% macro post_url(post) %}{% endmacro %}{% block content %}{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"post_url"), "empty query must include macros");
    assert!(names.contains(&"content"), "empty query must include blocks");
}

#[test]
fn sym03_toplevel_variable_not_in_workspace_results() {
    let src = "{% set page_title = 'Home' %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"page_title"), "top-level set must not appear in workspace search");
}

#[test]
fn sym03_imports_not_in_workspace_results() {
    let src = r#"{% from "macros.html" import post_url %}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"macros.html"), "from-import must not appear in workspace search");
    assert!(!names.contains(&"post_url"), "imported name must not appear in workspace search");
}

#[test]
fn sym03_workspace_results_have_container_name() {
    let src = "{% macro greet() %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("greetings.html", src);
    let results = workspace_symbols("greet", &ws);
    let r = results.first().expect("must find 'greet'");
    assert_eq!(r.container_name, "greetings.html", "containerName is the template path");
}

// ─── REQ-SYM-04: fuzzy matching ──────────────────────────────────────────────

#[test]
fn sym04_subsequence_match() {
    let src = "{% macro post_url(post) %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("pu", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"post_url"), "'pu' must subsequence-match 'post_url'");
}

#[test]
fn sym04_case_insensitive_match() {
    let src = "{% macro post_url(post) %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("PU", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"post_url"), "case-insensitive 'PU' must match 'post_url'");
}

#[test]
fn sym04_non_subsequence_returns_empty() {
    let src = "{% macro post_url(post) %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("zzz", &ws);
    assert!(results.is_empty(), "'zzz' must return no matches");
}

#[test]
fn sym04_ranking_exact_before_prefix_before_contiguous_before_subsequence() {
    // Names that hit each tier for query "pu":
    //   exact: "pu"
    //   prefix: "pub_notice"
    //   contiguous-substring: "excerpt_pubdate" (contains "pub" but "pu" is contiguous inside)
    //   scattered-subsequence: "post_url" (p…u not contiguous)
    let src = "{% macro pu() %}{% endmacro %}\
               {% macro pub_notice() %}{% endmacro %}\
               {% macro spun() %}{% endmacro %}\
               {% macro post_url() %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("pu", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    let pos_pu = names.iter().position(|&n| n == "pu");
    let pos_pub = names.iter().position(|&n| n == "pub_notice");
    let pos_spun = names.iter().position(|&n| n == "spun");
    let pos_post = names.iter().position(|&n| n == "post_url");
    if let (Some(a), Some(b)) = (pos_pu, pos_pub) {
        assert!(a < b, "exact must rank before prefix");
    }
    if let (Some(b), Some(c)) = (pos_pub, pos_spun) {
        assert!(b < c, "prefix must rank before contiguous-substring");
    }
    if let (Some(c), Some(d)) = (pos_spun, pos_post) {
        assert!(c < d, "contiguous-substring must rank before scattered-subsequence");
    }
}

#[test]
fn sym04_shorter_name_wins_within_same_tier() {
    // Both "pub" and "pub_notice" are prefix matches of "pu"; "pub" is shorter.
    let src = "{% macro pub() %}{% endmacro %}{% macro pub_notice() %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("t.html", src);
    let results = workspace_symbols("pu", &ws);
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    let pos_pub = names.iter().position(|&n| n == "pub");
    let pos_pub_notice = names.iter().position(|&n| n == "pub_notice");
    if let (Some(a), Some(b)) = (pos_pub, pos_pub_notice) {
        assert!(a < b, "shorter name must rank first within same tier");
    }
}


#[test]
fn debug_block_and_macro_spans() {
    let src = "{% block outer %}{% macro inner() %}{% endmacro %}{% endblock %}";
    let idx = extract(src);
    for b in &idx.blocks {
        eprintln!("BLOCK '{}': span bytes {}..{}", b.name, b.span.start_byte, b.span.end_byte);
    }
    for m in &idx.macros {
        eprintln!("MACRO '{}': span bytes {}..{}", m.name, m.span.start_byte, m.span.end_byte);
    }
    let src2 = "{% block foo %}{% set inner = 1 %}{{ inner }}{% endblock %}";
    let idx2 = extract(src2);
    for b in &idx2.blocks {
        eprintln!("BLOCK '{}': span bytes {}..{}", b.name, b.span.start_byte, b.span.end_byte);
    }
    for v in &idx2.variables {
        eprintln!("VAR '{}' scope={:?}: span bytes {}..{}", v.name, v.scope, v.span.start_byte, v.span.end_byte);
    }
}

// ─── REQ-SYM-04: deterministic ordering across files ─────────────────────────

#[test]
fn sym04_same_tier_same_name_len_ordered_by_path_then_name() {
    // Two macros with the same name in different files — final tiebreak must be
    // deterministic (alphabetical path) so golden files and CLI output are stable.
    let macro_src = "{% macro foo() %}{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("b.html", macro_src);
    ws.index_inline("a.html", macro_src);
    let results = workspace_symbols("foo", &ws);
    assert_eq!(results.len(), 2, "both macros must match");
    assert_eq!(results[0].container_name, "a.html", "a.html must come first (alphabetical)");
    assert_eq!(results[1].container_name, "b.html", "b.html must come second");
}
