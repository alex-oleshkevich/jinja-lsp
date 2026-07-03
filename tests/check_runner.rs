// F01 / REQ-DIAG — check runner tests (jinja-lsp-aznq + jinja-lsp-3ayw).

use jinja_lsp::builtins::registry::{parse_doc_str, Registry, Source};
#[allow(unused_imports)]
use jinja_lsp::builtins::registry::AttrDoc;
use jinja_lsp::diagnostic::DiagnosticSeverity;
use jinja_lsp::diagnostics::checks::run_checks;
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

fn registry() -> Registry {
    Registry::load_core()
}

fn ws_with(pairs: &[(&str, &str)]) -> WorkspaceIndex {
    let mut ws = WorkspaceIndex::default();
    for (path, src) in pairs {
        ws.index_inline(path, src);
    }
    ws
}

// ─── E001: syntax errors + cascade guard (jinja-lsp-pta6) ────────────────────

#[test]
fn syntax_error_template_produces_only_e001_no_cascade() {
    // F01 §10: half-typed expression → only E001 fires, no W201 cascade.
    let src = "{% for item in items";  // unclosed — tree-sitter ERROR
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    // If the parser produced errors, all non-E001 diagnostics must be suppressed.
    if !idx.syntax_errors.is_empty() {
        let non_e001: Vec<_> = diags.iter().filter(|d| d.code != "JINJA-E001").collect();
        assert!(
            non_e001.is_empty(),
            "syntax-error template must not emit secondary diagnostics: {:?}",
            non_e001
        );
        assert!(diags.iter().any(|d| d.code == "JINJA-E001"), "E001 must still fire");
    }
}

#[test]
fn e001_emitted_for_syntax_error() {
    let src = "{% if %} unclosed";       // malformed if
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    // If the parser detected a syntax error, emit E001
    if !idx.syntax_errors.is_empty() {
        let e001 = diags.iter().find(|d| d.code == "JINJA-E001");
        assert!(e001.is_some(), "E001 must be emitted for syntax errors");
        assert_eq!(e001.unwrap().severity, DiagnosticSeverity::Error);
    }
}

#[test]
fn no_e001_for_valid_template() {
    let src = "{% for i in items %}{{ i }}{% endfor %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e001 = diags.iter().filter(|d| d.code == "JINJA-E001").count();
    assert_eq!(e001, 0, "valid template must produce no E001");
}

// ─── E102: undefined filter ───────────────────────────────────────────────────

#[test]
fn e102_emitted_for_undefined_filter() {
    let src = "{{ name | totally_fake_filter_xyz }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e102 = diags.iter().find(|d| d.code == "JINJA-E102");
    assert!(e102.is_some(), "E102 must be emitted for undefined filter");
    assert!(e102.unwrap().message.contains("totally_fake_filter_xyz"), "message must name the filter");
}

#[test]
fn no_e102_for_builtin_filter() {
    let src = "{{ name | upper }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e102 = diags.iter().filter(|d| d.code == "JINJA-E102").count();
    assert_eq!(e102, 0, "builtin filter 'upper' must not trigger E102");
}

#[test]
fn no_e101_cascade_for_undefined_filter() {
    // F01 §10: an undefined filter must produce exactly E102, never an additional E101.
    // The filter name is captured as both @identifier and @filter by tree-sitter;
    // check_e101 must skip positions already tagged as Filter.
    let src = r#"{{ "hello" | totally_fake_filter_xyz }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e102_count = diags.iter().filter(|d| d.code == "JINJA-E102").count();
    let e101_count = diags.iter().filter(|d| d.code == "JINJA-E101").count();
    assert_eq!(e102_count, 1, "exactly one E102 must be emitted: {diags:?}");
    assert_eq!(e101_count, 0, "E101 must NOT cascade from an undefined filter: {diags:?}");
}

#[test]
fn no_e101_cascade_for_undefined_filter_after_variable() {
    // Same as above but filter applied to a real variable — that variable's E101
    // must still fire for `name`, but NOT for the filter name.
    let src = "{{ name | totally_fake_filter_xyz }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e101_names: Vec<&str> = diags.iter()
        .filter(|d| d.code == "JINJA-E101")
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        !e101_names.iter().any(|m| m.contains("totally_fake_filter_xyz")),
        "filter name must not cascade to E101: {diags:?}"
    );
}

// ─── E104: undefined test ─────────────────────────────────────────────────────

#[test]
fn e104_emitted_for_undefined_test() {
    let src = "{% if x is totally_fake_test_xyz %}yes{% endif %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e104 = diags.iter().find(|d| d.code == "JINJA-E104");
    assert!(e104.is_some(), "E104 must be emitted for undefined test");
    assert!(e104.unwrap().message.contains("totally_fake_test_xyz"), "message must name the test");
}

#[test]
fn no_e104_for_builtin_test() {
    let src = "{% if x is defined %}yes{% endif %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e104 = diags.iter().filter(|d| d.code == "JINJA-E104").count();
    assert_eq!(e104, 0, "builtin test 'defined' must not trigger E104");
}

