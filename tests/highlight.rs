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

// ─── imij/9cfn: column unit must be bytes, not char counts ───────────────────

#[test]
fn hl_9cfn_write_highlight_col_matches_byte_offset_after_multibyte() {
    // "{# café #}" is 11 bytes (é = 2 bytes) but 10 chars.
    // "x" in the set tag that follows appears at different byte vs char positions.
    // byte_to_line_col must count bytes so Write and Read spans use the same unit.
    let src = "{# café #}{% set x = 1 %}{{ x }}";
    // Find byte offsets of "x" (first occurrence is in the set tag).
    let write_byte = src.find("x").unwrap() as u32;
    let read_byte = src.rfind("x").unwrap() as u32;
    let idx = extract(src);
    let reg = Registry::load_core();
    let highlights = document_highlight(src, 0, write_byte, &idx, &reg);
    assert!(!highlights.is_empty(), "must find highlights for x after multibyte prefix");
    let write_hl = highlights.iter().find(|h| h.kind == HighlightKind::Write);
    let read_hl = highlights.iter().find(|h| h.kind == HighlightKind::Read);
    if let (Some(w), Some(r)) = (write_hl, read_hl) {
        // Write is from make_span→byte_to_line_col; Read is from extractor byte columns.
        // After the fix both must equal the byte position.
        assert_eq!(w.range.start_col, write_byte, "Write start_col must be byte offset, got {}", w.range.start_col);
        assert_eq!(r.range.start_col, read_byte, "Read start_col must be byte offset, got {}", r.range.start_col);
    }
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

#[test]
fn hl_nvch_block_named_block_write_span_not_on_keyword() {
    // Block named "block" — the Write span must land on the name, not on the "block" keyword
    let src = "{% block block %}body{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on "block" name (col 9, which is after "{% block ")
    let col = 9u32;
    let results = document_highlight(src, 0, col, &idx, &reg);
    let write: Vec<_> = results.iter().filter(|r| r.kind == HighlightKind::Write).collect();
    assert!(!write.is_empty(), "must have a Write highlight");
    assert_eq!(write[0].range.start_col, 9, "Write span must be on the name at col 9, not the keyword at col 3");
}

// ─── eafo: word-boundary guard for set/for keywords ─────────────────────────

#[test]
fn hl_eafo_var_with_set_suffix_in_name_not_write_site() {
    // "reset" ends with "set" — the old `before.ends_with("set")` would wrongly
    // treat `{% set reset = ... %}{{ reset }}` as the write site being "reset"
    // when the cursor is on the read `{{ reset }}`. But here, a variable named
    // "forset" must NOT trigger a false write site because the token before it
    // is "forset" which ends with "set" but is not the keyword "set".
    let src = "{% set x = 1 %}{% set infor = 2 %}{{ x }}{{ infor }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on read of "x" — write span must point to the {% set x … %} binding
    let col = src.find("{{ x }}").unwrap() as u32 + 3;
    let results = document_highlight(src, 0, col, &idx, &reg);
    let write: Vec<_> = results.iter().filter(|r| r.kind == HighlightKind::Write).collect();
    assert_eq!(write.len(), 1, "must find exactly one Write for 'x'");
    // "x" appears at col 7 in "{% set x = 1 %}"
    let x_set_col = src.find("{% set x").unwrap() as u32 + 7;
    assert_eq!(write[0].range.start_col, x_set_col, "write span must be the set binding");
}

#[test]
fn hl_eafo_for_keyword_requires_word_boundary() {
    // "therefor" ends with "for" but is not the keyword — must not match as a
    // for-loop binding. Simple smoke test: a plain set variable named "item"
    // whose name appears after "therefor" in a comment must still resolve correctly.
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on "item" in the read position.
    let col = src.find("{{ item }}").unwrap() as u32 + 3;
    let results = document_highlight(src, 0, col, &idx, &reg);
    let write: Vec<_> = results.iter().filter(|r| r.kind == HighlightKind::Write).collect();
    assert_eq!(write.len(), 1, "for-loop target must produce exactly one Write");
    // "item" appears at col 7 in "{% for item in items %}"
    let for_item_col = src.find("{% for item").unwrap() as u32 + 7;
    assert_eq!(write[0].range.start_col, for_item_col, "write span must be the for binding");
}

#[test]
fn jinja_lsp_kj7z_write_resolves_to_the_cursors_own_loop_not_the_first_same_named_loop() {
    // Two loops bind the same name "x". find_variable_write_span used to text-scan
    // top-to-bottom and always resolve to the FIRST loop's target regardless of
    // which loop's body the cursor is actually inside — highlighting "x" inside
    // the second loop must mark the SECOND loop's target as the Write, not the first.
    let src = "{% for x in a %}{{ x }}{% endfor %}{% for x in b %}{{ x }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();

    let second_read_col = src.rfind("{{ x }}").unwrap() as u32 + 3;
    let results = document_highlight(src, 0, second_read_col, &idx, &reg);
    let write: Vec<_> = results.iter().filter(|r| r.kind == HighlightKind::Write).collect();
    assert_eq!(write.len(), 1, "must find exactly one Write for the cursor's own loop");

    let second_for_x_col = src.rfind("{% for x").unwrap() as u32 + 7;
    assert_eq!(
        write[0].range.start_col, second_for_x_col,
        "write span must be the SECOND loop's target (the cursor's own scope), not the first"
    );
}
