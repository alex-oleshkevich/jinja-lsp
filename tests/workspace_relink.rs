use std::path::Path;

use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::{build_workspace, index::WorkspaceIndex};

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
    assert_eq!(
        chain,
        vec!["blog/post.html", "base.html"],
        "chain: {chain:?}"
    );
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
    use jinja_lsp::parsing::extract;
    use jinja_lsp::workspace::index::WorkspaceIndex;

    let source = r#"{% extends "ghost.html" %}"#;
    let mut idx = extract(source);
    idx.path = "orphan.html".to_owned();

    let mut workspace = WorkspaceIndex {
        templates: std::collections::HashMap::new(),
        ..Default::default()
    };
    workspace.templates.insert("orphan.html".to_owned(), idx);

    let chain = workspace.template_chain("orphan.html");
    // "ghost.html" is not in the workspace → chain stops at orphan
    assert_eq!(chain, vec!["orphan.html"]);
}

#[test]
fn jinja_lsp_gtgh_resolve_key_suffix_match_is_deterministic() {
    // Two templates share the basename "base.html" under different app roots.
    // {% extends "base.html" %} must always resolve to the SAME one (not a
    // random pick depending on HashMap iteration order) — the shortest
    // matching key wins, tie-broken lexicographically.
    let mut ws = WorkspaceIndex::default();
    ws.templates.insert(
        "app2/base.html".to_owned(),
        extract("{% block content %}{% endblock %}"),
    );
    ws.templates.insert(
        "app1/base.html".to_owned(),
        extract("{% block content %}{% endblock %}"),
    );

    let mut child_idx = extract(r#"{% extends "base.html" %}"#);
    child_idx.path = "child.html".to_owned();
    ws.templates.insert("child.html".to_owned(), child_idx);

    let chain = ws.template_chain("child.html");
    assert_eq!(
        chain,
        vec!["child.html", "app1/base.html"],
        "must deterministically resolve to app1/base.html (lexicographically first among equal-length candidates): {chain:?}"
    );
}

#[test]
fn jinja_lsp_gtgh_resolve_key_prefers_shortest_suffix_match() {
    // Among ambiguous (non-exact) suffix matches, the shortest key — the closer,
    // less-nested match — must win deterministically over a longer one.
    let mut ws = WorkspaceIndex::default();
    ws.templates.insert(
        "vendor/shared/base.html".to_owned(),
        extract("{% block content %}{% endblock %}"),
    );
    ws.templates.insert(
        "app/base.html".to_owned(),
        extract("{% block content %}{% endblock %}"),
    );

    let mut child_idx = extract(r#"{% extends "base.html" %}"#);
    child_idx.path = "child.html".to_owned();
    ws.templates.insert("child.html".to_owned(), child_idx);

    let chain = ws.template_chain("child.html");
    assert_eq!(
        chain,
        vec!["child.html", "app/base.html"],
        "shortest suffix match must win: {chain:?}"
    );
}

#[test]
fn workspace_contains_all_discovered_templates() {
    let dir = tdir();
    let workspace = build_workspace(&[&dir], &["html"]);
    assert!(
        workspace.templates.contains_key("base.html"),
        "keys: {:?}",
        workspace.templates.keys().collect::<Vec<_>>()
    );
    assert!(
        workspace.templates.contains_key("blog/post.html"),
        "keys: {:?}",
        workspace.templates.keys().collect::<Vec<_>>()
    );
}

#[test]
fn jinja_lsp_l8ve_first_dir_wins_on_relative_path_collision() {
    // jinja-lsp-l8ve: when two templates_dirs both contain the same relative path,
    // build_workspace inserted in discover_templates order, so the LAST dir silently
    // won — the opposite of Jinja's FileSystemLoader (first-match-wins). The first
    // dir's content must be the one indexed.
    let first = std::env::temp_dir().join("jinja_lsp_l8ve_first");
    let second = std::env::temp_dir().join("jinja_lsp_l8ve_second");
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    std::fs::write(
        first.join("shared.html"),
        "{% block from_first %}{% endblock %}",
    )
    .unwrap();
    std::fs::write(
        second.join("shared.html"),
        "{% block from_second %}{% endblock %}",
    )
    .unwrap();

    let workspace = build_workspace(&[&first, &second], &["html"]);
    let idx = workspace
        .templates
        .get("shared.html")
        .expect("shared.html must be indexed");
    assert_eq!(
        idx.blocks.first().map(|b| b.name.as_str()),
        Some("from_first"),
        "the first templates_dir must win on a relative-path collision: {:?}",
        idx.blocks
    );
}