// ─── W301: duplicate block ────────────────────────────────────────────────────

#[test]
fn w301_emitted_for_duplicate_block() {
    let src = "{% block foo %}a{% endblock %}{% block foo %}b{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w301 = diags.iter().find(|d| d.code == "JINJA-W301");
    assert!(w301.is_some(), "W301 must be emitted for duplicate block name");
    assert!(w301.unwrap().message.contains("foo"), "message must name the duplicate block");
}

#[test]
fn jinja_lsp_v43s_w301_emitted_for_third_duplicate_block() {
    // jinja-lsp-v43s: a name defined 3+ times must be flagged on every occurrence
    // after the first, not just the 2nd.
    let src = "{% block foo %}a{% endblock %}{% block foo %}b{% endblock %}{% block foo %}c{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w301_count = diags.iter().filter(|d| d.code == "JINJA-W301").count();
    assert_eq!(w301_count, 2, "the 2nd and 3rd occurrences must both be flagged: {diags:?}");
}

#[test]
fn no_w301_for_unique_blocks() {
    let src = "{% block header %}a{% endblock %}{% block footer %}b{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W301").count(), 0);
}

// ─── W302: duplicate macro ────────────────────────────────────────────────────

#[test]
fn w302_emitted_for_duplicate_macro() {
    let src = "{% macro render() %}a{% endmacro %}{% macro render() %}b{% endmacro %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w302 = diags.iter().find(|d| d.code == "JINJA-W302");
    assert!(w302.is_some(), "W302 must be emitted for duplicate macro name");
    assert!(w302.unwrap().message.contains("render"), "message must name the duplicate macro");
}

#[test]
fn jinja_lsp_v43s_w302_emitted_for_third_duplicate_macro() {
    let src = "{% macro render() %}a{% endmacro %}{% macro render() %}b{% endmacro %}{% macro render() %}c{% endmacro %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w302_count = diags.iter().filter(|d| d.code == "JINJA-W302").count();
    assert_eq!(w302_count, 2, "the 2nd and 3rd occurrences must both be flagged: {diags:?}");
}

// ─── W201: unused-variable ───────────────────────────────────────────────────

#[test]
fn w201_emitted_for_set_variable_never_used() {
    let src = "{% set x = 1 %}hello world";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w201 = diags.iter().find(|d| d.code == "JINJA-W201");
    assert!(w201.is_some(), "W201 must be emitted for unused set variable: {diags:?}");
    assert!(w201.unwrap().message.contains('x'), "message must name the unused variable");
}

#[test]
fn no_w201_when_variable_is_used() {
    let src = "{% set x = 1 %}{{ x }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W201").count(), 0, "used variable must not trigger W201");
}

// ─── W202: unused-macro ───────────────────────────────────────────────────────

#[test]
fn w202_emitted_for_macro_never_called() {
    let src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}nothing here";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w202 = diags.iter().find(|d| d.code == "JINJA-W202");
    assert!(w202.is_some(), "W202 must fire for locally-unused macro: {diags:?}");
    assert!(w202.unwrap().message.contains("greet"), "message must name the macro");
    // jinja-lsp-l6hk: the check treats a macro as used if called/imported from ANY
    // template in the workspace, so the message must not tell users to look only
    // in "this template" — that's wrong guidance for a cross-file check.
    assert!(
        !w202.unwrap().message.contains("in this template"),
        "message must not claim the check is scoped to the current template: {:?}", w202.unwrap()
    );
    assert!(
        w202.unwrap().message.contains("workspace"),
        "message must clarify the check is workspace-wide: {:?}", w202.unwrap()
    );
}

