// REQ-EXTR-05: inline regions index into WorkspaceIndex identically to standalone files.

use std::{collections::HashMap, fs};

use jinja_lsp::workspace::{build_workspace, index::WorkspaceIndex};

const JINJA: &str = r#"{% macro greet(name) %}Hello {{ name }}{% endmacro %}
{% set site = "My Site" %}
"#;

fn file_based_index(dir_suffix: &str) -> jinja_lsp::workspace::index::TemplateIndex {
    let tmp = std::env::temp_dir().join(format!("jinja_lsp_inline_{dir_suffix}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("inline_test.html"), JINJA).unwrap();

    let ws = build_workspace(&[&tmp], &["html"]);
    ws.templates.into_values().next().unwrap()
}

fn inline_index() -> jinja_lsp::workspace::index::TemplateIndex {
    let mut ws = WorkspaceIndex {
        templates: HashMap::new(),
        ..Default::default()
    };
    ws.index_inline("render#0", JINJA);
    ws.templates.into_values().next().unwrap()
}

#[test]
fn inline_regions_have_same_macros_as_files() {
    let file_idx = file_based_index("macros");
    let inline_idx = inline_index();

    assert_eq!(
        file_idx.macros.len(),
        inline_idx.macros.len(),
        "macro count must match: file={} inline={}",
        file_idx.macros.len(),
        inline_idx.macros.len(),
    );
    assert_eq!(
        file_idx.macros[0].name, inline_idx.macros[0].name,
        "macro name must match"
    );
}

#[test]
fn inline_regions_have_same_variables_as_files() {
    let file_idx = file_based_index("vars");
    let inline_idx = inline_index();

    assert_eq!(
        file_idx.variables.len(),
        inline_idx.variables.len(),
        "variable count must match"
    );
    let file_var = file_idx.variables.iter().find(|v| v.name == "site");
    let inline_var = inline_idx.variables.iter().find(|v| v.name == "site");
    assert!(file_var.is_some(), "file index must have 'site' variable");
    assert!(
        inline_var.is_some(),
        "inline index must have 'site' variable"
    );
}

#[test]
fn inline_entry_is_in_workspace_index() {
    let mut ws = WorkspaceIndex {
        templates: HashMap::new(),
        ..Default::default()
    };
    ws.index_inline("render#0", JINJA);
    assert!(
        ws.templates.contains_key("render#0"),
        "inline entry must be keyed by the given key"
    );
}

#[test]
fn workspace_with_inline_and_file_entries_coexist() {
    let tmp = std::env::temp_dir().join("jinja_lsp_inline_coexist");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("base.html"), "{% block content %}{% endblock %}").unwrap();

    let mut ws = build_workspace(&[&tmp], &["html"]);
    ws.index_inline("render#0", JINJA);

    assert!(
        ws.templates.contains_key("base.html"),
        "file entry must be present"
    );
    assert!(
        ws.templates.contains_key("render#0"),
        "inline entry must be present"
    );
    assert_eq!(ws.templates.len(), 2);
}
