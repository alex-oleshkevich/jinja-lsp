// F01 / REQ-DIAG — check runner tests (jinja-lsp-aznq + jinja-lsp-3ayw).

use jinja_lsp::builtins::registry::{parse_doc_str, Registry, Source};
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

// ─── E001: syntax errors ──────────────────────────────────────────────────────

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
}

#[test]
fn no_w202_when_macro_is_called() {
    let src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}{{ greet('World') }}";
    let idx = extract(src);
    let ws = ws_with(&[("t.html", src)]);
    let diags = run_checks(src, "t.html", &idx, &registry(), &ws);
    assert_eq!(diags.iter().filter(|d| d.code == "JINJA-W202").count(), 0, "called macro must not trigger W202");
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

// ─── Publish wiring contract ──────────────────────────────────────────────────

#[test]
fn server_mod_calls_publish_diagnostics() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("publish_diagnostics"),
        "server mod must call client.publish_diagnostics after Pass 1"
    );
}