#[test]
fn no_w202_when_macro_is_called() {
    let src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}{{ greet('World') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W202").count(), 0, "called macro must not trigger W202");
}

// ─── jinja-lsp-vpd6: W202 is cross-file (Pass 2) ────────────────────────────

#[test]
fn no_w202_when_macro_called_from_another_template() {
    let macro_src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let caller_src = r#"{% from "macros.html" import greet %}{{ greet("World") }}"#;
    let ws = ws_with(&[("macros.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let diags = run_checks(macro_src, "macros.html", &idx, &registry(), &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-W202").count(), 0,
        "macro used via from-import in another template must not trigger W202: {diags:?}"
    );
}

#[test]
fn no_w202_when_macro_imported_but_not_called() {
    // If another template imports the macro but doesn't call it, W202 should still suppress
    // (the import itself constitutes "exporting" the macro).
    let macro_src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let importer_src = r#"{% from "macros.html" import greet %}"#;
    let ws = ws_with(&[("macros.html", macro_src), ("importer.html", importer_src)]);
    let idx = extract(macro_src);
    let diags = run_checks(macro_src, "macros.html", &idx, &registry(), &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-W202").count(), 0,
        "imported macro must not trigger W202 even if not called in the importer: {diags:?}"
    );
}

// ─── W203: unused-import ──────────────────────────────────────────────────────

#[test]
fn w203_emitted_for_unused_import_alias() {
    let src = r#"{% import "macros.html" as m %}hello"#;
    let idx = extract(src);
    let ws = ws_with(&[("macros.html", "{% macro fn() %}{% endmacro %}")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w203 = diags.iter().find(|d| d.code == "JINJA-W203");
    assert!(w203.is_some(), "W203 must fire for unused import alias: {diags:?}");
    assert!(w203.unwrap().message.contains('m'), "message must name the alias");
}

#[test]
fn no_w203_when_import_alias_is_used() {
    let src = r#"{% import "macros.html" as m %}{{ m.fn() }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("macros.html", "{% macro fn() %}{% endmacro %}")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W203").count(), 0, "used import alias must not trigger W203");
}

#[test]
fn w203_emitted_for_unused_from_import() {
    let src = r#"{% from "macros.html" import post_url %}hello"#;
    let idx = extract(src);
    let ws = ws_with(&[("macros.html", "{% macro post_url() %}{% endmacro %}")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w203 = diags.iter().find(|d| d.code == "JINJA-W203");
    assert!(w203.is_some(), "W203 must fire for unused from-import: {diags:?}");
    assert!(w203.unwrap().message.contains("post_url"), "message must name the unused name");
}

// ─── W303: duplicate-import-alias ────────────────────────────────────────────

#[test]
fn w303_emitted_when_same_alias_imported_twice() {
    let src = r#"{% import "a.html" as m %}{% import "b.html" as m %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("a.html", ""), ("b.html", "")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w303 = diags.iter().find(|d| d.code == "JINJA-W303");
    assert!(w303.is_some(), "W303 must be emitted when same alias is used twice: {diags:?}");
    assert!(w303.unwrap().message.contains('m'), "message must name the duplicate alias");
}

#[test]
fn jinja_lsp_v43s_w303_emitted_for_third_duplicate_alias() {
    let src = r#"{% import "a.html" as m %}{% import "b.html" as m %}{% import "c.html" as m %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("a.html", ""), ("b.html", ""), ("c.html", "")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w303_count = diags.iter().filter(|d| d.code == "JINJA-W303").count();
    assert_eq!(w303_count, 2, "the 2nd and 3rd occurrences must both be flagged: {diags:?}");
}

#[test]
fn no_w303_when_aliases_are_distinct() {
    let src = r#"{% import "a.html" as m1 %}{% import "b.html" as m2 %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("a.html", ""), ("b.html", "")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W303").count(), 0, "distinct aliases must not trigger W303");
}

// ─── W304: duplicate-from-import ─────────────────────────────────────────────

#[test]
fn w304_emitted_when_same_name_imported_twice() {
    let src = r#"{% from "a.html" import foo %}{% from "b.html" import foo %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("a.html", "{% macro foo() %}{% endmacro %}"), ("b.html", "{% macro foo() %}{% endmacro %}")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w304 = diags.iter().find(|d| d.code == "JINJA-W304");
    assert!(w304.is_some(), "W304 must be emitted for duplicate from-import name: {diags:?}");
    assert!(w304.unwrap().message.contains("foo"), "message must name the duplicate: {:?}", w304);
}

#[test]
fn jinja_lsp_v43s_w304_emitted_for_third_duplicate_name() {
    let src = r#"{% from "a.html" import foo %}{% from "b.html" import foo %}{% from "c.html" import foo %}"#;
    let idx = extract(src);
    let ws = ws_with(&[
        ("a.html", "{% macro foo() %}{% endmacro %}"),
        ("b.html", "{% macro foo() %}{% endmacro %}"),
        ("c.html", "{% macro foo() %}{% endmacro %}"),
    ]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w304_count = diags.iter().filter(|d| d.code == "JINJA-W304").count();
    assert_eq!(w304_count, 2, "the 2nd and 3rd occurrences must both be flagged: {diags:?}");
}

#[test]
fn no_w304_when_names_are_distinct() {
    let src = r#"{% from "a.html" import foo %}{% from "b.html" import bar %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("a.html", "{% macro foo() %}{% endmacro %}"), ("b.html", "{% macro bar() %}{% endmacro %}")]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W304").count(), 0, "distinct names must not trigger W304");
}

// ─── W305: name-shadowing ─────────────────────────────────────────────────────

#[test]
fn w305_emitted_when_inner_var_shadows_outer() {
    // for-loop var `x` shadows set var `x` from outer scope.
    let src = "{% set x = 1 %}{% for x in items %}{{ x }}{% endfor %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let w305 = diags.iter().find(|d| d.code == "JINJA-W305");
    assert!(w305.is_some(), "W305 must be emitted when inner var shadows outer: {diags:?}");
    assert!(w305.unwrap().message.contains('x'), "message must name the shadowing variable: {:?}", w305);
}

#[test]
fn no_w305_when_no_shadowing() {
    let src = "{% set x = 1 %}{% for y in items %}{{ y }}{% endfor %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W305").count(), 0, "distinct names must not trigger W305");
}

// ─── E403: missing-required-block ─────────────────────────────────────────────

#[test]
fn e403_emitted_when_required_block_not_overridden() {
    // base.html has `{% block title required %}{% endblock %}` — child doesn't override it.
    let child = r#"{% extends "base.html" %}{% block content %}hello{% endblock %}"#;
    let base = "{% block title required %}{% endblock %}{% block content %}{% endblock %}";
    let idx = extract(child);
    let ws = ws_with(&[("base.html", base)]);
    let diags = run_checks(child, "child.html", &idx, &registry(), &ws);
    let e403 = diags.iter().find(|d| d.code == "JINJA-E403");
    assert!(e403.is_some(), "E403 must fire when required block is missing: {diags:?}");
    assert!(e403.unwrap().message.contains("title"), "message must name the missing block");
}

#[test]
fn no_e403_when_required_block_is_overridden() {
    let child = r#"{% extends "base.html" %}{% block title %}My page{% endblock %}{% block content %}hello{% endblock %}"#;
    let base = "{% block title required %}{% endblock %}{% block content %}{% endblock %}";
    let idx = extract(child);
    let ws = ws_with(&[("base.html", base)]);
    let diags = run_checks(child, "child.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E403").count(), 0, "overridden required block must not trigger E403");
}

#[test]
fn no_e403_for_non_extends_template() {
    let src = "{% block title required %}default{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E403").count(), 0, "non-extends template must not trigger E403");
}

// ─── W402: unreachable-content ────────────────────────────────────────────────

#[test]
fn w402_set_outside_block_in_extends_template() {
    let src = r#"{% extends "base.html" %}{% set x = 1 %}{% block content %}{% endblock %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let w402 = diags.iter().find(|d| d.code == "JINJA-W402");
    assert!(w402.is_some(), "W402 must be emitted for set outside block in extends template: {diags:?}");
}

#[test]
fn no_w402_for_non_extends_template() {
    let src = "{% set x = 1 %}{{ x }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W402").count(), 0, "non-extends template must not trigger W402");
}

