// F01 diagnostics tests: REQ-DIAG-01 through REQ-DIAG-06.

use jinja_lsp::diagnostics::{
    filter_by_config, parse_noqa_directives, suppress_by_noqa, DiagCode, NoqaDirective,
};
use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};

fn make_diag(line: u32, code: &str, slug: &str) -> Diagnostic {
    Diagnostic {
        file: "test.html".to_owned(),
        line,
        col: 0,
        code: code.to_owned(),
        slug: slug.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: "test".to_owned(),
    }
}

// ---------- ADR-003: severity derived from code prefix ----------------------

#[test]
fn severity_from_code_str_e_is_error() {
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-E001"), DiagnosticSeverity::Error);
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-E601"), DiagnosticSeverity::Error);
}

#[test]
fn severity_from_code_str_w_is_warning() {
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-W107"), DiagnosticSeverity::Warning);
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-W301"), DiagnosticSeverity::Warning);
}

#[test]
fn severity_from_code_str_i_is_info() {
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-I001"), DiagnosticSeverity::Info);
}

#[test]
fn severity_from_code_str_h_is_hint() {
    assert_eq!(DiagnosticSeverity::from_code_str("JINJA-H001"), DiagnosticSeverity::Hint);
}

#[test]
fn diag_code_severity_matches_prefix() {
    // Every DiagCode's severity() must agree with its code_str() prefix.
    let codes = [
        DiagCode::E001, DiagCode::E101, DiagCode::E102, DiagCode::E103,
        DiagCode::E104, DiagCode::W106, DiagCode::W107, DiagCode::W201,
        DiagCode::W202, DiagCode::W203, DiagCode::W301, DiagCode::W302,
        DiagCode::W303, DiagCode::W304, DiagCode::W305, DiagCode::E401,
        DiagCode::W402, DiagCode::E403, DiagCode::E404, DiagCode::E501,
        DiagCode::E601,
    ];
    for code in codes {
        let expected = DiagnosticSeverity::from_code_str(code.code_str());
        assert_eq!(
            code.severity(), expected,
            "{}.severity() must match code_str() prefix",
            code.code_str()
        );
    }
}

#[test]
fn jinja_lsp_rm5r_all_lists_every_diagcode_variant_exhaustively() {
    // Compile-time enforcement: this match has no wildcard arm, so adding a new
    // DiagCode variant makes it fail to compile — forcing DiagCode::ALL (the
    // single source noqa's known-codes list derives from) to be updated in the
    // same change instead of silently drifting out of sync.
    fn check(c: DiagCode) {
        assert!(DiagCode::ALL.contains(&c), "{c:?} is missing from DiagCode::ALL");
        match c {
            DiagCode::E001 | DiagCode::E102 | DiagCode::E104 | DiagCode::W201 | DiagCode::W301
            | DiagCode::W302 | DiagCode::W303 | DiagCode::W304 | DiagCode::W305 | DiagCode::W106
            | DiagCode::W107 | DiagCode::E101 | DiagCode::E103 | DiagCode::W202 | DiagCode::W203
            | DiagCode::E401 | DiagCode::W402 | DiagCode::E403 | DiagCode::E404 | DiagCode::E501
            | DiagCode::E601 => {}
        }
    }
    for &c in DiagCode::ALL {
        check(c);
    }
    assert_eq!(DiagCode::ALL.len(), 21, "DiagCode::ALL must list all 21 variants exactly once");
}

#[test]
fn w107_noqa_warning_uses_derived_severity() {
    // noqa.rs W107 diagnostic must derive its severity from the code, not hardcode it
    let src = include_str!("../src/diagnostics/noqa.rs");
    assert!(
        !src.contains("DiagnosticSeverity::Warning"),
        "noqa.rs must not hardcode DiagnosticSeverity::Warning — derive it from DiagCode::W107.severity()"
    );
}

// ---------- REQ-DIAG-01: stable kebab-case slugs ----------------------------

#[test]
fn all_known_codes_have_slug() {
    // Every entry in DiagCode has a kebab-case slug
    let codes = [
        (DiagCode::E001, "syntax-error"),
        (DiagCode::E101, "undefined-variable"),
        (DiagCode::E102, "undefined-filter"),
        (DiagCode::W201, "unused-variable"),
        (DiagCode::W301, "duplicate-block"),
        (DiagCode::W107, "invalid-noqa"),
        (DiagCode::E601, "template-does-not-exist"),
    ];
    for (code, expected_slug) in codes {
        assert_eq!(code.slug(), expected_slug, "code {code:?} slug mismatch");
    }
}

