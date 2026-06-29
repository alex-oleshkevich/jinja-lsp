// F17 — Code action tests: REQ-ACT-01 (remove unused imports and macros).

use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
use jinja_lsp::features::code_actions::{code_actions, ActionKind};
use jinja_lsp::parsing::extract;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn w202(line: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col: 0,
        code: "JINJA-W202".to_owned(),
        slug: "unused-macro".to_owned(),
        severity: DiagnosticSeverity::Warning,
        message: format!("unused macro '{name}'"),
    }
}

fn w203(line: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col: 0,
        code: "JINJA-W203".to_owned(),
        slug: "unused-import".to_owned(),
        severity: DiagnosticSeverity::Warning,
        message: format!("unused import '{name}'"),
    }
}

/// Apply the first edit from the first action to `source` and return the result.
/// Implements standard LSP TextEdit byte-range replacement.
fn apply(source: &str, actions: &[jinja_lsp::features::code_actions::CodeAction]) -> String {
    assert!(!actions.is_empty(), "expected at least one action");
    let edit = actions[0].edit.as_ref().expect("action must have an edit");
    let edits = edit.changes.get("t.html").expect("must have edits for t.html");
    assert_eq!(edits.len(), 1, "expected exactly one text edit");
    let e = &edits[0];
    // Compute byte offsets for (line, col) positions.
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.char_indices().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1))
        .collect();
    let start_byte = line_starts.get(e.start_line as usize).copied().unwrap_or(source.len())
        + e.start_col as usize;
    let end_byte = line_starts.get(e.end_line as usize).copied().unwrap_or(source.len())
        + e.end_col as usize;
    format!("{}{}{}", &source[..start_byte], e.new_text, &source[end_byte..])
}

// ─── REQ-ACT-01: T-01 — Remove unused `{% import … as … %}` ─────────────────

#[test]
fn act01_t01_remove_import_alias_whole_line() {
    let src = "{% import \"shared.html\" as shared %}\n{{ content }}";
    let idx = extract(src);
    let diags = vec![w203(0, "shared")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("Remove"));
    assert!(actions[0].title.contains("shared"));
    assert!(actions[0].is_preferred);
    let result = apply(src, &actions);
    assert_eq!(result, "{{ content }}", "import line must be gone, no blank line left");
}

// ─── REQ-ACT-01: T-02 — Remove unused `{% macro … %}…{% endmacro %}` ─────────

#[test]
fn act01_t02_remove_unused_macro_whole_region() {
    let src = "{% macro foo() %}\n  body\n{% endmacro %}\n{{ bar }}";
    let idx = extract(src);
    let diags = vec![w202(0, "foo")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("Remove"));
    assert!(actions[0].title.contains("foo"));
    assert!(actions[0].is_preferred);
    let result = apply(src, &actions);
    assert_eq!(result, "{{ bar }}", "full macro region must be gone, no blank line left");
}

// ─── REQ-ACT-01: T-03 — Remove one name from a multi-name from-import ────────

#[test]
fn act01_t03_remove_one_name_from_multi_name_from_import() {
    let src = "{% from \"x.html\" import a, b %}\n{{ a }}";
    let idx = extract(src);
    let diags = vec![w203(0, "b")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("b"));
    let result = apply(src, &actions);
    assert_eq!(
        result,
        "{% from \"x.html\" import a %}\n{{ a }}",
        "only unused name removed, used name and line intact"
    );
}

// ─── REQ-ACT-01: Additional — single-name from-import deletes whole line ──────

#[test]
fn act01_single_name_from_import_deletes_whole_line() {
    let src = "{% from \"x.html\" import b %}\n{{ content }}";
    let idx = extract(src);
    let diags = vec![w203(0, "b")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    let result = apply(src, &actions);
    assert_eq!(result, "{{ content }}", "single-name from-import → whole line deleted");
}

// ─── REQ-ACT-01: Additional — no action when diagnostic code is not W202/W203 ─

#[test]
fn act01_no_action_for_unrelated_diagnostic() {
    let src = "{% macro foo() %}body{% endmacro %}";
    let idx = extract(src);
    let diags = vec![Diagnostic {
        file: "t.html".to_owned(),
        line: 0,
        col: 0,
        code: "JINJA-E101".to_owned(),
        slug: "undefined-variable".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: "undefined variable 'foo'".to_owned(),
    }];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert!(actions.is_empty(), "non-W202/W203 diagnostic must not produce actions");
}

// ─── REQ-ACT-01: Additional — macro with multi-line body ─────────────────────

#[test]
fn act01_macro_with_multiline_body() {
    let src = "{% macro card(title) %}\n  <h1>{{ title }}</h1>\n  <p>body</p>\n{% endmacro %}\n{{ other }}";
    let idx = extract(src);
    let diags = vec![w202(0, "card")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    let result = apply(src, &actions);
    assert_eq!(result, "{{ other }}", "all macro lines removed");
}

// ─── REQ-ACT-01: Additional — alias removal in from-import ───────────────────

#[test]
fn act01_remove_aliased_name_from_multi_name_from_import() {
    let src = "{% from \"x.html\" import a, b as bb %}\n{{ a }}";
    let idx = extract(src);
    let diags = vec![w203(0, "bb")];
    let actions = code_actions(src, "t.html", &diags, &idx);
    assert_eq!(actions.len(), 1);
    let result = apply(src, &actions);
    assert_eq!(
        result,
        "{% from \"x.html\" import a %}\n{{ a }}",
        "aliased name and alias removed together"
    );
}