#[test]
fn jinja_lsp_p3o7_no_w402_for_set_inside_top_level_macro_in_extends_template() {
    // A top-level {% macro %} is valid Jinja (callable from blocks) — {% set %}/{% for %}
    // bindings inside its body are reachable through a macro call, not directly rendered
    // like top-level content, so they must be exempt from W402 the same way block bodies are.
    let src = r#"{% extends "base.html" %}
{% macro greet(name) %}{% set msg = "hi" %}{{ msg }} {{ name }}{% endmacro %}
{% block content %}{{ greet("world") }}{% endblock %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let w402: Vec<_> = diags.iter().filter(|d| d.code == "JINJA-W402").collect();
    assert!(w402.is_empty(), "set inside a top-level macro body must not trigger W402: {w402:?}");
}

// ─── E401: invalid-super ──────────────────────────────────────────────────────

#[test]
fn e401_super_outside_block() {
    let src = r#"{% extends "base.html" %}{{ super() }}{% block content %}{% endblock %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let e401 = diags.iter().find(|d| d.code == "JINJA-E401");
    assert!(e401.is_some(), "E401 must be emitted for super() outside block: {diags:?}");
}

#[test]
fn no_e401_super_inside_block() {
    let src = r#"{% extends "base.html" %}{% block content %}{{ super() }}{% endblock %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}base{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E401").count(), 0, "super() inside block must not trigger E401");
}

#[test]
fn jinja_lsp_96oh_no_e401_for_literal_super_text_outside_jinja() {
    // E401 scanned the raw source for the literal bytes "super()" with no word-boundary
    // check and no check that the match sits inside a Jinja expression — HTML prose,
    // a Jinja comment, and an identifier merely ending in "super()" all false-positived.
    let src = "{% extends \"base.html\" %}\n\
        <p>call super() here</p>\n\
        {# TODO: super() #}\n\
        <script>mysuper()</script>\n\
        {% block content %}{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let e401: Vec<_> = diags.iter().filter(|d| d.code == "JINJA-E401").collect();
    assert!(e401.is_empty(), "literal 'super()' text outside a Jinja expression must not trigger E401: {e401:?}");
}

#[test]
fn jinja_lsp_96oh_e401_col_is_byte_col_not_char_col_on_multibyte_line() {
    // Every other check reports byte columns from tree-sitter spans; E401's
    // byte_to_line_col counted chars instead, so the reported column drifted on
    // lines with multibyte text before the match.
    let src = "{% extends \"base.html\" %}\n\
        café {{ super() }}\n\
        {% block content %}{% endblock %}";
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let e401 = diags.iter().find(|d| d.code == "JINJA-E401").expect("E401 must fire for super() outside block");
    // "café " is 5 chars / 6 bytes (é is 2 bytes) — "{{ super" starts at byte 6, char 5.
    let expected_byte_col = src.lines().nth(1).unwrap().find("super").unwrap() as u32;
    assert_eq!(e401.col, expected_byte_col, "E401 col must be a byte offset, not a char offset: {e401:?}");
}

// ─── E601: template-does-not-exist ───────────────────────────────────────────

#[test]
fn e601_emitted_for_extends_with_unknown_path() {
    let src = r#"{% extends "ghost.html" %}"#;
    let idx = extract(src);
    let ws = WorkspaceIndex::default(); // ghost.html not in workspace
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    let e601 = diags.iter().find(|d| d.code == "JINJA-E601");
    assert!(e601.is_some(), "E601 must be emitted for unknown extends path");
    assert!(e601.unwrap().message.contains("ghost.html"), "message must name the missing template");
}

#[test]
fn no_e601_for_known_extends_path() {
    let src = r#"{% extends "base.html" %}"#;
    let idx = extract(src);
    let ws = ws_with(&[("base.html", "{% block content %}{% endblock %}")]);
    let diags = run_checks(src, "child.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E601").count(), 0);
}

