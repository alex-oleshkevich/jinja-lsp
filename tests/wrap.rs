// REQ-ACT-08: Wrap selection in block, if, or for.

use jinja_lsp::features::wrap::{selection_is_well_formed, wrap_selection, WrapKind};

fn single_edit(source: &str, start: u32, end: u32, kind: WrapKind) -> jinja_lsp::edit::TextEdit {
    let we = wrap_selection(source, "/tpl.html", start, end, kind)
        .expect("wrap_selection returned None");
    let mut edits = we.changes.into_values().next().expect("no changes");
    assert_eq!(edits.len(), 1, "expected exactly 1 edit");
    edits.remove(0)
}

// ─── T-01: wrap single-line selection in if ───────────────────────────────────

#[test]
fn act08_t01_wrap_in_if() {
    let e = single_edit("<p>hello</p>", 0, 0, WrapKind::If);
    assert!(e.new_text.contains("{% if condition %}"), "missing open tag");
    assert!(e.new_text.contains("{% endif %}"), "missing close tag");
    assert!(e.new_text.contains("  <p>hello</p>"), "body must be indented one level");
}

// ─── T-02: wrap selection in for ─────────────────────────────────────────────

#[test]
fn act08_t02_wrap_in_for() {
    let e = single_edit("<li>{{ item }}</li>", 0, 0, WrapKind::For);
    assert!(e.new_text.contains("{% for item in items %}"), "missing open tag");
    assert!(e.new_text.contains("{% endfor %}"), "missing close tag");
    assert!(e.new_text.contains("  <li>{{ item }}</li>"), "body must be indented one level");
}

// ─── T-03: wrap selection in block ───────────────────────────────────────────

#[test]
fn act08_t03_wrap_in_block_with_placeholder() {
    let e = single_edit("<main>content</main>", 0, 0, WrapKind::Block("main_block".to_owned()));
    assert!(e.new_text.contains("{% block main_block %}"), "missing block open tag");
    assert!(e.new_text.contains("{% endblock %}"), "missing endblock tag");
    assert!(e.new_text.contains("  <main>content</main>"), "body must be indented one level");
}

// ─── T-04: multi-line selection re-indents all body lines ────────────────────

#[test]
fn act08_t04_multi_line_wrap() {
    let source = "line1\nline2\nline3";
    let e = single_edit(source, 0, 2, WrapKind::If);
    // Edit covers the entire [0:0 .. 2:end] range.
    assert_eq!(e.start_line, 0);
    assert_eq!(e.start_col, 0);
    assert_eq!(e.end_line, 2);
    assert_eq!(e.end_col, "line3".len() as u32);
    // All body lines must be indented.
    let expected = "{% if condition %}\n  line1\n  line2\n  line3\n{% endif %}";
    assert_eq!(e.new_text, expected);
}

// ─── T-05: middle-of-file selection — surrounding lines are not touched ───────

#[test]
fn act08_t05_middle_of_file_wrap() {
    let source = "header\n<p>one</p>\n<p>two</p>\nfooter";
    // Wrap lines 1-2; lines 0 ("header") and 3 ("footer") must be untouched.
    let e = single_edit(source, 1, 2, WrapKind::If);
    assert_eq!(e.start_line, 1, "edit starts at line 1");
    assert_eq!(e.start_col, 0);
    assert_eq!(e.end_line, 2, "edit ends at line 2");
    assert_eq!(e.end_col, "<p>two</p>".len() as u32, "edit ends at end-of-line col");
    let expected = "{% if condition %}\n  <p>one</p>\n  <p>two</p>\n{% endif %}";
    assert_eq!(e.new_text, expected);
}

// ─── T-21: REQ-ACT-08 — body re-indented one level (T-22/T-23 spec rows) ─────

#[test]
fn act08_t21_body_re_indented_one_level() {
    // A body line already indented 2 spaces gets 2 more → 4 spaces total.
    let source = "  <p>nested</p>";
    let e = single_edit(source, 0, 0, WrapKind::If);
    assert!(e.new_text.contains("    <p>nested</p>"), "pre-existing indent preserved + one level added");
}

// ─── T-22: empty lines within the body are not padded ────────────────────────

#[test]
fn act08_t22_empty_lines_not_padded() {
    let source = "a\n\nb";
    let e = single_edit(source, 0, 2, WrapKind::If);
    // The middle empty line must remain empty (no trailing spaces added).
    let lines: Vec<&str> = e.new_text.split('\n').collect();
    assert_eq!(lines[2], "", "empty line must stay empty");
}

// ─── selection_is_well_formed unit tests (T-24 spec row guard) ───────────────

#[test]
fn well_formed_plain_text_ok() {
    assert!(selection_is_well_formed("<p>hello</p>", 0, 0));
}

#[test]
fn well_formed_balanced_statement_ok() {
    assert!(selection_is_well_formed("{% if x %}\nhello\n{% endif %}", 0, 2));
}

#[test]
fn well_formed_unbalanced_open_rejected() {
    // `{%` with no matching `%}`
    assert!(!selection_is_well_formed("{% if x", 0, 0));
}

#[test]
fn well_formed_unbalanced_close_rejected() {
    // `%}` with no matching `{%`
    assert!(!selection_is_well_formed("hello%}", 0, 0));
}

#[test]
fn well_formed_expression_balanced_ok() {
    assert!(selection_is_well_formed("{{ x }}", 0, 0));
}

#[test]
fn well_formed_expression_split_rejected() {
    assert!(!selection_is_well_formed("{{ x", 0, 0));
}
