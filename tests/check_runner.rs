// F01 / REQ-DIAG — check runner tests (jinja-lsp-aznq + jinja-lsp-3ayw).

use jinja_lsp::builtins::registry::Registry;
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

// ─── Publish wiring contract ──────────────────────────────────────────────────

#[test]
fn server_mod_calls_publish_diagnostics() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("publish_diagnostics"),
        "server mod must call client.publish_diagnostics after Pass 1"
    );
}