// ─── jinja-lsp-1i26: workspace key contract (abs keys ↔ relative refs) ───────

#[test]
fn no_e601_for_valid_extends_with_abs_keyed_workspace() {
    // Simulates the server path: build_workspace_abs keys templates by absolute path.
    // Template references use relative paths ("base.html").
    // E601 must NOT fire when the referred template exists in the workspace.
    use std::fs;
    use tempfile::TempDir;
    use jinja_lsp::workspace::build_workspace_abs;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("base.html"), "{% block content %}{% endblock %}").unwrap();
    fs::write(dir.path().join("child.html"), r#"{% extends "base.html" %}"#).unwrap();

    let ws = build_workspace_abs(&[dir.path()], &["html"]);
    let child_abs = dir.path().join("child.html").to_string_lossy().into_owned();
    let source = fs::read_to_string(&child_abs).unwrap();
    let idx = extract(&source);

    let diags = run_checks(&source, &child_abs, &idx, &registry(), &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-E601").count(), 0,
        "E601 must not fire for a valid extends when workspace uses absolute keys: {diags:?}"
    );
}

#[test]
fn no_e404_for_non_cyclic_extends_with_abs_keyed_workspace() {
    use std::fs;
    use tempfile::TempDir;
    use jinja_lsp::workspace::build_workspace_abs;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("base.html"), "{% block content %}{% endblock %}").unwrap();
    fs::write(dir.path().join("child.html"), r#"{% extends "base.html" %}"#).unwrap();

    let ws = build_workspace_abs(&[dir.path()], &["html"]);
    let child_abs = dir.path().join("child.html").to_string_lossy().into_owned();
    let source = fs::read_to_string(&child_abs).unwrap();
    let idx = extract(&source);

    let diags = run_checks(&source, &child_abs, &idx, &registry(), &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-E404").count(), 0,
        "E404 must not fire for non-cyclic extends with abs-keyed workspace: {diags:?}"
    );
}

// ─── E101: undefined-variable ────────────────────────────────────────────────

fn registry_with_context_var(name: &str) -> Registry {
    let mut reg = Registry::load_core();
    let src = format!("---\nname: {name}\ncategory: context_variable\n---\nA hinted variable.");
    if let Some((entry, _)) = parse_doc_str(&src, Source::Hint) {
        reg.insert(entry);
    }
    reg
}

fn registry_with_scoped_context_var(name: &str, template: &str) -> Registry {
    let mut reg = Registry::load_core();
    let src = format!("---\nname: {name}\ncategory: context_variable\ntemplate: {template}\n---\nA hinted variable.");
    if let Some((entry, _)) = parse_doc_str(&src, Source::Hint) {
        reg.insert(entry);
    }
    reg
}

#[test]
fn e101_emitted_for_undefined_identifier() {
    let src = "{{ totally_undefined_xyz }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e101 = diags.iter().find(|d| d.code == "JINJA-E101");
    assert!(e101.is_some(), "E101 must fire for an undefined identifier");
    assert!(e101.unwrap().message.contains("totally_undefined_xyz"));
}

#[test]
fn no_e101_for_locally_set_variable() {
    let src = "{% set x = 1 %}{{ x }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "locally-set variable must not trigger E101");
}

#[test]
fn no_e101_for_for_loop_variable() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "for-loop variable must not trigger E101");
}

#[test]
fn no_e101_for_jinja2_global_variable() {
    // `loop` is a Jinja2 built-in variable (Category::Variable in core registry)
    let src = "{% for i in items %}{{ loop.index }}{% endfor %}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "Jinja2 global 'loop' must not trigger E101");
}

#[test]
fn no_e101_for_hinted_context_variable() {
    // REQ-HINT-04: a ContextVariable hint suppresses E101
    let src = "{{ post }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var("post");
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "hinted context_variable must suppress E101 (REQ-HINT-04)");
}

#[test]
fn no_e101_for_scoped_hint_matching_template() {
    // REQ-HINT-04: template-scoped hint suppresses in that template
    let src = "{{ user }}";
    let idx = extract(src);
    let ws = ws_with(&[("detail.html", src)]);
    let reg = registry_with_scoped_context_var("user", "detail.html");
    let diags = run_checks(src, "detail.html", &idx, &reg, &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "template-scoped hint matching template must suppress E101");
}

