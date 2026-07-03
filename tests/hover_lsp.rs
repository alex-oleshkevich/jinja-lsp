// F06 — hover LSP handler wiring (jinja-lsp-o7hp).

use jinja_lsp::features::hover::hover;
use jinja_lsp::server::state::ServerState;

// ─── Wiring contract ──────────────────────────────────────────────────────────

#[test]
fn hover_handler_delegates_to_feature_function() {
    let src = include_str!("../src/server/mod.rs");
    // Check for the import of the hover feature function, not just the method name.
    assert!(
        src.contains("features::hover"),
        "server mod must import features::hover to wire the handler"
    );
}

#[test]
fn hover_handler_uses_markup_content_markdown() {
    // REQ-HOV-07: result must be MarkupContent with kind Markdown.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("Markdown"),
        "hover handler must emit MarkupContent with MarkupKind::Markdown (REQ-HOV-07)"
    );
}

// ─── Integration: state chain ─────────────────────────────────────────────────

#[test]
fn hover_returns_doc_for_builtin_filter() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{{ name | upper }}");

    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    // Cursor on "upper" (col 11)
    let result = hover(source, 0, 11, index, &state.registry, &state.workspace);
    assert!(
        result.is_some(),
        "hover must return Some for builtin filter 'upper'"
    );
    let h = result.unwrap();
    assert!(
        h.markdown.contains("upper"),
        "hover markdown must mention the filter name"
    );
}

#[test]
fn hover_returns_none_outside_jinja() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "plain text here");

    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    let result = hover(source, 0, 5, index, &state.registry, &state.workspace);
    assert!(
        result.is_none(),
        "hover must return None outside Jinja delimiters"
    );
}
