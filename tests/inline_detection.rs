// E31 inline template tests: REQ-INLN-01..05.

use jinja_lsp::parsing::inline::detect_inline_regions;

// ---------- REQ-INLN-02: embedded templates detected by host patterns --------

#[test]
fn detects_render_template_string_literal() {
    let source = r#"return render_template_string("{% if x %}{{ x }}{% endif %}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 1, "must detect one inline region");
    assert_eq!(regions[0].content, "{% if x %}{{ x }}{% endif %}");
}

#[test]
fn detects_custom_pattern() {
    let source = r#"render_jinja("{{ user.name }}")"#;
    let regions = detect_inline_regions(source, &["render_jinja"]);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].content, "{{ user.name }}");
}

#[test]
fn detects_multiple_calls_in_one_file() {
    let source = "render_template_string(\"{{ a }}\")\nrender_template_string(\"{{ b }}\")";
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 2);
}

// ---------- REQ-INLN-04: non-literal args are NOT detected ------------------

#[test]
fn variable_arg_is_not_detected() {
    // REQ-INLN-04: only literal string args are detected in v1
    let source = r#"render_template_string(template_var)"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert!(regions.is_empty(), "variable arg must not be detected: {regions:?}");
}

#[test]
fn empty_patterns_detects_nothing() {
    let source = r#"render_template_string("{{ x }}")"#;
    let regions = detect_inline_regions(source, &[]);
    assert!(regions.is_empty());
}

// ---------- REQ-INLN-03: inline ranges record host-file offset ---------------

#[test]
fn region_records_host_offset() {
    let source = r#"x = render_template_string("{% set a = 1 %}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 1);
    // The region's host_offset must be the byte position of "{% set..." inside source
    let expected_offset = source.find("{% set").unwrap();
    assert_eq!(regions[0].host_offset, expected_offset, "host_offset must point to content start");
}

#[test]
fn region_records_host_line_and_col() {
    let source = "# line 0\nresult = render_template_string(\"{{ x }}\")";
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].host_line, 1, "must be on line 1 (0-indexed)");
}

// ---------- REQ-INLN-01: inline grammar parses intra-delimiter content -------

#[test]
fn inline_grammar_parses_expression() {
    use jinja_lsp::parsing::extract;
    // Inline content extracted from a detect_inline_regions call feeds extract()
    let source = r#"render_template_string("{% macro greet(name) %}{{ name }}{% endmacro %}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 1);
    let idx = extract(&regions[0].content);
    assert_eq!(idx.macros.len(), 1, "inline grammar must parse macro definition");
    assert_eq!(idx.macros[0].name, "greet");
}

// ---------- REQ-INLN-05: inline ranges are ordinary TemplateIndex entries ---

#[test]
fn inline_region_indexes_like_standalone_file() {
    use std::collections::HashMap;
    use jinja_lsp::workspace::index::WorkspaceIndex;

    let source = r#"render_template_string("{% set x = 1 %}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);

    let mut ws = WorkspaceIndex { templates: HashMap::new(), ..Default::default() };
    ws.index_inline("views.py#0", &regions[0].content);

    assert!(ws.templates.contains_key("views.py#0"));
    assert!(!ws.templates["views.py#0"].variables.is_empty());
}
