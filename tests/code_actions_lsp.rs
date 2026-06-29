// REQ-ACT-09: WorkspaceEdit dispatch — unit tests for the LSP code-action handler path.
// These tests call the feature functions directly (the handler wiring is thin);
// end-to-end LSP wiring is covered by architecture/e2e tests.

use jinja_lsp::features::code_actions::{code_actions, ActionKind};
use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;
use jinja_lsp::builtins::registry::Registry;

fn w_diag(code: &str, msg: &str, line: u32, col: u32) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        slug: code.to_lowercase(),
        message: msg.to_owned(),
        file: "/tpl.html".to_owned(),
        line,
        col,
        severity: DiagnosticSeverity::Warning,
    }
}

// ─── T-01: quick-fix returns inline WorkspaceEdit (no command) ───────────────

#[test]
fn act09_t01_quick_fix_has_workspace_edit() {
    let source = "{% macro foo() %}{% endmacro %}\n{% macro foo() %}{% endmacro %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let reg = Registry::load_core();

    let diag = w_diag("JINJA-W302", "Duplicate macro 'foo'", 1, 0);
    let actions = code_actions(source, "/tpl.html", &[diag], &idx, &ws, &reg);

    assert!(!actions.is_empty(), "expected at least one action for W302");
    let action = &actions[0];
    // REQ-ACT-09: direct fixes carry an inline WorkspaceEdit, not a command.
    assert!(action.edit.is_some(), "quick-fix must have inline WorkspaceEdit");
    assert_eq!(action.kind, ActionKind::QuickFix);
}

// ─── T-02: empty diagnostics → no actions ────────────────────────────────────

#[test]
fn act09_t02_empty_diagnostics_returns_empty() {
    let source = "{{ x }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let reg = Registry::load_core();

    let actions = code_actions(source, "/tpl.html", &[], &idx, &ws, &reg);
    assert!(actions.is_empty());
}

// ─── T-03: unknown diagnostic code → no actions ──────────────────────────────

#[test]
fn act09_t03_unknown_code_returns_empty() {
    let source = "{{ x }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let reg = Registry::load_core();

    let diag = w_diag("JINJA-Z999", "Unknown", 0, 0);
    let actions = code_actions(source, "/tpl.html", &[diag], &idx, &ws, &reg);
    assert!(actions.is_empty());
}

// ─── T-03b: create-template action carries create_files entry ────────────────

#[test]
fn act09_t03b_create_template_action_has_create_files() {
    use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
    let source = "{% extends 'missing/base.html' %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let reg = Registry::load_core();

    let diag = Diagnostic {
        code: "JINJA-E601".to_owned(),
        slug: "jinja-e601".to_owned(),
        message: "Template 'missing/base.html' not found".to_owned(),
        file: "/tpl.html".to_owned(),
        line: 0,
        col: 0,
        severity: DiagnosticSeverity::Error,
    };
    let actions = code_actions(source, "/tpl.html", &[diag], &idx, &ws, &reg);
    let create_action = actions.iter().find(|a| a.title.contains("Create template"));
    assert!(create_action.is_some(), "E101 should offer a 'Create template' action");
    let edit = create_action.unwrap().edit.as_ref().unwrap();
    assert!(!edit.create_files.is_empty(), "create-template action must populate create_files");
    assert_eq!(edit.create_files[0].0, "missing/base.html");
}

// ─── T-03c: to_lsp_action emits Create before Edit in document_changes ───────

#[test]
fn act09_t03c_workspace_edit_create_before_text_edit_in_source() {
    // Static source-check: verify the fix is in place by reading server/mod.rs.
    let src = include_str!("../src/server/mod.rs");
    // After the fix, Create ops are pushed first; the changes loop comes after.
    // The "for (path, _content) in we.create_files" pattern (bug) must be gone.
    assert!(
        !src.contains("for (path, _content) in we.create_files"),
        "dropped-content bug must be removed: 'for (path, _content)' found in server/mod.rs"
    );
    // The CreateFile block must appear before the changes iteration.
    let create_pos = src.find("ResourceOp::Create").unwrap_or(usize::MAX);
    let changes_pos = src.rfind("for (path, edits) in we.changes").unwrap_or(usize::MAX);
    assert!(
        create_pos < changes_pos,
        "ResourceOp::Create must appear before the 'we.changes' loop in document_changes builder"
    );
}

// ─── T-04: W301 duplicate block produces a quick-fix with WorkspaceEdit ──────

#[test]
fn act09_t04_duplicate_block_quick_fix() {
    let source = "{% block nav %}nav{% endblock %}\n{% block nav %}nav2{% endblock %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let reg = Registry::load_core();

    let diag = w_diag("JINJA-W301", "Duplicate block 'nav'", 1, 0);
    let actions = code_actions(source, "/tpl.html", &[diag], &idx, &ws, &reg);

    assert!(!actions.is_empty());
    assert!(actions[0].edit.is_some());
    assert_eq!(actions[0].kind, ActionKind::QuickFix);
}
