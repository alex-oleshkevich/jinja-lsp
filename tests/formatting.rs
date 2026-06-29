// REQ-FMT-07: formatting / rangeFormatting feature unit tests.

use jinja_lsp::features::formatting::{format_document, format_range};

// ─── T-01: whole-document format returns single edit when source changes ──────

#[test]
fn fmt07_t01_format_document_returns_edit_when_changed() {
    let source = "{{x}}";
    let edits = format_document(source);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "{{ x }}");
    assert_eq!(edits[0].start_line, 0);
    assert_eq!(edits[0].start_col, 0);
}

// ─── T-02: already-formatted document returns no edits ────────────────────────

#[test]
fn fmt07_t02_format_document_returns_empty_when_unchanged() {
    let source = "{{ x }}";
    let edits = format_document(source);
    assert!(edits.is_empty());
}

// ─── T-03: range format returns only edits within the range ──────────────────

#[test]
fn fmt07_t03_format_range_returns_edits_in_range() {
    // Line 0: already formatted; line 1: needs formatting.
    let source = "{{ x }}\n{{y}}";
    let edits = format_range(source, 1, 1);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].start_line, 1);
    assert_eq!(edits[0].new_text, "{{ y }}");
}

// ─── T-04: range format excludes edits outside the range ─────────────────────

#[test]
fn fmt07_t04_format_range_excludes_out_of_range() {
    // Line 0 needs formatting; line 1 is fine. Ask for range [1, 1].
    let source = "{{x}}\n{{ y }}";
    let edits = format_range(source, 1, 1);
    assert!(edits.is_empty());
}

// ─── T-05: multi-line range ───────────────────────────────────────────────────

#[test]
fn fmt07_t05_format_range_multi_line() {
    let source = "{{a}}\n{{b}}\n{{c}}";
    // Format lines 0..=1 only (not line 2).
    let edits = format_range(source, 0, 1);
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|e| e.start_line <= 1));
    assert!(edits.iter().any(|e| e.new_text == "{{ a }}"));
    assert!(edits.iter().any(|e| e.new_text == "{{ b }}"));
}