#[test]
fn e101_for_scoped_hint_not_matching_template() {
    // REQ-HINT-04: template-scoped hint must NOT suppress in other templates
    let src = "{{ user }}";
    let idx = extract(src);
    let ws = ws_with(&[("other.html", src)]);
    let reg = registry_with_scoped_context_var("user", "detail.html");
    let diags = run_checks(src, "other.html", &idx, &reg, &ws);
    let e101 = diags.iter().find(|d| d.code == "JINJA-E101");
    assert!(e101.is_some(), "template-scoped hint for 'detail.html' must not suppress E101 in 'other.html'");
}

#[test]
fn no_e101_for_import_alias() {
    let src = r#"{% import "macros.html" as m %}{{ m.foo }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "import alias must not trigger E101");
}

#[test]
fn no_e101_for_local_macro_name() {
    let src = "{% macro greet() %}hi{% endmacro %}{{ greet }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "local macro name used as identifier must not trigger E101");
}

// ─── W106: unknown-attribute ─────────────────────────────────────────────────

fn registry_with_context_var_attrs(name: &str, attrs: &[&str]) -> Registry {
    let attrs_yaml = attrs.iter().map(|a| format!("  - name: {a}")).collect::<Vec<_>>().join("\n");
    let src = format!(
        "---\nname: {name}\ncategory: context_variable\nattributes:\n{attrs_yaml}\n---\nA hinted variable."
    );
    let mut reg = Registry::load_core();
    if let Some((entry, attr_docs)) = parse_doc_str(&src, Source::Hint) {
        reg.insert(entry);
        for a in attr_docs {
            reg.insert_attr(a);
        }
    }
    reg
}

#[test]
fn w106_emitted_for_unknown_attribute_on_hinted_var() {
    // post has attrs [title, slug] — post.autor is a typo → W106
    let src = "{{ post.autor }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    let w106 = diags.iter().find(|d| d.code == "JINJA-W106");
    assert!(w106.is_some(), "W106 must fire for an unknown attribute on a hinted context_variable");
    assert!(w106.unwrap().message.contains("autor"), "message must name the unknown attribute");
}

#[test]
fn no_w106_for_known_attribute() {
    let src = "{{ post.title }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W106").count(), 0,
        "known attribute must not trigger W106");
}

#[test]
fn no_w106_when_no_attrs_declared() {
    // context_variable with no attributes list → W106 never fires (empty list = "I haven't declared")
    let src = "{{ post.title }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var("post"); // no attrs
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W106").count(), 0,
        "variable with no attrs declaration must not trigger W106");
}

#[test]
fn no_w106_for_non_hinted_variable() {
    // post not in registry at all → W106 cannot fire
    let src = "{{ post.title }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W106").count(), 0,
        "non-hinted variable must not trigger W106");
}

#[test]
fn w106_is_off_by_default_in_filter() {
    use jinja_lsp::diagnostics::filter_by_config;
    let src = "{{ post.autor }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let raw_diags = run_checks(src, "t.html", &idx, &reg, &ws);
    // With empty select (default), W106 is suppressed
    let filtered = filter_by_config(&raw_diags, &[], &[]);
    assert!(filtered.iter().all(|d| d.code != "JINJA-W106"),
        "W106 must be filtered out by default when select is empty");
    // With explicit select, W106 appears
    let filtered_selected = filter_by_config(&raw_diags, &["JINJA-W106"], &[]);
    assert!(filtered_selected.iter().any(|d| d.code == "JINJA-W106"),
        "W106 must appear when explicitly selected");
}

// ─── W106: template-scope fix (jinja-lsp-o45w) ───────────────────────────────

fn registry_with_scoped_context_var_attrs(name: &str, template: &str, attrs: &[&str]) -> Registry {
    let attrs_yaml = attrs.iter().map(|a| format!("  - name: {a}")).collect::<Vec<_>>().join("\n");
    let src = format!(
        "---\nname: {name}\ncategory: context_variable\ntemplate: {template}\nattributes:\n{attrs_yaml}\n---\nA scoped hinted variable."
    );
    let mut reg = Registry::load_core();
    if let Some((entry, attr_docs)) = parse_doc_str(&src, Source::Hint) {
        reg.insert(entry);
        for a in attr_docs { reg.insert_attr(a); }
    }
    reg
}

#[test]
fn w106_scoped_hint_fires_only_in_scoped_template() {
    // post scoped to "detail.html" — W106 must fire in detail.html but NOT in "other.html".
    let src = "{{ post.autor }}";
    let idx = extract(src);
    let ws = ws_with(&[("detail.html", src)]);
    let reg = registry_with_scoped_context_var_attrs("post", "detail.html", &["title", "slug"]);

    let diags_scoped = run_checks(src, "detail.html", &idx, &reg, &ws);
    assert!(
        diags_scoped.iter().any(|d| d.code == "JINJA-W106"),
        "W106 must fire in the scoped template"
    );

    let diags_other = run_checks(src, "other.html", &idx, &reg, &ws);
    assert!(
        diags_other.iter().all(|d| d.code != "JINJA-W106"),
        "W106 must NOT fire in a different template when the hint is scoped"
    );
}

