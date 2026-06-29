use jinja_lsp::workspace::index::{TemplateIndex, WorkspaceIndex};
use jinja_lsp::workspace::symbols::{
    BlockDefinition, FromImport, ImportAlias, ImportedName, MacroDefinition, Parameter,
    Reference, ReferenceKind, Span, TemplateRefKind, TemplateReference,
    VariableDefinition, VariableScope,
};
use std::collections::HashMap;

fn span() -> Span {
    Span::default()
}

// REQ-DATA-01
#[test]
fn macro_definition_has_name_params_body_span() {
    let m = MacroDefinition {
        name: "post_url".into(),
        parameters: vec![
            Parameter { name: "post".into(), default: None },
            Parameter { name: "label".into(), default: Some("\"\"".into()) },
        ],
        body: span(),
        span: span(),
    };
    assert_eq!(m.name, "post_url");
    assert_eq!(m.parameters.len(), 2);
    assert!(m.parameters[0].default.is_none());
    assert!(m.parameters[1].default.is_some());
}

// REQ-DATA-02
#[test]
fn block_definition_has_scoped_and_required_flags() {
    let b = BlockDefinition {
        name: "content".into(),
        scoped: false,
        required: true,
        body: span(),
        span: span(),
    };
    assert_eq!(b.name, "content");
    assert!(!b.scoped);
    assert!(b.required);
}

// REQ-DATA-03
#[test]
fn variable_definition_has_name_scope_span_valid_range() {
    let v = VariableDefinition {
        name: "post".into(),
        scope: VariableScope::ForLoop,
        span: span(),
        valid_range: span(),
    };
    assert_eq!(v.name, "post");
    assert!(matches!(v.scope, VariableScope::ForLoop));
}

// REQ-DATA-04
#[test]
fn import_alias_has_alias_and_source() {
    let ia = ImportAlias { alias: "macros".into(), source: "blog/macros.html".into(), span: span() };
    assert_eq!(ia.alias, "macros");
    assert_eq!(ia.source, "blog/macros.html");
}

#[test]
fn from_import_has_source_and_names() {
    let fi = FromImport {
        source: "blog/macros.html".into(),
        names: vec![
            ImportedName { name: "post_url".into(), alias: None },
            ImportedName { name: "comment_card".into(), alias: Some("cc".into()) },
        ],
        span: span(),
    };
    assert_eq!(fi.source, "blog/macros.html");
    assert_eq!(fi.names.len(), 2);
    assert!(fi.names[0].alias.is_none());
    assert_eq!(fi.names[1].alias.as_deref(), Some("cc"));
}

// REQ-DATA-05
#[test]
fn template_reference_has_ignore_missing_and_is_dynamic_flags() {
    let static_ref = TemplateReference {
        kind: TemplateRefKind::Extends,
        path: "base.html".into(),
        ignore_missing: false,
        is_dynamic: false,
        span: span(),
    };
    assert!(!static_ref.ignore_missing);
    assert!(!static_ref.is_dynamic);

    let dynamic_ref = TemplateReference {
        kind: TemplateRefKind::Include,
        path: "".into(),
        ignore_missing: true,
        is_dynamic: true,
        span: span(),
    };
    assert!(dynamic_ref.ignore_missing);
    assert!(dynamic_ref.is_dynamic);
}

// REQ-DATA-06
#[test]
fn reference_records_name_kind_and_span() {
    let r = Reference { name: "post".into(), kind: ReferenceKind::Identifier, span: span() };
    assert_eq!(r.name, "post");
    assert!(matches!(r.kind, ReferenceKind::Identifier));

    let kinds = [
        ReferenceKind::Identifier,
        ReferenceKind::Attribute,
        ReferenceKind::Filter,
        ReferenceKind::Function,
        ReferenceKind::Test,
    ];
    assert_eq!(kinds.len(), 5);
}

// REQ-DATA-07
#[test]
fn variable_scope_has_nine_variants() {
    let scopes = [
        VariableScope::Template,
        VariableScope::Block,
        VariableScope::ForLoop,
        VariableScope::Macro,
        VariableScope::With,
        VariableScope::CallBlock,
        VariableScope::Trans,
        VariableScope::Filter,
        VariableScope::Autoescape,
    ];
    assert_eq!(scopes.len(), 9);
}

