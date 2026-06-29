// F11 — Document highlight tests: REQ-HL-01 through REQ-HL-04.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::document_highlight::{document_highlight, HighlightKind};
use jinja_lsp::parsing::extract;

// ─── REQ-HL-01: which symbols highlight ──────────────────────────────────────

#[test]
fn hl01_loop_variable_highlights_write_and_reads() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on the for-target "item"
    let col = src.find("item").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    assert!(!highlights.is_empty(), "must find highlights for loop variable");
    let write_count = highlights.iter().filter(|h| h.kind == HighlightKind::Write).count();
    let read_count = highlights.iter().filter(|h| h.kind == HighlightKind::Read).count();
    assert_eq!(write_count, 1, "for-loop target must be Write");
    assert!(read_count >= 1, "usages inside the loop must be Read");
}

#[test]
fn hl01_macro_highlights_write_and_reads() {
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet() }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on macro definition name "greet"
    let col = src.find("greet").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    let write = highlights.iter().find(|h| h.kind == HighlightKind::Write);
    let read = highlights.iter().find(|h| h.kind == HighlightKind::Read);
    assert!(write.is_some(), "macro definition must be Write");
    assert!(read.is_some(), "macro call must be Read");
}

#[test]
fn hl01_block_highlights_write() {
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("content").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    let write = highlights.iter().find(|h| h.kind == HighlightKind::Write);
    assert!(write.is_some(), "block definition must appear as Write");
}

#[test]
fn hl01_import_alias_highlights_write_and_reads() {
    let src = r#"{% import "macros.html" as m %}{{ m.greet() }}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on the alias "m" (in "as m")
    let col = src.find(" as m").map(|p| p + 4).unwrap() as u32; // position of 'm'
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    let write_count = highlights.iter().filter(|h| h.kind == HighlightKind::Write).count();
    assert_eq!(write_count, 1, "import alias binding must be Write");
}

// ─── REQ-HL-02: file-local, scope-aware ──────────────────────────────────────

#[test]
fn hl02_imported_macro_no_definition_in_file() {
    // post_url is defined in macros.html, not here → only reads in this file
    let src = r#"{% from "macros.html" import post_url %}{{ post_url() }}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.rfind("post_url").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    assert!(!highlights.is_empty(), "must find the call-site reference");
    let write_count = highlights.iter().filter(|h| h.kind == HighlightKind::Write).count();
    assert_eq!(write_count, 0, "no Write because definition is in another file");
}

// ─── REQ-HL-03: write vs read kinds ──────────────────────────────────────────

#[test]
fn hl03_set_target_is_write_usage_is_read() {
    let src = "{% set page_title = 'Home' %}{{ page_title }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on the set target "page_title"
    let col = src.find("page_title").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    let write_count = highlights.iter().filter(|h| h.kind == HighlightKind::Write).count();
    let read_count = highlights.iter().filter(|h| h.kind == HighlightKind::Read).count();
    assert!(write_count >= 1, "set target is Write");
    assert!(read_count >= 1, "usage is Read");
}

#[test]
fn hl03_macro_call_is_read_write_at_definition() {
    let src = "{% macro m() %}{% endmacro %}{{ m() }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on the CALL site "m()"
    let col = src.rfind("m()").unwrap() as u32;
    let highlights = document_highlight(src, 0, col, &idx, &reg);
    let write_count = highlights.iter().filter(|h| h.kind == HighlightKind::Write).count();
    let read_count = highlights.iter().filter(|h| h.kind == HighlightKind::Read).count();
    assert!(write_count >= 1, "macro definition is Write");
    assert!(read_count >= 1, "call site is Read");
}

// ─── Safety: non-char-boundary byte must not panic ───────────────────────────

#[test]
fn hl_multibyte_mid_char_does_not_panic() {
    // é is 2 bytes (0xC3 0xA9); byte 1 is NOT a char boundary.
    let src = "{{ é }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Passing byte=3 (mid-char) must not panic — returns empty result.
    let _results = document_highlight(src, 0, 3, &idx, &reg);
}

// ─── REQ-HL-04: non-symbol positions return empty ────────────────────────────

#[test]
fn hl04_html_text_returns_empty() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let results = document_highlight(src, 0, 5, &idx, &reg);
    assert!(results.is_empty(), "HTML text must return empty");
}

#[test]
fn hl04_delimiter_returns_empty() {
    // Cursor on the opening `{` of `{{`
    let src = "{{ item }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let results = document_highlight(src, 0, 0, &idx, &reg);
    assert!(results.is_empty(), "delimiter must return empty");
}

#[test]
fn hl04_builtin_filter_returns_empty() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("truncate").unwrap() as u32;
    let results = document_highlight(src, 0, col, &idx, &reg);
    assert!(results.is_empty(), "built-in filter must return empty (REQ-HL-04)");
}

#[test]
fn hl04_whitespace_inside_expression_returns_empty() {
    // Cursor on the space inside `{{ item }}`
    let src = "{{ item }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let results = document_highlight(src, 0, 2, &idx, &reg); // position of space after `{{`
    assert!(results.is_empty(), "whitespace inside expression must return empty");
}