// ---------- REQ-DIAG-03: select/ignore filter --------------------------------

#[test]
fn select_by_full_code_keeps_only_that_code() {
    let diags = vec![
        make_diag(1, "JINJA-E101", "undefined-variable"),
        make_diag(2, "JINJA-W201", "unused-variable"),
    ];
    let filtered = filter_by_config(&diags, &["JINJA-E101"], &[]);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].code, "JINJA-E101");
}

#[test]
fn select_by_class_prefix_keeps_class() {
    let diags = vec![
        make_diag(1, "JINJA-E101", "undefined-variable"),
        make_diag(2, "JINJA-E102", "undefined-filter"),
        make_diag(3, "JINJA-W201", "unused-variable"),
    ];
    let filtered = filter_by_config(&diags, &["JINJA-E"], &[]);
    assert_eq!(filtered.len(), 2, "only E-class must remain");
    assert!(filtered.iter().all(|d| d.code.starts_with("JINJA-E")));
}

#[test]
fn ignore_removes_matching_code() {
    let diags = vec![
        make_diag(1, "JINJA-E101", "undefined-variable"),
        make_diag(2, "JINJA-W201", "unused-variable"),
    ];
    let filtered = filter_by_config(&diags, &[], &["JINJA-W201"]);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].code, "JINJA-E101");
}

#[test]
fn ignore_wins_over_select_when_overlapping() {
    let diags = vec![make_diag(1, "JINJA-E101", "undefined-variable")];
    // both select and ignore include E101 → ignore wins
    let filtered = filter_by_config(&diags, &["JINJA-E101"], &["JINJA-E101"]);
    assert!(filtered.is_empty(), "ignore must win over select");
}

#[test]
fn empty_select_means_all_enabled() {
    let diags = vec![
        make_diag(1, "JINJA-E101", "undefined-variable"),
        make_diag(2, "JINJA-W201", "unused-variable"),
    ];
    // empty select = all enabled
    let filtered = filter_by_config(&diags, &[], &[]);
    assert_eq!(filtered.len(), 2);
}

// ---------- REQ-DIAG-04: noqa directive parsing ------------------------------

#[test]
fn parses_bare_noqa() {
    let directives = parse_noqa_directives("{# noqa #}", 5);
    assert_eq!(directives.len(), 1);
    assert!(matches!(directives[0], NoqaDirective::All { .. }));
}

#[test]
fn parses_noqa_with_codes() {
    let directives = parse_noqa_directives("{# noqa: JINJA-E101, JINJA-W2 #}", 3);
    assert_eq!(directives.len(), 1);
    if let NoqaDirective::Codes { codes, .. } = &directives[0] {
        assert!(codes.contains(&"JINJA-E101".to_owned()));
        assert!(codes.contains(&"JINJA-W2".to_owned()));
    } else {
        panic!("expected Codes variant");
    }
}

#[test]
fn parses_noqa_with_bare_space_separator() {
    // REQ-DIAG-04: bare space separator is tolerated
    let directives = parse_noqa_directives("{# noqa JINJA-E101 #}", 1);
    assert_eq!(directives.len(), 1);
    assert!(matches!(directives[0], NoqaDirective::Codes { .. }));
}

#[test]
fn parses_noqa_file_all() {
    let directives = parse_noqa_directives("{# noqa-file #}", 0);
    assert_eq!(directives.len(), 1);
    assert!(matches!(directives[0], NoqaDirective::FileAll { .. }));
}

#[test]
fn parses_noqa_file_with_codes() {
    let directives = parse_noqa_directives("{# noqa-file: JINJA-W2 #}", 0);
    assert_eq!(directives.len(), 1);
    assert!(matches!(directives[0], NoqaDirective::FileCodes { .. }));
}

// ---------- REQ-DIAG-05: noqa suppression scope model -----------------------

#[test]
fn noqa_on_same_line_suppresses_diagnostic() {
    let source = "{{ undefined_var }}   {# noqa: JINJA-E101 #}";
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, _w107) = suppress_by_noqa(&diags, source);
    assert!(kept.is_empty(), "E101 must be suppressed by same-line noqa");
}

