// REQ-FMT-07: formatting / rangeFormatting feature unit tests.

use jinja_lsp::features::formatting::{FormatOptions, format_document, format_range};

fn default_opts() -> FormatOptions {
    FormatOptions::default() // 4 spaces (default indent_size)
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
    let opts = FormatOptions {
        tab_size: 4,
        insert_spaces: true,
    };
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    // The `{% for %}` line should have 4 spaces of indent (depth=1 inside block).
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(
        lines[1].starts_with("    "),
        "for line must have 4-space indent: {:?}",
        lines[1]
    );
}

#[test]
fn fmt07_tabs_indent_nested_block() {
    let source = "{% block content %}\n{% for x in items %}\n{{ x }}\n{% endfor %}\n{% endblock %}";
    let opts = FormatOptions {
        tab_size: 1,
        insert_spaces: false,
    };
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(
        lines[1].starts_with('\t'),
        "for line must start with tab: {:?}",
        lines[1]
    );
}

// ─── jinja-lsp-7ym9: range snap outward to whole Jinja constructs ────────────

#[test]
fn fmt7ym9_range_snaps_outward_to_enclosing_opener() {
    // Select only the body line (line 1) of a {% if %} block.
    // format_range must snap start_line up to line 0 (the opening tag).
    let source = "{% if condition %}\n{{x}}\n{% endif %}";
    let opts = FormatOptions::default();
    let edits = format_range(source, 1, 1, opts);
    // The formatter normalizes {{x}} to {{ x }}. The edit for line 1 must appear.
    assert!(
        edits
            .iter()
            .any(|e| e.start_line == 1 && e.new_text.contains("{{ x }}")),
        "body line must be formatted: {:?}",
        edits
    );
}

#[test]
fn fmt7ym9_range_unchanged_when_no_snap_needed() {
    // If the range already covers whole tags, no expansion happens and we get only
    // the edits within that range (not outside it).
    let source = "{{ a }}\n{{b}}\n{{ c }}";
    let opts = FormatOptions::default();
    // Select only line 1 — there are no Jinja tags that need snapping.
    let edits = format_range(source, 1, 1, opts);
    assert!(
        edits.iter().all(|e| e.start_line >= 1 && e.end_line <= 1),
        "edits must stay within [1,1] when no snap is needed: {:?}",
        edits
    );
}

#[test]
fn jinja_lsp_tjr3_snap_does_not_over_expand_past_an_already_closed_multiline_tag() {
    // The multi-line {% if ... %} tag on lines 0-1 is already closed before the
    // selected line 2 — snapping must NOT expand upward into it. The dead
    // `depth > 0` check (always true right after `depth += 1`) used to latch onto
    // the nearest opener-only line regardless of whether an intervening
    // closer-only line had already resolved it.
    let source = "{%if c\n   and d %}\n{{x}}";
    let opts = default_opts();
    let edits = format_range(source, 2, 2, opts);
    assert!(
        edits.iter().all(|e| e.start_line >= 2),
        "must not touch the already-closed opener line 0 when only line 2 is selected: {edits:?}"
    );
    // The actually-selected line must still be formatted.
    assert!(
        edits
            .iter()
            .any(|e| e.start_line == 2 && e.new_text.contains("{{ x }}"))
    );
}

#[test]
fn jinja_lsp_tjr3_snap_end_does_not_over_expand_past_a_construct_that_opens_and_closes_after() {
    // Symmetric case: a new self-contained {% if ... %} construct starts and
    // fully closes AFTER end_line — snapping the end forward must not latch onto
    // its closer-only continuation line.
    let source = "{{x}}\n{%if c\n   and d %}";
    let opts = default_opts();
    let edits = format_range(source, 0, 0, opts);
    assert!(
        edits.iter().all(|e| e.end_line == 0),
        "must not touch the later, unrelated multi-line tag when only line 0 is selected: {edits:?}"
    );
    assert!(
        edits
            .iter()
            .any(|e| e.start_line == 0 && e.new_text.contains("{{ x }}"))
    );
}

#[test]
fn fmt07_default_options_produces_4space_indent() {
    let source = "{% block content %}\n{% for x in items %}\n{% endfor %}\n{% endblock %}";
    let opts = FormatOptions::default();
    let formatted = jinja_lsp::format::format_with_options(source, opts);
    let lines: Vec<&str> = formatted.split('\n').collect();
    assert!(
        lines[1].starts_with("    "),
        "default must produce 4-space indent: {:?}",
        lines[1]
    );
}
