// REQ-ACT-11: rename via code-action, NOT via textDocument/rename LSP method.
// Constitution §4.7 / F17 §2: rename is a Non-Goal as an LSP method;
// it is delivered as a code-action command instead.

use jinja_lsp::features::rename::{
    RenameTarget, check_rename_preconditions, compute_rename, rename_at_cursor,
};
use jinja_lsp::server::state::ServerState;

// ─── Architecture conformance: LSP method must NOT be advertised ─────────────

#[test]
fn rename_lsp_method_not_advertised_in_capabilities() {
    // Constitution §4.7: textDocument/rename is a Non-Goal.
    // The server must not declare rename_provider — clients must not call the method.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        !src.contains("rename_provider"),
        "rename_provider must NOT appear in server capabilities (Non-Goal per constitution §4.7)"
    );
}

#[test]
fn prepare_rename_handler_not_present() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        !src.contains("async fn prepare_rename"),
        "prepare_rename handler must NOT be present (Non-Goal per constitution §4.7)"
    );
}

// ─── Integration: feature functions still work (used by code-action path) ────

#[test]
fn rename_local_variable_via_state() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% set count = 1 %}{{ count }}");
    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();

    let result = rename_at_cursor(source, "t.html", 0, 22, index, &state.workspace);
    assert!(result.is_some(), "rename must be offered on local variable");
    let (target, name) = result.unwrap();
    assert_eq!(name, "count");
    assert!(matches!(target, RenameTarget::Local { .. }));

    let edit = compute_rename(
        &state.sources,
        "t.html",
        "count",
        "total",
        target,
        index,
        &state.workspace,
    );
    assert!(edit.is_some(), "compute_rename must return edits");
    let edits = edit
        .unwrap()
        .changes
        .get("t.html")
        .cloned()
        .unwrap_or_default();
    assert!(!edits.is_empty(), "must produce at least one rename edit");
    assert!(
        edits.iter().all(|e| e.new_text == "total"),
        "all edits must replace with 'total'"
    );
}

#[test]
fn rename_block_via_state() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% block content %}body{% endblock %}");
    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();

    let col = source.find("content").unwrap() as u32;
    let result = rename_at_cursor(source, "t.html", 0, col, index, &state.workspace);
    assert!(result.is_some(), "rename must be offered on block name");
    let (target, name) = result.unwrap();
    assert_eq!(name, "content");
    assert_eq!(target, RenameTarget::Workspace);
}

// ─── jinja-lsp-8vf4: validation integration via state ────────────────────────

#[test]
fn rename_invalid_identifier_refused_via_preconditions() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% set count = 1 %}{{ count }}");
    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();
    let (target, _) = rename_at_cursor(source, "t.html", 0, 22, index, &state.workspace).unwrap();
    assert!(check_rename_preconditions("123abc", &target, index).is_some());
    assert!(check_rename_preconditions("total", &target, index).is_none());
}

#[test]
fn rename_command_advertised_in_server() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("jinja-lsp.rename"),
        "rename command must be registered in execute_command_provider"
    );
}
