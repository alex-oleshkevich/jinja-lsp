// F10 — document_symbol LSP handler wiring (jinja-lsp-i9vf).

use jinja_lsp::features::symbols::{document_symbols, SymbolKind};
use jinja_lsp::server::state::ServerState;

// ─── Wiring contract: handler must delegate to feature function ───────────────

#[test]
fn document_symbol_handler_delegates_to_feature_function() {
    // Static contract: the handler body must call document_symbols().
    // This fails while the handler returns Ok(None) and passes after wiring.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("document_symbols("),
        "document_symbol handler must call features::symbols::document_symbols()"
    );
}

// ─── Integration: state populates source+index, feature returns symbols ───────

#[test]
fn state_chain_produces_symbols_for_block_template() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% block main %}body{% endblock %}");

    let source = state.sources.get("t.html").expect("source in state");
    let index = state.workspace.templates.get("t.html").expect("index in state");
    let syms = document_symbols(source, index);

    assert!(!syms.is_empty(), "document_symbols must produce symbols for a block template");
    assert_eq!(syms[0].name, "main");
    assert_eq!(syms[0].kind, SymbolKind::Class);
}

#[test]
fn state_chain_produces_symbols_for_macro_template() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% macro render_card(title) %}{{ title }}{% endmacro %}");

    let source = state.sources.get("t.html").expect("source in state");
    let index = state.workspace.templates.get("t.html").expect("index in state");
    let syms = document_symbols(source, index);

    let m = syms.iter().find(|s| s.name == "render_card").expect("macro must appear");
    assert_eq!(m.kind, SymbolKind::Function, "macro → Function");
    let detail = m.detail.as_deref().unwrap_or("");
    assert!(detail.contains("title"), "detail must list macro param");
}

#[test]
fn empty_template_produces_no_symbols() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "plain text, no jinja tags");

    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    let syms = document_symbols(source, index);
    assert!(syms.is_empty(), "plain template must produce no symbols");
}
