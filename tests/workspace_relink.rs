use std::path::Path;

use jinja_lsp::workspace::build_workspace;

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

    let mut workspace = WorkspaceIndex { templates: std::collections::HashMap::new() };
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
