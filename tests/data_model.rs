use jinja_lsp::workspace::index::{ResolvedBinding, TemplateIndex, WorkspaceIndex};
use jinja_lsp::workspace::symbols::{
    BlockDefinition, EnclosingOwner, FromImport, ImportAlias, ImportedName, MacroDefinition, Parameter,
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

// REQ-DATA-12: enclosing-owner computation ────────────────────────────────────

fn make_macro(name: &str, body_start: usize, body_end: usize) -> MacroDefinition {
    MacroDefinition {
        name: name.to_owned(),
        parameters: vec![],
        body: Span { start_byte: body_start, end_byte: body_end, ..Span::default() },
        span: Span::default(),
    }
}

fn make_block(name: &str, body_start: usize, body_end: usize) -> BlockDefinition {
    BlockDefinition {
        name: name.to_owned(),
        scoped: false,
        required: false,
        body: Span { start_byte: body_start, end_byte: body_end, ..Span::default() },
        span: Span::default(),
    }
}

fn query_span(start_byte: usize, end_byte: usize) -> Span {
    Span { start_byte, end_byte, ..Span::default() }
}

#[test]
fn enclosing_owner_returns_template_when_no_body_contains_span() {
    use jinja_lsp::parsing::extract;
    let src = "{% macro m() %}body{% endmacro %}{{ x }}";
    let idx = extract(src);
    // "x" reference is at byte >= body_end, outside any macro body
    let q = query_span(37, 38); // roughly where "x" is
    let owner = idx.enclosing_owner(&q);
    assert!(matches!(owner, EnclosingOwner::Template), "outside any body should be Template");
}

#[test]
fn enclosing_owner_returns_macro_when_span_in_macro_body() {
    use jinja_lsp::parsing::extract;
    let src = "{% macro m() %}{{ x }}{% endmacro %}";
    let idx = extract(src);
    // The macro body is between the end of the opening tag and start of endmacro
    let x_byte = src.find("x").unwrap();
    let q = query_span(x_byte, x_byte + 1);
    let owner = idx.enclosing_owner(&q);
    match owner {
        EnclosingOwner::Macro(m) => assert_eq!(m.name, "m"),
        other => panic!("expected Macro, got {other:?}"),
    }
}

#[test]
fn enclosing_owner_returns_innermost_for_nested_macro_in_block() {
    // REQ-DATA-12: when both a block and a macro contain the span,
    // the innermost (smallest containing body) wins.
    let idx = TemplateIndex {
        macros: vec![make_macro("inner", 50, 100)],
        blocks: vec![make_block("outer", 10, 200)],
        ..TemplateIndex::empty()
    };
    let q = query_span(60, 70); // inside both outer block and inner macro
    let owner = idx.enclosing_owner(&q);
    match owner {
        EnclosingOwner::Macro(m) => assert_eq!(m.name, "inner", "innermost (macro) should win"),
        other => panic!("expected Macro(inner), got {other:?}"),
    }
}

// REQ-DATA-11: reference → binding resolution ─────────────────────────────────

#[test]
fn resolve_variable_reference_finds_innermost_binding() {
    use jinja_lsp::parsing::extract;
    // Outer `post` set at top level; inner `post` in a for loop body.
    // A reference to `post` inside the for loop body must resolve to the for-loop binding.
    let src = "{% set post = 'outer' %}{% for post in items %}{{ post }}{% endfor %}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();

    // Find the reference to `post` inside the for body (the {{ post }} expression)
    let ref_post = idx.references.iter().find(|r| r.name == "post").expect("reference to post must exist");
    match idx.resolve_reference(ref_post, &ws) {
        ResolvedBinding::Variable(v) => {
            assert_eq!(v.name, "post");
            assert_eq!(v.scope, VariableScope::ForLoop, "innermost binding is from the for loop");
        }
        other => panic!("expected Variable, got {other:?}"),
    }
}

#[test]
fn resolve_variable_outside_scope_returns_host_owned() {
    use jinja_lsp::parsing::extract;
    // `ctx_var` is a host-injected variable — no VariableDefinition exists for it.
    let src = "{{ ctx_var }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();

    let ref_ctx = idx.references.iter()
        .find(|r| r.name == "ctx_var" && matches!(r.kind, ReferenceKind::Identifier))
        .expect("identifier reference to ctx_var must exist");
    assert!(matches!(idx.resolve_reference(ref_ctx, &ws), ResolvedBinding::HostOwned));
}

#[test]
fn resolve_macro_call_finds_local_macro() {
    use jinja_lsp::parsing::extract;
    let src = "{% macro greet(name) %}Hi{% endmacro %}{{ greet('Alice') }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();

    let ref_greet = idx.references.iter().find(|r| r.name == "greet" && matches!(r.kind, ReferenceKind::Function)).expect("function reference to greet");
    match idx.resolve_reference(ref_greet, &ws) {
        ResolvedBinding::Macro(m) => assert_eq!(m.name, "greet"),
        other => panic!("expected Macro, got {other:?}"),
    }
}

#[test]
fn resolve_macro_call_finds_workspace_macro() {
    // `post_url` is defined in "macros.html" but called from "post.html".
    let macros_idx = {
        use jinja_lsp::parsing::extract;
        let mut idx = extract("{% macro post_url(post) %}/post/{{ post.slug }}{% endmacro %}");
        idx.path = "macros.html".into();
        idx
    };
    let post_idx = {
        use jinja_lsp::parsing::extract;
        let mut idx = extract("{{ post_url(post) }}");
        idx.path = "post.html".into();
        idx
    };

    let mut ws = WorkspaceIndex::default();
    ws.templates.insert("macros.html".into(), macros_idx);

    let ref_post_url = post_idx.references.iter()
        .find(|r| r.name == "post_url" && matches!(r.kind, ReferenceKind::Function))
        .expect("function reference to post_url");
    match post_idx.resolve_reference(ref_post_url, &ws) {
        ResolvedBinding::Macro(m) => assert_eq!(m.name, "post_url"),
        other => panic!("expected workspace Macro, got {other:?}"),
    }
}

#[test]
fn resolve_set_variable_at_top_level_binds_correctly() {
    use jinja_lsp::parsing::extract;
    let src = "{% set title = 'Hello' %}{{ title }}";
    let idx = extract(src);
    let ws = WorkspaceIndex::default();

    let ref_title = idx.references.iter().find(|r| r.name == "title").expect("reference to title");
    match idx.resolve_reference(ref_title, &ws) {
        ResolvedBinding::Variable(v) => {
            assert_eq!(v.name, "title");
            assert_eq!(v.scope, VariableScope::Template);
        }
        other => panic!("expected Variable, got {other:?}"),
    }
}