// ─── W106: subscript access (jinja-lsp-4x6i) ─────────────────────────────────

#[test]
fn w106_fires_for_subscript_unknown_attribute() {
    // post["autor"] should trigger W106 the same as post.autor
    let src = r#"{{ post["autor"] }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert!(
        diags.iter().any(|d| d.code == "JINJA-W106"),
        "W106 must fire for subscript access with an unknown attribute"
    );
}

#[test]
fn no_w106_for_subscript_known_attribute() {
    let src = r#"{{ post["title"] }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-W106").count(), 0,
        "known attribute via subscript must not trigger W106"
    );
}

#[test]
fn w106_subscript_single_quote_key() {
    let src = "{{ post['autor'] }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title", "slug"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert!(
        diags.iter().any(|d| d.code == "JINJA-W106"),
        "W106 must fire for single-quoted subscript access with an unknown attribute"
    );
}

#[test]
fn jinja_lsp_l27o_subscript_scan_ignores_html_and_script_text() {
    // session["user"] inside a <script> block (plain host text, not Jinja) must
    // never trigger W106 — the scan used to cover the entire file text, not just
    // {{ }}/{% %} regions.
    let src = r#"<script>var x = session["user"];</script>{{ post["title"] }}"#;
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("session", &["id"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-W106" && d.message.contains("session")).count(), 0,
        "subscript access inside plain HTML/JS text must not trigger W106: {diags:?}"
    );
}

#[test]
fn jinja_lsp_l27o_subscript_position_correct_on_second_line() {
    // The key's reported line/col must be correct regardless of where in the
    // file the match occurs — the old rescan-from-0 approach was O(n^2) but at
    // least always recomputed from a fixed origin; this checks the new
    // incremental tracker computes the same correct position.
    let src = "line one\n{{ post[\"autor\"] }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let reg = registry_with_context_var_attrs("post", &["title"]);
    let diags = run_checks(src, "t.html", &idx, &reg, &ws);
    let d = diags.iter().find(|d| d.code == "JINJA-W106").expect("W106 must fire");
    assert_eq!(d.line, 1, "must report line 1 (0-indexed), not line 0");
    let expected_col = src.lines().nth(1).unwrap().find("autor").unwrap() as u32;
    assert_eq!(d.col, expected_col, "column must point at the key content on line 1");
}

// ─── E501: wrong-call-args ───────────────────────────────────────────────────

#[test]
fn e501_too_few_required_args() {
    // greet(name) requires 1 arg; called with 0 → E501
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet() }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e501 = diags.iter().find(|d| d.code == "JINJA-E501");
    assert!(e501.is_some(), "E501 must fire when required args are missing");
    assert!(e501.unwrap().message.contains("greet"), "message must name the callee");
}

#[test]
fn e501_too_many_positional_args() {
    // greet(name) has 1 param; called with 2 → E501
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet('a', 'b') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e501 = diags.iter().find(|d| d.code == "JINJA-E501");
    assert!(e501.is_some(), "E501 must fire when too many positional args are passed");
}

#[test]
fn no_e501_for_correct_positional_args() {
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet('world') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E501").count(), 0,
        "correct arg count must not trigger E501");
}

#[test]
fn no_e501_for_optional_args_omitted() {
    // greet(name, title=None) — title has a default, so calling with just 1 arg is fine
    let src = "{% macro greet(name, title='') %}hi {{ name }}{% endmacro %}{{ greet('world') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E501").count(), 0,
        "omitting optional args must not trigger E501");
}

#[test]
fn e501_unknown_keyword_arg() {
    // greet(name) — calling with greet(title='world') is an unknown keyword
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet(title='world') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e501 = diags.iter().find(|d| d.code == "JINJA-E501");
    assert!(e501.is_some(), "E501 must fire for unknown keyword argument");
    assert!(e501.unwrap().message.contains("title"), "message must name the unknown keyword");
}

#[test]
fn no_e501_for_valid_keyword_arg() {
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet(name='world') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E501").count(), 0,
        "valid keyword arg must not trigger E501");
}

// ─── E404: recursive-import ──────────────────────────────────────────────────

