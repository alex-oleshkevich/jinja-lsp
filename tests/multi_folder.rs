// REQ-EXTR-08: each workspace folder gets its own WorkspaceIndex; cross-folder
// extends references stay unresolved.

use std::fs;

use jinja_lsp::server::state::{FolderState, ServerState};
use jinja_lsp::workspace::build_workspace;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!("jinja_lsp_e08_{name}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    tmp
}

#[test]
fn cross_folder_reference_stays_unresolved() {
    // Folder A: child.html extends "shared/parent.html" (which lives in Folder B)
    let folder_a = fixture_dir("a");
    fs::write(folder_a.join("child.html"), r#"{% extends "parent.html" %}"#).unwrap();

    // Folder B: parent.html
    let folder_b = fixture_dir("b");
    fs::write(folder_b.join("parent.html"), "{% block content %}{% endblock %}").unwrap();

    // Build workspace A only — parent.html is NOT included
    let workspace_a = build_workspace(&[&folder_a], &["html"]);

    // Chain stops at child.html because parent.html is in a different folder
    let chain = workspace_a.template_chain("child.html");
    assert_eq!(chain, vec!["child.html"], "cross-folder ref must not be followed: {chain:?}");

    // Verify the reference IS recorded (it exists in the template), just unresolved
    let idx = workspace_a.templates.get("child.html").unwrap();
    assert_eq!(idx.template_refs.len(), 1, "extends ref must be recorded");
}

// ─── REQ-EXTR-08: server-state routing ───────────────────────────────────────

#[test]
fn server_state_routes_update_to_correct_folder() {
    // Folder A (primary) and folder B (extra) have separate WorkspaceIndex.
    // A file updated under B's root must land in B's workspace, not A's.
    let folder_a = fixture_dir("state_a");
    let folder_b = fixture_dir("state_b");

    let mut cfg_b = jinja_lsp::config::JinjaConfig::default();
    cfg_b.extras = vec!["starlette".to_owned()];
    let registry_b = ServerState::build_registry(&cfg_b);

    let mut state = ServerState::with_config(jinja_lsp::config::JinjaConfig::default());
    state.workspace_root = Some(folder_a.to_string_lossy().into_owned());
    state.extra_folders.push(FolderState {
        root: folder_b.clone(),
        workspace: jinja_lsp::workspace::index::WorkspaceIndex::default(),
        config: cfg_b,
        registry: registry_b,
        config_file_path: None,
        generation: 0,
    });

    let key_a = format!("{}/index.html", folder_a.display());
    let key_b = format!("{}/page.html", folder_b.display());

    state.update_file(&key_a, "{{ x }}");
    state.update_file(&key_b, "{{ request.url }}");

    // File A must be in the primary workspace
    assert!(state.workspace.templates.contains_key(&key_a), "key_a must be in primary workspace");
    assert!(!state.extra_folders[0].workspace.templates.contains_key(&key_a), "key_a must NOT be in extra folder");

    // File B must be in the extra folder's workspace
    assert!(state.extra_folders[0].workspace.templates.contains_key(&key_b), "key_b must be in extra folder");
    assert!(!state.workspace.templates.contains_key(&key_b), "key_b must NOT be in primary workspace");
}

#[test]
fn server_state_workspace_for_routes_by_prefix() {
    let folder_a = fixture_dir("ws_a");
    let folder_b = fixture_dir("ws_b");

    let mut state = ServerState::with_config(jinja_lsp::config::JinjaConfig::default());
    state.workspace_root = Some(folder_a.to_string_lossy().into_owned());
    state.extra_folders.push(FolderState {
        root: folder_b.clone(),
        workspace: jinja_lsp::workspace::index::WorkspaceIndex::default(),
        config: jinja_lsp::config::JinjaConfig::default(),
        registry: ServerState::build_registry(&jinja_lsp::config::JinjaConfig::default()),
        config_file_path: None,
        generation: 0,
    });

    let key_a = format!("{}/t.html", folder_a.display());
    let key_b = format!("{}/t.html", folder_b.display());

    // workspace_for must return the extra folder for key_b
    let ws_a = state.workspace_for(&key_a) as *const _;
    let ws_b = state.workspace_for(&key_b) as *const _;
    let primary = &state.workspace as *const _;
    let extra = &state.extra_folders[0].workspace as *const _;

    assert_eq!(ws_a, primary, "key_a must route to primary workspace");
    assert_eq!(ws_b, extra, "key_b must route to extra folder workspace");
}

#[test]
fn each_folder_has_independent_chain() {
    let folder_a = fixture_dir("ind_a");
    let folder_b = fixture_dir("ind_b");

    fs::write(folder_a.join("child.html"), r#"{% extends "base.html" %}"#).unwrap();
    fs::write(folder_a.join("base.html"), "").unwrap();

    fs::write(folder_b.join("page.html"), r#"{% extends "layout.html" %}"#).unwrap();
    fs::write(folder_b.join("layout.html"), "").unwrap();

    let ws_a = build_workspace(&[&folder_a], &["html"]);
    let ws_b = build_workspace(&[&folder_b], &["html"]);

    assert_eq!(ws_a.template_chain("child.html"), vec!["child.html", "base.html"]);
    assert_eq!(ws_b.template_chain("page.html"), vec!["page.html", "layout.html"]);

    // Each workspace is isolated — templates from the other don't appear
    assert!(!ws_a.templates.contains_key("layout.html"), "B's template must not be in A");
    assert!(!ws_b.templates.contains_key("base.html"), "A's template must not be in B");
}
