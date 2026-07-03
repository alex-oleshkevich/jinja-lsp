// F08 — goto_definition LSP handler wiring (jinja-lsp-lcun).

use jinja_lsp::features::definition::go_to_definition;
use jinja_lsp::server::state::ServerState;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Wiring contract: handler must delegate to feature function ───────────────

#[test]
fn goto_definition_handler_delegates_to_feature_function() {
    // Static contract: the handler body must call go_to_definition().
    // This fails while the handler returns Ok(None) and passes after wiring.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("go_to_definition("),
        "goto_definition handler must call features::definition::go_to_definition()"
    );
}

// ─── Integration: state populates source+index, feature resolves template refs ─

#[test]
fn go_to_definition_resolves_extends_in_workspace() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("base.html", "{% block content %}{% endblock %}");
    state.update_file("child.html", r#"{% extends "base.html" %}"#);

    let source = state.sources.get("child.html").unwrap();
    let index = state.workspace.templates.get("child.html").unwrap();

    // Cursor on the "base.html" string literal (line 0, col ~12, inside the string)
    let result = go_to_definition(
        source,
        0,
        13,
        "child.html",
        index,
        &state.registry,
        &state.workspace,
    );
    assert!(
        result.is_some(),
        "go_to_definition must resolve 'base.html' extends reference"
    );
    let loc = result.unwrap();
    assert_eq!(loc.target_path, "base.html", "target must be base.html");
    assert_eq!(loc.target_start_line, 0);
}

#[test]
fn go_to_definition_returns_none_for_unresolvable_path() {
    let mut state = ServerState::with_config(Default::default());
    state.update_file("child.html", r#"{% extends "ghost.html" %}"#);

    let source = state.sources.get("child.html").unwrap();
    let index = state.workspace.templates.get("child.html").unwrap();
    let ws = WorkspaceIndex::default(); // ghost.html not in workspace

    let result = go_to_definition(source, 0, 13, "child.html", index, &state.registry, &ws);
    assert!(
        result.is_none(),
        "must return None for path not in workspace"
    );
}
