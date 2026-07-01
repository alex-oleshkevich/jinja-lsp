use std::path::Path;

use jinja_lsp::workspace::{build_workspace, index::WorkspaceIndex};
use jinja_lsp::parsing::extract;

// ── REQ-DATA-10: template_chain resolves across absolute-path keys ─────────

#[test]
fn cnvx_template_chain_resolves_with_absolute_keys() {
    // Simulates the LSP server: templates are stored under absolute-path keys
    // (uri.path()), but extends targets in Jinja source are relative strings.
    // template_chain must resolve "base.html" → "/abs/templates/base.html".
    let mut ws = WorkspaceIndex::default();
    let abs_base = "/abs/templates/base.html";
    let abs_post = "/abs/templates/blog/post.html";

    let mut base_idx = extract("{% block content %}{% endblock %}");
    base_idx.path = abs_base.to_owned();
    ws.templates.insert(abs_base.to_owned(), base_idx);

    let mut post_idx = extract(r#"{% extends "base.html" %}"#);
    post_idx.path = abs_post.to_owned();
    ws.templates.insert(abs_post.to_owned(), post_idx);

    let chain = ws.template_chain(abs_post);
    assert_eq!(
        chain,
        vec![abs_post, abs_base],
        "absolute-key workspace must follow extends to resolved ancestor: {chain:?}"
    );
}

fn tdir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/templates")
}

#[test]
fn template_chain_follows_extends() {
    let dir = tdir();
    let workspace = build_workspace(&[&dir], &["html"]);

    // blog/post.html extends base.html → chain is [post, base]
    let chain = workspace.template_chain("blog/post.html");
    assert_eq!(chain, vec!["blog/post.html", "base.html"], "chain: {chain:?}");
}

#[test]
fn root_template_chain_has_single_entry() {
    let dir = tdir();
    let workspace = build_workspace(&[&dir], &["html"]);

    // base.html has no extends → chain is just [base]
    let chain = workspace.template_chain("base.html");
    assert_eq!(chain, vec!["base.html"]);
}

#[test]
fn missing_target_does_not_panic() {
    // Template references a non-existent parent — chain stops gracefully
    use jinja_lsp::workspace::index::WorkspaceIndex;
    use jinja_lsp::parsing::extract;

    let source = r#"{% extends "ghost.html" %}"#;
    let mut idx = extract(source);
    idx.path = "orphan.html".to_owned();

    let mut workspace = WorkspaceIndex { templates: std::collections::HashMap::new(), ..Default::default() };
    workspace.templates.insert("orphan.html".to_owned(), idx);

    let chain = workspace.template_chain("orphan.html");
    // "ghost.html" is not in the workspace → chain stops at orphan
    assert_eq!(chain, vec!["orphan.html"]);
}

#[test]
fn workspace_contains_all_discovered_templates() {
    let dir = tdir();
    let workspace = build_workspace(&[&dir], &["html"]);
    assert!(workspace.templates.contains_key("base.html"), "keys: {:?}", workspace.templates.keys().collect::<Vec<_>>());
    assert!(workspace.templates.contains_key("blog/post.html"), "keys: {:?}", workspace.templates.keys().collect::<Vec<_>>());
}