// REQ-DATA-08
#[test]
fn template_index_holds_one_files_symbols_and_errors() {
    let idx = TemplateIndex {
        path: "blog/post.html".into(),
        macros: vec![],
        blocks: vec![BlockDefinition { name: "content".into(), scoped: false, required: false, body: span(), span: span() }],
        variables: vec![],
        import_aliases: vec![],
        from_imports: vec![],
        template_refs: vec![TemplateReference {
            kind: TemplateRefKind::Extends,
            path: "base.html".into(),
            ignore_missing: false,
            is_dynamic: false,
            span: span(),
        }],
        references: vec![],
        syntax_errors: vec![],
    };
    assert_eq!(idx.path, "blog/post.html");
    assert_eq!(idx.blocks.len(), 1);
    assert_eq!(idx.template_refs.len(), 1);
    assert!(idx.syntax_errors.is_empty());
}

// REQ-DATA-09
#[test]
fn workspace_index_maps_paths_to_template_indexes() {
    let mut templates = HashMap::new();
    templates.insert(
        "blog/post.html".into(),
        TemplateIndex {
            path: "blog/post.html".into(),
            macros: vec![],
            blocks: vec![],
            variables: vec![],
            import_aliases: vec![],
            from_imports: vec![],
            template_refs: vec![],
            references: vec![],
            syntax_errors: vec![],
        },
    );
    let ws = WorkspaceIndex { templates, ..Default::default() };
    assert!(ws.templates.contains_key("blog/post.html"));
}

// REQ-DATA-10
#[test]
fn workspace_index_can_compute_template_chain() {
    let base = TemplateIndex {
        path: "base.html".into(),
        macros: vec![],
        blocks: vec![],
        variables: vec![],
        import_aliases: vec![],
        from_imports: vec![],
        template_refs: vec![],
        references: vec![],
        syntax_errors: vec![],
    };
    let post = TemplateIndex {
        path: "blog/post.html".into(),
        macros: vec![],
        blocks: vec![],
        variables: vec![],
        import_aliases: vec![],
        from_imports: vec![],
        template_refs: vec![TemplateReference {
            kind: TemplateRefKind::Extends,
            path: "base.html".into(),
            ignore_missing: false,
            is_dynamic: false,
            span: span(),
        }],
        references: vec![],
        syntax_errors: vec![],
    };
    let mut templates = HashMap::new();
    templates.insert("base.html".into(), base);
    templates.insert("blog/post.html".into(), post);
    let ws = WorkspaceIndex { templates, ..Default::default() };

    let chain = ws.template_chain("blog/post.html");
    assert_eq!(chain, vec!["blog/post.html", "base.html"]);
}

// REQ-DATA-03/07: VariableScope derived from syntactic context ───────────────

#[test]
fn set_inside_block_gets_block_scope() {
    use jinja_lsp::parsing::extract;
    let src = "{% block content %}{% set x = 1 %}{% endblock %}";
    let idx = extract(src);
    let var = idx.variables.iter().find(|v| v.name == "x");
    assert!(var.is_some(), "variable x should be extracted");
    assert_eq!(
        var.unwrap().scope, VariableScope::Block,
        "set inside block must have Block scope"
    );
}

#[test]
fn set_inside_macro_gets_macro_scope() {
    use jinja_lsp::parsing::extract;
    let src = "{% macro m() %}{% set x = 1 %}{% endmacro %}";
    let idx = extract(src);
    let var = idx.variables.iter().find(|v| v.name == "x");
    assert!(var.is_some(), "variable x should be extracted");
    assert_eq!(var.unwrap().scope, VariableScope::Macro, "set inside macro must have Macro scope");
}

#[test]
fn set_inside_filter_gets_filter_scope() {
    use jinja_lsp::parsing::extract;
    let src = "{% filter upper %}{% set x = 1 %}{% endfilter %}";
    let idx = extract(src);
    let var = idx.variables.iter().find(|v| v.name == "x");
    assert!(var.is_some(), "variable x should be extracted");
    assert_eq!(var.unwrap().scope, VariableScope::Filter, "set inside filter must have Filter scope");
}

#[test]
fn set_inside_autoescape_gets_autoescape_scope() {
    use jinja_lsp::parsing::extract;
    let src = "{% autoescape true %}{% set x = 1 %}{% endautoescape %}";
    let idx = extract(src);
    let var = idx.variables.iter().find(|v| v.name == "x");
    assert!(var.is_some(), "variable x should be extracted");
    assert_eq!(var.unwrap().scope, VariableScope::Autoescape, "set inside autoescape must have Autoescape scope");
}

#[test]
fn set_at_top_level_gets_template_scope() {
    use jinja_lsp::parsing::extract;
    let src = "{% set x = 1 %}";
    let idx = extract(src);
    let var = idx.variables.iter().find(|v| v.name == "x");
    assert!(var.is_some(), "variable x should be extracted");
    assert_eq!(var.unwrap().scope, VariableScope::Template, "top-level set must have Template scope");
}
