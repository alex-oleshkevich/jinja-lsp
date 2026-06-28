// REQ-EXTR-08: each workspace folder gets its own WorkspaceIndex; cross-folder
// extends references stay unresolved.

use std::fs;

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