#[test]
fn e404_emitted_for_direct_cycle() {
    // a.html imports b.html, b.html imports a.html — a cycle
    let a_src = r#"{% import "b.html" as b %}"#;
    let b_src = r#"{% import "a.html" as a %}"#;
    let idx_a = extract(a_src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("a.html", a_src);
    ws.index_inline("b.html", b_src);
    let diags = run_checks(a_src, "a.html", &idx_a, &registry(), &ws);
    let e404 = diags.iter().find(|d| d.code == "JINJA-E404");
    assert!(e404.is_some(), "E404 must fire when a.html and b.html import each other");
    assert!(e404.unwrap().message.contains("b.html"), "message must name the cyclic target");
}

#[test]
fn e404_emitted_for_indirect_cycle() {
    // a → b → c → a
    let a_src = r#"{% import "b.html" as b %}"#;
    let b_src = r#"{% import "c.html" as c %}"#;
    let c_src = r#"{% import "a.html" as a %}"#;
    let idx_a = extract(a_src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("a.html", a_src);
    ws.index_inline("b.html", b_src);
    ws.index_inline("c.html", c_src);
    let diags = run_checks(a_src, "a.html", &idx_a, &registry(), &ws);
    assert!(diags.iter().any(|d| d.code == "JINJA-E404"),
        "E404 must fire for indirect cycle a→b→c→a");
}

#[test]
fn no_e404_for_non_cyclic_import() {
    let a_src = r#"{% import "b.html" as b %}"#;
    let b_src = "{% macro foo() %}hi{% endmacro %}";
    let idx_a = extract(a_src);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("a.html", a_src);
    ws.index_inline("b.html", b_src);
    let diags = run_checks(a_src, "a.html", &idx_a, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E404").count(), 0,
        "non-cyclic import must not trigger E404");
}

#[test]
fn no_e404_for_extends_no_cycle() {
    let child = r#"{% extends "base.html" %}{% block content %}hi{% endblock %}"#;
    let base = "{% block content %}{% endblock %}";
    let idx = extract(child);
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("child.html", child);
    ws.index_inline("base.html", base);
    let diags = run_checks(child, "child.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E404").count(), 0,
        "linear extends chain must not trigger E404");
}

// ─── E103: undefined-function ────────────────────────────────────────────────

#[test]
fn e103_emitted_for_undefined_function_call() {
    let src = "{{ totally_fake_fn_xyz() }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    let e103 = diags.iter().find(|d| d.code == "JINJA-E103");
    assert!(e103.is_some(), "E103 must fire for an undefined function call");
    assert!(e103.unwrap().message.contains("totally_fake_fn_xyz"));
}

#[test]
fn no_e103_for_builtin_jinja2_function() {
    let src = "{{ range(10) }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E103").count(), 0,
        "built-in Jinja2 function 'range' must not trigger E103");
}

#[test]
fn no_e103_for_local_macro_call() {
    let src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}{{ greet('world') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E103").count(), 0,
        "local macro call must not trigger E103");
}

#[test]
fn no_e103_for_from_imported_macro_call() {
    let macro_src = "{% macro greet(name) %}hi {{ name }}{% endmacro %}";
    let child_src = r#"{% from "macros.html" import greet %}{{ greet("world") }}"#;
    let idx = extract(child_src);
    let ws = ws_with(&[("macros.html", macro_src), ("child.html", child_src)]);
    let diags = run_checks(child_src, "child.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-E103").count(), 0,
        "from-imported macro call must not trigger E103");
}

// ─── REQ-EXTR-09: block-set variable extraction ──────────────────────────────

#[test]
fn block_set_variable_is_indexed() {
    // {% set nav %}…{% endset %} must produce a VariableDefinition for `nav`.
    let src = "{% set nav %}hello{% endset %}{{ nav }}";
    let idx = extract(src);
    assert!(
        idx.variables.iter().any(|v| v.name == "nav"),
        "block-set variable 'nav' must be indexed; got: {:?}",
        idx.variables.iter().map(|v| &v.name).collect::<Vec<_>>()
    );
}

#[test]
fn block_set_variable_no_e101() {
    // {{ nav }} after {% set nav %}…{% endset %} must not trigger E101.
    let src = "{% set nav %}hello{% endset %}{{ nav }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(
        diags.iter().filter(|d| d.code == "JINJA-E101").count(), 0,
        "block-set variable must not trigger E101; diags: {diags:?}"
    );
}

#[test]
fn multiple_block_set_variables_are_indexed() {
    let src = "{% set a %}x{% endset %}{% set b %}y{% endset %}{{ a }}{{ b }}";
    let idx = extract(src);
    assert!(idx.variables.iter().any(|v| v.name == "a"), "variable 'a' must be indexed");
    assert!(idx.variables.iter().any(|v| v.name == "b"), "variable 'b' must be indexed");
}

#[test]
fn block_set_with_whitespace_control_is_indexed() {
    // {%- set name -%} must be recognized even with whitespace-control modifiers.
    let src = "{%- set nav -%}hello{%- endset -%}{{ nav }}";
    let idx = extract(src);
    assert!(
        idx.variables.iter().any(|v| v.name == "nav"),
        "block-set with whitespace-control modifier must be indexed; got: {:?}",
        idx.variables.iter().map(|v| &v.name).collect::<Vec<_>>()
    );
}

// ─── Publish wiring contract ──────────────────────────────────────────────────

#[test]
fn server_mod_calls_publish_diagnostics() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("publish_diagnostics"),
        "server mod must call client.publish_diagnostics after Pass 1"
    );
}
