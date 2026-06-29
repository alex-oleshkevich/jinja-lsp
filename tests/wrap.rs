// REQ-ACT-08: Wrap selection in block, if, or for.

use jinja_lsp::features::wrap::{wrap_selection, WrapKind};

// ─── T-01: wrap selection in if ──────────────────────────────────────────────

#[test]
fn act08_t01_wrap_in_if() {
    let source = "<p>hello</p>";
    let we = wrap_selection(source, "/tpl.html", 0, 0, WrapKind::If);
    assert!(we.is_some(), "expected a WorkspaceEdit");
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").unwrap();
    // Should insert {% if condition %} before and {% endif %} after
    let new_text: Vec<&str> = edits.iter().map(|e| e.new_text.as_str()).collect();
    assert!(new_text.iter().any(|t| t.contains("{% if")), "expected if tag");
    assert!(new_text.iter().any(|t| t.contains("{% endif %}")), "expected endif tag");
}

// ─── T-02: wrap selection in for ─────────────────────────────────────────────

#[test]
fn act08_t02_wrap_in_for() {
    let source = "<li>{{ item }}</li>";
    let we = wrap_selection(source, "/tpl.html", 0, 0, WrapKind::For);
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").unwrap();
    let new_text: Vec<&str> = edits.iter().map(|e| e.new_text.as_str()).collect();
    assert!(new_text.iter().any(|t| t.contains("{% for")), "expected for tag");
    assert!(new_text.iter().any(|t| t.contains("{% endfor %}")), "expected endfor tag");
}

// ─── T-03: wrap selection in block ───────────────────────────────────────────

#[test]
fn act08_t03_wrap_in_block_with_placeholder() {
    let source = "<main>content</main>";
    let we = wrap_selection(source, "/tpl.html", 0, 0, WrapKind::Block("main_block".to_owned()));
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").unwrap();
    let new_text: Vec<&str> = edits.iter().map(|e| e.new_text.as_str()).collect();
    assert!(new_text.iter().any(|t| t.contains("{% block main_block %}")), "expected block tag");
    assert!(new_text.iter().any(|t| t.contains("{% endblock %}")), "expected endblock tag");
}

// ─── T-04: multi-line selection wraps whole range ────────────────────────────

#[test]
fn act08_t04_multi_line_wrap() {
    let source = "line1\nline2\nline3";
    let we = wrap_selection(source, "/tpl.html", 0, 2, WrapKind::If);
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").unwrap();
    // One insertion before line 0, one after line 2
    assert_eq!(edits.len(), 2, "expected exactly 2 edits (open+close)");
    let sorted: Vec<_> = {
        let mut v = edits.clone();
        v.sort_by_key(|e| e.start_line);
        v
    };
    assert_eq!(sorted[0].start_line, 0, "opener at line 0");
    // close_edit anchors at the END of end_line (col = len("line3")) so adjacent
    // file content is not disturbed — start_line is end_line, not end_line+1.
    assert_eq!(sorted[1].start_line, 2, "closer anchored at end of line 2");
    assert!(sorted[1].new_text.contains("{% endif %}"), "closer contains endif");
}

// ─── T-05: middle-of-file selection — closer must not merge with next line ───

#[test]
fn act08_t05_middle_of_file_wrap() {
    let source = "header\n<p>one</p>\n<p>two</p>\nfooter";
    // Wrap lines 1-2 (the two <p> lines); line 3 "footer" must stay below.
    let we = wrap_selection(source, "/tpl.html", 1, 2, WrapKind::If);
    assert!(we.is_some());
    let we = we.unwrap();
    let edits = we.changes.get("/tpl.html").unwrap();
    assert_eq!(edits.len(), 2);

    // The close edit must be anchored at the END of line 2, not the START of line 3.
    let close = edits.iter().max_by_key(|e| e.start_line).unwrap();
    assert_eq!(close.start_line, 2, "close anchored at line 2");
    // col must be at the end of "<p>two</p>" (10 chars)
    assert_eq!(close.start_col, "<p>two</p>".len() as u32, "close at end-of-line col");
    // new_text must start with \n so the tag gets its own line
    assert!(close.new_text.starts_with('\n'), "close_tag preceded by newline");
    assert!(close.new_text.contains("{% endif %}"), "close contains endif");
}
