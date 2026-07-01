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

// ---------- REQ-INLN-02: word-boundary check prevents substring false positives (2d3i) ---

#[test]
fn d2di_suffix_match_does_not_trigger() {
    // "prerender_template_string" ends with "render_template_string" — must not match.
    let source = r#"prerender_template_string("{{ x }}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert!(regions.is_empty(), "suffix of longer identifier must not match: {regions:?}");
}

#[test]
fn d2di_exact_function_name_still_matches() {
    let source = r#"render_template_string("{{ x }}")"#;
    let regions = detect_inline_regions(source, &["render_template_string"]);
    assert_eq!(regions.len(), 1);
}

// ---------- REQ-INLN-03: host_col is byte offset within host line (lvi1) -----

#[test]
fn lvi1_host_col_counts_bytes() {
    // "é" is 2 UTF-8 bytes but 1 UTF-16 code unit.
    // host_col uses byte offset (consistent with the rest of the pipeline which
    // uses byte columns; server's byte_col_to_lsp_char converts to LSP units).
    let source = "é = render_jinja(\"{{ x }}\")";
    let regions = detect_inline_regions(source, &["render_jinja"]);
    assert_eq!(regions.len(), 1);
    // Byte offset before content: 'é'(2 bytes) + ' = render_jinja("'(17 bytes) = 19
    assert_eq!(regions[0].host_col, 19, "host_col must be byte offset, got {}", regions[0].host_col);
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

// ---------- REQ-INLN-03: InlineRange coordinate translation -----------------

#[test]
fn inline_range_to_host_position_line0() {
    use jinja_lsp::workspace::inline::InlineRange;
    let range = InlineRange { host_path: "v.py".into(), host_offset: 10, host_line: 3, host_col: 20, content_len: 50 };
    // inline line 0 col 5 → host (3, 25)
    assert_eq!(range.to_host_position(0, 5), (3, 25));
}

#[test]
fn inline_range_to_host_position_later_line() {
    use jinja_lsp::workspace::inline::InlineRange;
    let range = InlineRange { host_path: "v.py".into(), host_offset: 0, host_line: 2, host_col: 10, content_len: 80 };
    // inline line 1 col 4 → host (3, 4) — host_col NOT added on lines > 0
    assert_eq!(range.to_host_position(1, 4), (3, 4));
}

#[test]
fn inline_range_to_inline_position_line0() {
    use jinja_lsp::workspace::inline::InlineRange;
    let range = InlineRange { host_path: "v.py".into(), host_offset: 5, host_line: 0, host_col: 5, content_len: 30 };
    // host (0, 7) → inline (0, 2)
    assert_eq!(range.to_inline_position(0, 7), Some((0, 2)));
}

#[test]
fn inline_range_to_inline_position_before_start() {
    use jinja_lsp::workspace::inline::InlineRange;
    let range = InlineRange { host_path: "v.py".into(), host_offset: 10, host_line: 1, host_col: 5, content_len: 20 };
    // host line 0 is before inline start (line 1)
    assert_eq!(range.to_inline_position(0, 0), None);
}

#[test]
fn inline_range_contains_host_byte() {
    use jinja_lsp::workspace::inline::InlineRange;
    let range = InlineRange { host_path: "v.py".into(), host_offset: 10, host_line: 0, host_col: 10, content_len: 20 };
    assert!(range.contains_host_byte(10));
    assert!(range.contains_host_byte(15));
    assert!(!range.contains_host_byte(30)); // past end
    assert!(!range.contains_host_byte(9));  // before start
}

#[test]
fn server_state_populates_inline_ranges_on_update_file() {
    use jinja_lsp::server::state::ServerState;
    use jinja_lsp::config::JinjaConfig;

    let mut cfg = JinjaConfig::default();
    cfg.inline_patterns = vec!["render_tpl".to_owned()];
    // Mark .py files as host files (not in extensions)
    cfg.extensions = vec!["html".to_owned()];

    let mut state = ServerState::with_config(cfg);
    let source = r#"x = render_tpl("{{ user }}")"#;
    state.update_file("views.py", source);

    let workspace = &state.workspace;
    // There must be exactly one inline range for views.py
    let ranges: Vec<_> = workspace.inline_ranges_for("views.py").collect();
    assert_eq!(ranges.len(), 1, "must have one inline range: {ranges:?}");
    let (ikey, range) = &ranges[0];
    assert!(ikey.starts_with("views.py::"), "inline key must be views.py::<offset>");
    assert_eq!(range.host_path, "views.py");
    assert_eq!(range.host_line, 0, "content is on line 0");
}

#[test]
fn inline_diagnostic_position_translated_to_host_coords() {
    // REQ-INLN-03: diagnostic from an inline region must point at the host-file position.
    use jinja_lsp::workspace::index::WorkspaceIndex;
    use jinja_lsp::workspace::inline::InlineRange;
    use jinja_lsp::diagnostics::checks::run_checks;
    use jinja_lsp::builtins::registry::Registry;
    use std::collections::HashMap;

    // Host file: one line with an inline Jinja template containing an unknown filter.
    let host_source = r#"result = render_tpl("{{ x | undefined_filter }}")"#;
    let inline_content = "{{ x | undefined_filter }}";
    let host_offset = host_source.find(inline_content).unwrap();

    let mut ws = WorkspaceIndex { templates: HashMap::new(), ..Default::default() };
    ws.index_inline("views.py::0", inline_content);
    let range = InlineRange {
        host_path: "views.py".into(),
        host_offset,
        host_line: 0,
        host_col: host_offset as u32,
        content_len: inline_content.len(),
    };
    ws.register_inline_range("views.py::0", range.clone());

    let registry = Registry::load_core();
    let iidx = ws.templates.get("views.py::0").unwrap();
    let raw = run_checks(inline_content, "views.py::0", iidx, &registry, &ws);
    // E102 fires for undefined_filter; E101 for `x` (undefined variable).
    // Whatever diagnostics appear, just verify the position translation math.
    for d in &raw {
        let (hl, hc) = range.to_host_position(d.line, d.col);
        // The host-translated position must be >= host_col (since content starts after it)
        assert!(hc >= host_offset as u32, "translated col {hc} must be >= host_col {}", host_offset);
        assert_eq!(hl, 0, "all inline content on line 0 stays on line 0");
    }
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
