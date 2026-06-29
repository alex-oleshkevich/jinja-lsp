use std::{collections::HashMap, path::Path};

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

// ── REQ-EXTR-06: import graph built by relink() ────────────────────────────

fn make_workspace(pairs: &[(&str, &str)]) -> WorkspaceIndex {
    let mut ws = WorkspaceIndex { templates: HashMap::new(), ..Default::default() };
    for (path, src) in pairs {
        let mut idx = extract(src);
        idx.path = path.to_string();
        ws.templates.insert(path.to_string(), idx);
    }
    ws
}

#[test]
fn relink_builds_import_graph_from_extends() {
    let mut ws = make_workspace(&[
        ("base.html", "{% block content %}{% endblock %}"),
        ("child.html", r#"{% extends "base.html" %}"#),
    ]);
    ws.relink();
    let refs = ws.import_graph.get("child.html").expect("child must have graph entry");
    assert!(refs.contains(&"base.html".to_string()), "child → base must be in import graph");
}

#[test]
fn relink_builds_import_graph_from_import() {
    let mut ws = make_workspace(&[
        ("macros.html", "{% macro foo() %}hi{% endmacro %}"),
        ("page.html", r#"{% import "macros.html" as m %}"#),
    ]);
    ws.relink();
    let refs = ws.import_graph.get("page.html").expect("page must have graph entry");
    assert!(refs.contains(&"macros.html".to_string()), "page → macros must be in import graph");
}

#[test]
fn relink_root_template_has_empty_import_graph_entry() {
    let mut ws = make_workspace(&[
        ("base.html", "{% block content %}{% endblock %}"),
    ]);
    ws.relink();
    let refs = ws.import_graph.get("base.html").expect("base must have graph entry");
    assert!(refs.is_empty(), "root with no refs must have empty graph entry");
}

#[test]
fn relink_detects_direct_cycle() {
    let mut ws = make_workspace(&[
        ("a.html", r#"{% extends "b.html" %}"#),
        ("b.html", r#"{% extends "a.html" %}"#),
    ]);
    ws.relink();
    assert!(ws.has_import_cycle("a.html"), "a→b→a must be a cycle");
}

#[test]
fn relink_detects_no_cycle_in_linear_chain() {
    let mut ws = make_workspace(&[
        ("base.html", "{% block content %}{% endblock %}"),
        ("mid.html", r#"{% extends "base.html" %}"#),
        ("leaf.html", r#"{% extends "mid.html" %}"#),
    ]);
    ws.relink();
    assert!(!ws.has_import_cycle("leaf.html"), "linear chain must not be a cycle");
}

#[test]
fn relink_diamond_dependency_is_not_a_cycle() {
    // A imports both B and C; both import D — a valid diamond, not a cycle.
    // The single-visited-set algorithm false-positives here; the two-set DFS must not.
    let mut ws = make_workspace(&[
        ("d.html", "{% macro m() %}{% endmacro %}"),
        ("b.html", r#"{% import "d.html" as d %}"#),
        ("c.html", r#"{% import "d.html" as d %}"#),
        ("a.html", r#"{% import "b.html" as b %}{% import "c.html" as c %}"#),
    ]);
    ws.relink();
    assert!(!ws.has_import_cycle("a.html"), "diamond dependency must not be reported as a cycle");
}

#[test]
fn relink_skips_dynamic_refs_in_graph() {
    // Dynamic extends (is_dynamic=true) cannot be statically resolved — must not appear in graph.
    use jinja_lsp::workspace::symbols::{TemplateReference, TemplateRefKind};
    use jinja_lsp::workspace::index::TemplateIndex;
    let mut ws = WorkspaceIndex { templates: HashMap::new(), ..Default::default() };
    let mut idx = TemplateIndex::empty();
    idx.path = "dynamic.html".to_string();
    idx.template_refs.push(TemplateReference {
        kind: TemplateRefKind::Extends,
        path: "runtime_var".to_string(),
        ignore_missing: false,
        is_dynamic: true,
        span: Default::default(),
    });
    ws.templates.insert("dynamic.html".to_string(), idx);
    ws.relink();
    let refs = ws.import_graph.get("dynamic.html").expect("must have entry");
    assert!(refs.is_empty(), "dynamic refs must be excluded from import graph");
}