#[test]
fn noqa_all_suppresses_all_on_line() {
    let source = "{{ x }} {{ y }}   {# noqa #}";
    let diags = vec![
        make_diag(0, "JINJA-E101", "undefined-variable"),
        make_diag(0, "JINJA-W201", "unused-variable"),
    ];
    let (kept, _w107) = suppress_by_noqa(&diags, source);
    assert!(kept.is_empty());
}

#[test]
fn noqa_does_not_suppress_different_line() {
    let source = "line0\n{{ undefined_var }}   {# noqa: JINJA-E101 #}";
    // Diagnostic is on line 0, noqa is on line 1
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, _w107) = suppress_by_noqa(&diags, source);
    assert_eq!(kept.len(), 1, "noqa on different line must not suppress");
}

#[test]
fn noqa_file_suppresses_whole_file() {
    let source = "{# noqa-file #}\n{{ x }}\n{{ y }}";
    let diags = vec![
        make_diag(1, "JINJA-E101", "undefined-variable"),
        make_diag(2, "JINJA-E101", "undefined-variable"),
    ];
    let (kept, _w107) = suppress_by_noqa(&diags, source);
    assert!(kept.is_empty());
}

// ---------- REQ-DIAG-06: invalid noqa IDs raise JINJA-W107 ------------------

#[test]
fn invalid_noqa_code_produces_w107() {
    // "undefined-variable" is a slug, not a code — raises W107
    let source = "{{ x }}   {# noqa: undefined-variable #}";
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, w107s) = suppress_by_noqa(&diags, source);
    // The invalid ID doesn't suppress; W107 is produced
    assert_eq!(kept.len(), 1, "invalid ID must not suppress");
    assert!(!w107s.is_empty(), "must produce W107 for invalid ID");
    assert!(w107s[0].code == "JINJA-W107");
}

#[test]
fn valid_and_invalid_noqa_ids_mixed() {
    // Valid "JINJA-E101" suppresses; invalid slug produces W107
    let source = "{{ x }}   {# noqa: JINJA-E101, not-a-code #}";
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, w107s) = suppress_by_noqa(&diags, source);
    assert!(kept.is_empty(), "valid ID must suppress E101");
    assert!(!w107s.is_empty(), "invalid ID must produce W107");
}

#[test]
fn ulcx_unknown_uppercase_jinja_code_produces_w107() {
    // "JINJA-FOO" starts with JINJA- and has no lowercase, but is not a known code.
    // The old implementation accepted it silently; the fix must reject it with W107.
    let source = "{{ x }}   {# noqa: JINJA-FOO #}";
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, w107s) = suppress_by_noqa(&diags, source);
    assert_eq!(kept.len(), 1, "JINJA-FOO must not suppress diagnostics");
    assert!(!w107s.is_empty(), "JINJA-FOO must produce W107 (not in known-codes list)");
}

#[test]
fn ulcx_partial_prefix_does_not_over_suppress() {
    // "JINJA-E10" is NOT a valid prefix (the known-prefix list has "JINJA-E1", not "JINJA-E10").
    // With the old starts_with suppression check, JINJA-E10 silently suppressed JINJA-E101.
    // After the fix, JINJA-E10 must be rejected as invalid → W107, no suppression.
    let source = "{{ x }}   {# noqa: JINJA-E10 #}";
    let diags = vec![make_diag(0, "JINJA-E101", "undefined-variable")];
    let (kept, w107s) = suppress_by_noqa(&diags, source);
    assert_eq!(kept.len(), 1, "JINJA-E10 is not a valid prefix and must not suppress");
    assert!(!w107s.is_empty(), "JINJA-E10 must produce W107");
}

// ---------- jinja-lsp-ep8u: whitespace-control markers in noqa comments ------

#[test]
fn trim_marker_bare_noqa_all_suppresses() {
    let directives = parse_noqa_directives("{#- noqa -#}", 0);
    assert_eq!(directives.len(), 1, "trim-marker noqa must parse as NoqaDirective::All");
    assert!(matches!(directives[0], NoqaDirective::All { .. }));
}

#[test]
fn trim_marker_noqa_with_code_suppresses() {
    let directives = parse_noqa_directives("{#- noqa: JINJA-E101 -#}", 0);
    assert_eq!(directives.len(), 1, "trim-marker noqa with code must parse as NoqaDirective::Codes");
    assert!(matches!(&directives[0], NoqaDirective::Codes { codes, .. } if codes[0] == "JINJA-E101"));
}

