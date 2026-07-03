// REQ-ACT-07: Extract selection to a macro.

use jinja_lsp::features::extract_macro::compute_extract_macro;

// ─── T-01: single-line extraction ────────────────────────────────────────────

#[test]
fn act07_t01_single_line_extract() {
    let source = "<p>hello</p>\n<p>world</p>";
    let we = compute_extract_macro(source, "/tpl.html", 0, 0, "greeting");
    assert!(we.is_some(), "expected WorkspaceEdit");
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").expect("file edits");

    // Should contain an edit that replaces the selection with {{ greeting() }}
    let has_call = edits.iter().any(|e| e.new_text == "{{ greeting() }}");
    assert!(
        has_call,
        "expected {{ greeting() }} replacement; edits: {:?}",
        edits
    );

    // Should contain an edit that appends the macro definition
    let has_macro = edits.iter().any(|e| {
        e.new_text.contains("{% macro greeting() %}") && e.new_text.contains("{% endmacro %}")
    });
    assert!(
        has_macro,
        "expected macro definition appended; edits: {:?}",
        edits
    );
}

// ─── T-02: multi-line extraction ─────────────────────────────────────────────

#[test]
fn act07_t02_multi_line_extract() {
    let source = "header\n<p>one</p>\n<p>two</p>\nfooter";
    let we = compute_extract_macro(source, "/tpl.html", 1, 2, "body_content");
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").expect("file edits");

    let has_call = edits.iter().any(|e| e.new_text == "{{ body_content() }}");
    assert!(has_call, "expected call replacement; edits: {:?}", edits);

    let has_macro = edits
        .iter()
        .any(|e| e.new_text.contains("{% macro body_content() %}"));
    assert!(has_macro, "expected macro definition; edits: {:?}", edits);
}

// ─── T-03: macro body contains the extracted content ─────────────────────────

#[test]
fn act07_t03_extracted_content_in_macro() {
    let source = "<nav>menu</nav>\n<article>body</article>";
    let we = compute_extract_macro(source, "/tpl.html", 0, 0, "header_nav");
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").expect("file edits");

    let macro_def = edits
        .iter()
        .find(|e| e.new_text.contains("{% macro header_nav() %}"))
        .expect("macro definition edit");
    assert!(
        macro_def.new_text.contains("<nav>menu</nav>"),
        "expected extracted content in macro body; got: {}",
        macro_def.new_text
    );
}

// ─── jinja-lsp-ifrq: invalid ranges must return None, not a corrupt edit ─────

#[test]
fn jinja_lsp_ifrq_inverted_range_returns_none() {
    // The server executes this command with client-supplied line numbers and no
    // other validation — an inverted range must not silently produce an
    // empty-body macro plus an edit addressing nonexistent lines.
    let source = "<p>hello</p>\n<p>world</p>";
    let we = compute_extract_macro(source, "/tpl.html", 1, 0, "greeting");
    assert!(
        we.is_none(),
        "start_line > end_line must return None: {we:?}"
    );
}

#[test]
fn jinja_lsp_ifrq_out_of_bounds_end_line_returns_none() {
    let source = "<p>hello</p>\n<p>world</p>"; // 2 lines: indices 0, 1
    let we = compute_extract_macro(source, "/tpl.html", 0, 5, "greeting");
    assert!(we.is_none(), "end_line beyond EOF must return None: {we:?}");
}

#[test]
fn jinja_lsp_ifrq_out_of_bounds_start_line_returns_none() {
    let source = "<p>hello</p>\n<p>world</p>";
    let we = compute_extract_macro(source, "/tpl.html", 5, 6, "greeting");
    assert!(
        we.is_none(),
        "start_line beyond EOF must return None: {we:?}"
    );
}

#[test]
fn jinja_lsp_ifrq_selection_splitting_a_tag_returns_none() {
    // Reuses selection_is_well_formed (the same check the code-action wrap path
    // gates on): a selection that splits a Jinja delimiter (here, a multi-line
    // {% if ... %} tag cut off before its closing %}) would corrupt the
    // template if extracted as-is.
    let source = "{% if\nx %}\nhello\n{% endif %}";
    let we = compute_extract_macro(source, "/tpl.html", 0, 0, "partial");
    assert!(
        we.is_none(),
        "selection splitting an open {{% tag must return None: {we:?}"
    );
}
