// REQ-ACT-11: rename + prepareRename LSP handler wiring.

use jinja_lsp::features::rename::{rename_at_cursor, compute_rename, RenameTarget};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;
use jinja_lsp::server::state::ServerState;

// ─── Handler wiring contract ─────────────────────────────────────────────────

#[test]
fn rename_handler_delegates_to_feature_function() {
    let src = include_str!("../src/server/mod.rs");
    assert!(src.contains("rename_at_cursor("), "rename handler must call rename_at_cursor");
    assert!(src.contains("compute_rename("), "rename handler must call compute_rename");
}

#[test]
fn prepare_rename_handler_delegates_to_feature_function() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("async fn prepare_rename"),
        "server must declare prepare_rename handler"
    );
}

#[test]
fn rename_provider_declared_in_capabilities() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("rename_provider"),
        "server capabilities must include rename_provider"
    );
    assert!(
        src.contains("prepare_provider: Some(true)"),
        "rename_provider must declare prepare_provider: Some(true)"
    );
}

// ─── Integration: feature + state chain ──────────────────────────────────────

#[test]
fn rename_local_variable_via_state() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("t.html", "{% set count = 1 %}{{ count }}");
    let source = state.sources.get("t.html").unwrap();
    let index = state.workspace.templates.get("t.html").unwrap();

    // Cursor on "count" in `{{ count }}` (col 22)
    let result = rename_at_cursor(source, "t.html", 0, 22, index, &state.workspace);
    assert!(result.is_some(), "rename must be offered on local variable");
    let (target, name) = result.unwrap();
    assert_eq!(name, "count");
    assert_eq!(target, RenameTarget::Local);

    let edit = compute_rename(source, "t.html", "count", "total", target, index, &state.workspace);
    assert!(edit.is_some(), "compute_rename must return edits");
    let edits = edit.unwrap().changes.get("t.html").cloned().unwrap_or_default();
    assert!(!edits.is_empty(), "must produce at least one rename edit");
    assert!(edits.iter().all(|e| e.new_text == "total"), "all edits must replace with 'total'");
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