#[test]
fn trim_marker_noqa_suppresses_via_suppress_by_noqa() {
    let diags = vec![Diagnostic {
        file: String::new(), line: 0, col: 0,
        code: "JINJA-E101".to_owned(), slug: "undefined-identifier".to_owned(),
        severity: DiagCode::E101.severity(), message: "x undefined".to_owned(),
    }];
    let source = "{{ x }}   {#- noqa -#}";
    let (kept, _w107) = suppress_by_noqa(&diags, source);
    assert!(kept.is_empty(), "trim-marker noqa must suppress the diagnostic on the same line");
}

// ---------- E101 false-positive regression tests ─────────────────────────────

#[test]
fn e101_macro_parameter_not_undefined() {
    use jinja_lsp::parsing::extract;
    use jinja_lsp::diagnostics::checks::run_checks;
    use jinja_lsp::builtins::registry::Registry;
    use jinja_lsp::workspace::index::WorkspaceIndex;

    let source = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let idx = extract(source);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let diags = run_checks(source, "t.html", &idx, &reg, &ws);
    let e101: Vec<_> = diags.iter().filter(|d| d.code == "JINJA-E101").collect();
    assert!(e101.is_empty(), "macro parameter 'name' must not fire E101: {e101:?}");
}

#[test]
fn e101_attribute_chain_intermediate_not_undefined() {
    use jinja_lsp::parsing::extract;
    use jinja_lsp::diagnostics::checks::run_checks;
    use jinja_lsp::builtins::registry::Registry;
    use jinja_lsp::workspace::index::WorkspaceIndex;

    // request.user captured as dotted Identifier by @object; must not fire E101.
    let source = "{{ request.user.name }}";
    let idx = extract(source);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let diags = run_checks(source, "t.html", &idx, &reg, &ws);
    // Exactly one E101 for 'request' (the root) — NOT for 'request.user' (the intermediate).
    let e101_names: Vec<_> = diags.iter()
        .filter(|d| d.code == "JINJA-E101")
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        e101_names.iter().all(|m| !m.contains("request.user")),
        "intermediate attribute chain 'request.user' must not fire E101: {e101_names:?}"
    );
}

// ---------- jinja-lsp-qzq6: noqa on opening-delimiter line of multi-line tag --

#[test]
fn noqa_on_opening_delimiter_line_suppresses_inner_diagnostic() {
    // Multi-line {%...%} tag: noqa on line 0 (the opening delimiter line)
    // but the diagnostic is on line 1 (inside the tag body).
    // REQ-DIAG-05: noqa on the opening delimiter line must also suppress.
    let source = "{% set x =  {# noqa: JINJA-E101 #}\n    undefined_var %}";
    let diag = make_diag(1, "JINJA-E101", "undefined-variable");
    let (kept, _w107s) = suppress_by_noqa(&[diag], source);
    assert!(kept.is_empty(),
        "noqa on opening delimiter line 0 must suppress diagnostic on line 1");
}

#[test]
fn noqa_on_diagnostic_line_still_suppresses() {
    // Existing single-line behavior must be unaffected.
    let source = "{% set x = undefined_var %}  {# noqa: JINJA-E101 #}";
    let diag = make_diag(0, "JINJA-E101", "undefined-variable");
    let (kept, _) = suppress_by_noqa(&[diag], source);
    assert!(kept.is_empty(), "noqa on the same line still suppresses");
}

#[test]
fn noqa_bare_all_on_opening_line_suppresses_any_code() {
    let source = "{% set x = {# noqa #}\n    undefined_var %}";
    let diag = make_diag(1, "JINJA-E101", "undefined-variable");
    let (kept, _) = suppress_by_noqa(&[diag], source);
    assert!(kept.is_empty(), "bare noqa on opening line suppresses all codes");
}

#[test]
fn noqa_on_unrelated_earlier_line_does_not_suppress() {
    // noqa on line 0, diagnostic on line 2 — line 0 is NOT the opening delimiter for line 2
    let source = "{# noqa: JINJA-E101 #}\n{% for x in y %}\n    {{ z }}\n{% endfor %}";
    let diag = make_diag(2, "JINJA-E101", "undefined-variable");
    let (kept, _) = suppress_by_noqa(&[diag], source);
    // Line 0 noqa is not on the opening-delimiter line of the for loop (line 1 is),
    // and the for loop does close on the same line. So it must NOT suppress line 2.
    assert_eq!(kept.len(), 1, "noqa on unrelated earlier line must not suppress");
}
