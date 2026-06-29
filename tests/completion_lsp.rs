// F05 — completion + completionItem/resolve LSP handler wiring (jinja-lsp-3san).

use jinja_lsp::features::completions::{complete, resolve_doc, CompletionKind, TRIGGER_CHARS};
use jinja_lsp::server::state::ServerState;

// ─── Wiring contract: handler must delegate to feature function ───────────────

#[test]
fn completion_handler_delegates_to_feature_function() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("complete("),
        "completion handler must call features::completions::complete()"
    );
}

#[test]
fn completion_resolve_handler_delegates_to_feature_function() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("resolve_doc("),
        "completion_resolve handler must call features::completions::resolve_doc()"
    );
}

#[test]
fn trigger_chars_match_req_cmp_01() {
    // REQ-CMP-01: server must declare trigger chars that match TRIGGER_CHARS.
    let src = include_str!("../src/server/mod.rs");
    // Every trigger char must appear in the CompletionOptions declaration.
    // We check the string literals that name the chars.
    let required_chars = ["{", "%", "(", ",", "|", "."];
    for ch in &required_chars {
        assert!(
            src.contains(&format!("\"{ch}\"")),
            "trigger_characters must include '{ch}' (REQ-CMP-01)"
        );
    }
}

// ─── Integration: state chain + feature function ──────────────────────────────

#[test]
fn complete_returns_filters_in_filter_context() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{{ name | ");

    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    // Cursor after "| " — filter context
    let items = complete(source, 0, source.len() as u32, index, &state.registry, &state.workspace);
    assert!(!items.is_empty(), "filter context must produce completions");
    assert!(items.iter().all(|i| i.kind == CompletionKind::Filter), "all items must be Filter kind");
}

#[test]
fn complete_returns_keywords_in_statement_context() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% ");

    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    let items = complete(source, 0, 3, index, &state.registry, &state.workspace);
    assert!(!items.is_empty(), "statement context must produce keyword completions");
    let has_for = items.iter().any(|i| i.label == "for");
    let has_if = items.iter().any(|i| i.label == "if");
    assert!(has_for, "statement completions must include 'for'");
    assert!(has_if, "statement completions must include 'if'");
}

#[test]
fn resolve_doc_returns_markdown_for_known_filter() {
    let state = ServerState::with_config(Default::default());
    // "upper" is a core Jinja2 filter — always in the registry.
    let doc = resolve_doc("filter:upper", &state.registry);
    assert!(doc.is_some(), "resolve_doc must return Some for known filter 'upper'");
    let doc = doc.unwrap();
    assert!(doc.contains("upper"), "resolved doc must mention the filter name");
}

#[test]
fn trigger_chars_constant_covers_expected_chars() {
    // TRIGGER_CHARS must include the chars declared in REQ-CMP-01.
    for ch in ['{', '%', '(', ',', '|', '.', ' '] {
        assert!(
            TRIGGER_CHARS.contains(&ch),
            "TRIGGER_CHARS must include '{ch}' (REQ-CMP-01)"
        );
    }
}
