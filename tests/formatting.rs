// REQ-FMT-07: formatting / rangeFormatting feature unit tests.

use jinja_lsp::features::formatting::{format_document, format_range, FormatOptions};

fn default_opts() -> FormatOptions {
    FormatOptions::default() // 2 spaces
}

// ─── T-01: whole-document format returns single edit when source changes ──────

#[test]
fn fmt07_t01_format_document_returns_edit_when_changed() {
    let source = "{{x}}";
    let edits = format_document(source, default_opts());
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "{{ x }}");
    assert_eq!(edits[0].start_line, 0);
    assert_eq!(edits[0].start_col, 0);
}

// ─── T-02: already-formatted document returns no edits ────────────────────────

#[test]
fn fmt07_t02_format_document_returns_empty_when_unchanged() {
    let source = "{{ x }}";
    let edits = format_document(source, default_opts());
    assert!(edits.is_empty());
}

// ─── T-03: range format returns only edits within the range ──────────────────

#[test]
fn fmt07_t03_format_range_returns_edits_in_range() {
    // Line 0: already formatted; line 1: needs formatting.
    let source = "{{ x }}\n{{y}}";
    let edits = format_range(source, 1, 1, default_opts());
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].start_line, 1);
    assert_eq!(edits[0].new_text, "{{ y }}");
}

// ─── T-04: range format excludes edits outside the range ─────────────────────

#[test]
fn fmt07_t04_format_range_excludes_out_of_range() {
    // Line 0 needs formatting; line 1 is fine. Ask for range [1, 1].
    let source = "{{x}}\n{{ y }}";
    let edits = format_range(source, 1, 1, default_opts());
    assert!(edits.is_empty());
}

// ─── T-05: multi-line range ───────────────────────────────────────────────────

#[test]
fn fmt07_t05_format_range_multi_line() {
    let source = "{{a}}\n{{b}}\n{{c}}";
    // Format lines 0..=1 only (not line 2).
    let edits = format_range(source, 0, 1, default_opts());
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|e| e.start_line <= 1));
    assert!(edits.iter().any(|e| e.new_text == "{{ a }}"));
    assert!(edits.iter().any(|e| e.new_text == "{{ b }}"));
}

// ─── REQ-FMT-07: tab_size and insert_spaces options ──────────────────────────

#[test]
fn fmt07_4spaces_indents_nested_block() {
    // A nested for inside block should indent with 4 spaces when tab_size=4.
    let source = "{% block content %}\n{% for x in items %}\n{{ x }}\n{% endfor %}\n{% endblock %}";
    let opts = FormatOptions { tab_size: 4, insert_spaces: true };
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    // The `{% for %}` line should have 4 spaces of indent (depth=1 inside block).
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(lines[1].starts_with("    "), "for line must have 4-space indent: {:?}", lines[1]);
}

#[test]
fn fmt07_tabs_indent_nested_block() {
    let source = "{% block content %}\n{% for x in items %}\n{{ x }}\n{% endfor %}\n{% endblock %}";
    let opts = FormatOptions { tab_size: 1, insert_spaces: false };
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(lines[1].starts_with('\t'), "for line must start with tab: {:?}", lines[1]);
}

#[test]
fn fmt07_default_options_produces_2space_indent() {
    let source = "{% block content %}\n{% for x in items %}\n{% endfor %}\n{% endblock %}";
    let opts = FormatOptions::default();
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(lines[1].starts_with("  "), "default must produce 2-space indent: {:?}", lines[1]);
    assert!(!lines[1].starts_with("    "), "default must NOT produce 4-space indent: {:?}", lines[1]);
}
