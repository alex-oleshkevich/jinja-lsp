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
    assert_eq!(sorted[1].start_line, 3, "closer after line 2 (becomes line 3)");
}
