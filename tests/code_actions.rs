// F17 — Code action tests: REQ-ACT-01, REQ-ACT-02, REQ-ACT-03, REQ-ACT-04, REQ-ACT-05.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
use jinja_lsp::features::code_actions::{code_actions, ActionKind, CodeAction};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn no_ws() -> WorkspaceIndex {
    WorkspaceIndex::default()
}

fn reg() -> Registry {
    Registry::load_core()
}

fn ws_with(templates: &[(&str, &str)]) -> WorkspaceIndex {
    let mut w = WorkspaceIndex::default();
    for (path, src) in templates {
        w.index_inline(path, src);
    }
    w
}

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

fn e103(line: u32, col: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col,
        code: "JINJA-E103".to_owned(),
        slug: "undefined-function".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("undefined function '{name}'"),
    }
}

/// Apply the first edit from a specific action (found by title substring) to `source`.
fn apply_by_title(source: &str, file: &str, actions: &[CodeAction], title_contains: &str) -> String {
    let action = actions
        .iter()
        .find(|a| a.title.contains(title_contains))
        .unwrap_or_else(|| panic!("no action with title containing '{title_contains}'"));
    apply_action(source, file, action)
}

/// Apply the first edit from `actions[0]` to `source`.
fn apply(source: &str, file: &str, actions: &[CodeAction]) -> String {
    assert!(!actions.is_empty(), "expected at least one action");
    apply_action(source, file, &actions[0])
}

fn apply_action(source: &str, file: &str, action: &CodeAction) -> String {
    let edit = action.edit.as_ref().expect("action must have an edit");
    let edits = edit.changes.get(file).unwrap_or_else(|| panic!("must have edits for {file}"));
    assert_eq!(edits.len(), 1, "expected exactly one text edit");
    let e = &edits[0];
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
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("Remove"));
    assert!(actions[0].title.contains("shared"));
    assert!(actions[0].is_preferred);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{{ content }}", "import line must be gone, no blank line left");
}

// ─── REQ-ACT-01: T-02 — Remove unused `{% macro … %}…{% endmacro %}` ─────────

#[test]
fn act01_t02_remove_unused_macro_whole_region() {
    let src = "{% macro foo() %}\n  body\n{% endmacro %}\n{{ bar }}";
    let idx = extract(src);
    let diags = vec![w202(0, "foo")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("Remove"));
    assert!(actions[0].title.contains("foo"));
    assert!(actions[0].is_preferred);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{{ bar }}", "full macro region must be gone, no blank line left");
}

// ─── REQ-ACT-01: T-03 — Remove one name from a multi-name from-import ────────

#[test]
fn act01_t03_remove_one_name_from_multi_name_from_import() {
    let src = "{% from \"x.html\" import a, b %}\n{{ a }}";
    let idx = extract(src);
    let diags = vec![w203(0, "b")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix));
    assert!(actions[0].title.contains("b"));
    let result = apply(src, "t.html", &actions);
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
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
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
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "non-W202/W203 diagnostic must not produce actions");
}

// ─── REQ-ACT-01: Additional — macro with multi-line body ─────────────────────

#[test]
fn act01_macro_with_multiline_body() {
    let src = "{% macro card(title) %}\n  <h1>{{ title }}</h1>\n  <p>body</p>\n{% endmacro %}\n{{ other }}";
    let idx = extract(src);
    let diags = vec![w202(0, "card")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{{ other }}", "all macro lines removed");
}

// ─── REQ-ACT-01: Additional — alias removal in from-import ───────────────────

#[test]
fn act01_remove_aliased_name_from_multi_name_from_import() {
    let src = "{% from \"x.html\" import a, b as bb %}\n{{ a }}";
    let idx = extract(src);
    let diags = vec![w203(0, "bb")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(
        result,
        "{% from \"x.html\" import a %}\n{{ a }}",
        "aliased name and alias removed together"
    );
}

// ─── REQ-ACT-02: T-04 — Insert import after extends for exact workspace match ──

#[test]
fn act02_t04_import_fix_after_extends() {
    let src = "{% extends \"base.html\" %}\n{{ post_url(post) }}";
    let macro_src = "{% macro post_url(post) %}url{% endmacro %}";
    let ws = ws_with(&[("blog/macros.html", macro_src)]);
    let idx = extract(src);
    // col=3 points into "post_url" in "{{ post_url(post) }}"
    let diags = vec![e103(1, 3, "post_url")];
    let actions = code_actions(src, "blog/post.html", &diags, &idx, &ws, &reg());
    let import_action = actions.iter().find(|a| a.title.contains("Import")).unwrap();
    assert!(import_action.title.contains("post_url"));
    assert!(import_action.title.contains("blog/macros.html"));
    assert!(import_action.is_preferred);
    let result = apply_by_title(src, "blog/post.html", &actions, "Import");
    assert_eq!(
        result,
        "{% extends \"base.html\" %}\n{% from \"blog/macros.html\" import post_url %}\n{{ post_url(post) }}",
        "import inserted after extends line"
    );
}

// ─── REQ-ACT-02: T-04b — No extends: insert import at top ───────────────────

#[test]
fn act02_t04b_import_fix_at_top_when_no_extends() {
    let src = "{{ post_url(post) }}";
    let macro_src = "{% macro post_url(post) %}url{% endmacro %}";
    let ws = ws_with(&[("macros.html", macro_src)]);
    let idx = extract(src);
    let diags = vec![e103(0, 3, "post_url")];
    let actions = code_actions(src, "t.html", &diags, &idx, &ws, &reg());
    let result = apply_by_title(src, "t.html", &actions, "Import");
    assert_eq!(
        result,
        "{% from \"macros.html\" import post_url %}\n{{ post_url(post) }}",
        "import inserted at top when no extends"
    );
}

// ─── REQ-ACT-02: T-05 — Near-miss offers did-you-mean + import fix ──────────

#[test]
fn act02_t05_near_miss_offers_did_you_mean_and_import() {
    let src = "{{ post_ur(post) }}";
    let macro_src = "{% macro post_url(post) %}url{% endmacro %}";
    let ws = ws_with(&[("blog/macros.html", macro_src)]);
    let idx = extract(src);
    let diags = vec![e103(0, 3, "post_ur")];
    let actions = code_actions(src, "t.html", &diags, &idx, &ws, &reg());

    // did-you-mean action replaces the identifier
    let mean_action = actions
        .iter()
        .find(|a| a.title.contains("Did you mean") && a.title.contains("post_url"))
        .expect("did-you-mean post_url action must be present");
    let result = apply_action(src, "t.html", mean_action);
    assert_eq!(result, "{{ post_url(post) }}", "identifier replaced with near-match");

    // import fix also offered for near-match workspace macro (spec T-05 "import fix plus")
    assert!(
        actions.iter().any(|a| a.title.contains("Import") && a.title.contains("post_url")),
        "import fix for near-match must be offered alongside did-you-mean"
    );
}

// ─── REQ-ACT-02: T-06 — No match → no action ────────────────────────────────

#[test]
fn act02_t06_no_match_no_action() {
    let src = "{{ zzqq(post) }}";
    let ws = WorkspaceIndex::default();
    let idx = extract(src);
    let diags = vec![e103(0, 3, "zzqq")];
    let actions = code_actions(src, "t.html", &diags, &idx, &ws, &reg());
    assert!(actions.is_empty(), "no match must produce no actions");
}

// ─── REQ-ACT-02: Additional — import action preferred over did-you-mean ───────

#[test]
fn act02_exact_match_is_preferred_over_near_miss() {
    let src = "{{ post_url(post) }}";
    let macro_src = "{% macro post_url(post) %}url{% endmacro %}";
    let ws = ws_with(&[("macros.html", macro_src)]);
    let idx = extract(src);
    let diags = vec![e103(0, 3, "post_url")];
    let actions = code_actions(src, "t.html", &diags, &idx, &ws, &reg());
    let preferred = actions.iter().filter(|a| a.is_preferred).count();
    assert_eq!(preferred, 1, "exactly one action must be preferred");
    assert!(actions.iter().any(|a| a.is_preferred && a.title.contains("Import")));
}

// ─── REQ-ACT-03: T-07 — E102 undefined-filter suggests close match ───────────

fn e102(line: u32, col: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col,
        code: "JINJA-E102".to_owned(),
        slug: "undefined-filter".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("undefined filter '{name}'"),
    }
}

fn e104(line: u32, col: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col,
        code: "JINJA-E104".to_owned(),
        slug: "undefined-test".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("undefined test '{name}'"),
    }
}

#[test]
fn act03_t07_undefined_filter_suggests_close_match() {
    // "uppe" is 1 edit from "upper" (insert 'r') — within threshold 1 for 4-char names
    let src = "{{ x | uppe }}";
    let idx = extract(src);
    let col = src.find("uppe").unwrap() as u32;
    let diags = vec![e102(0, col, "uppe")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(!actions.is_empty(), "must suggest at least one close filter");
    assert!(
        actions.iter().all(|a| a.title.contains("Did you mean")),
        "all actions must be did-you-mean"
    );
    let has_upper = actions.iter().any(|a| a.title.contains("upper"));
    assert!(has_upper, "must suggest 'upper' as a near-match for 'uppe'");
}

// ─── REQ-ACT-03: T-08 — E104 undefined-test suggests close match ──────────────

#[test]
fn act03_t08_undefined_test_suggests_close_match() {
    // "evn" is 1 edit from "even" (insert 'e') — within threshold 1 for 3-char names
    let src = "{% if x is evn %}{% endif %}";
    let idx = extract(src);
    let col = src.find("evn").unwrap() as u32;
    let diags = vec![e104(0, col, "evn")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(!actions.is_empty(), "must suggest at least one close test");
    assert!(
        actions.iter().any(|a| a.title.contains("even")),
        "must suggest 'even' as a near-match for 'evn'"
    );
    // applying the action replaces "evn" with "even"
    let even_action = actions.iter().find(|a| a.title.contains("even")).unwrap();
    let result = apply_action(src, "t.html", even_action);
    assert!(result.contains("is even"), "misspelled test name replaced");
}

// ─── REQ-ACT-03: T-09 — No close match → no action ──────────────────────────

#[test]
fn act03_t09_no_close_match_no_action() {
    let src = "{{ x | zzqq_filter }}";
    let idx = extract(src);
    let diags = vec![e102(0, 7, "zzqq_filter")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "no close match must produce no actions — we don't guess");
}

// ─── REQ-ACT-03: Additional — action replaces filter name in source ──────────

#[test]
fn act03_filter_action_replaces_name_in_source() {
    // "lowe" is 1 edit from "lower"
    let src = "{{ x | lowe }}";
    let idx = extract(src);
    let col = src.find("lowe").unwrap() as u32;
    let diags = vec![e102(0, col, "lowe")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    let lower_action = actions.iter().find(|a| a.title.contains("lower")).unwrap();
    let result = apply_action(src, "t.html", lower_action);
    assert_eq!(result, "{{ x | lower }}", "filter name replaced correctly");
}

// ─── REQ-ACT-04: T-10 — Insert block stub after extends ──────────────────────

fn e403(line: u32, col: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col,
        code: "JINJA-E403".to_owned(),
        slug: "missing-required-block".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("missing required block '{name}'"),
    }
}

#[test]
fn act04_t10_insert_block_stub_after_extends() {
    let src = "{% extends \"base.html\" %}\n{{ content }}";
    let idx = extract(src);
    let diags = vec![e403(0, 0, "content")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1, "must offer exactly one action");
    assert!(actions[0].title.contains("content"), "title must name the block");
    assert!(actions[0].is_preferred, "block stub fix is the only option — must be preferred");
    let result = apply(src, "t.html", &actions);
    assert_eq!(
        result,
        "{% extends \"base.html\" %}\n{% block content %}{% endblock %}\n{{ content }}",
        "block stub inserted after extends line"
    );
}

#[test]
fn act04_no_extends_no_action() {
    // E403 with no extends in index → no action (should not happen in practice but must be safe)
    let src = "{{ content }}";
    let idx = extract(src);
    let diags = vec![e403(0, 0, "content")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "no extends → no block stub action");
}

#[test]
fn act04_indented_extends_matches_indent() {
    // Indentation is taken from the extends line and applied to the block stub.
    let src = "  {% extends \"base.html\" %}\n  {{ content }}";
    let idx = extract(src);
    let diags = vec![e403(0, 0, "content")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(
        result,
        "  {% extends \"base.html\" %}\n  {% block content %}{% endblock %}\n  {{ content }}",
        "block stub indented to match extends line"
    );
}

// ─── REQ-ACT-05: T-11 — Create missing template ──────────────────────────────

fn e601(line: u32, col: u32, path: &str) -> Diagnostic {
    Diagnostic {
        file: "t.html".to_owned(),
        line,
        col,
        code: "JINJA-E601".to_owned(),
        slug: "template-does-not-exist".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("template does not exist '{path}'"),
    }
}

#[test]
fn act05_t11_create_missing_template() {
    let src = "{% extends \"missing/base.html\" %}";
    let idx = extract(src);
    let diags = vec![e601(0, 0, "missing/base.html")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1, "must offer exactly one action");
    assert!(actions[0].title.contains("missing/base.html"), "title must name the path");
    assert!(actions[0].is_preferred);
    let edit = actions[0].edit.as_ref().unwrap();
    assert_eq!(edit.create_files.len(), 1, "must request exactly one file creation");
    assert_eq!(edit.create_files[0].0, "missing/base.html", "correct path");
}

// ─── REQ-ACT-05: T-12 — No action for escaping paths ────────────────────────

#[test]
fn act05_t12_escaping_path_no_action() {
    let src = "{% extends \"../secret.html\" %}";
    let idx = extract(src);
    let diags = vec![e601(0, 0, "../secret.html")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "path escaping templates root must not get a create action");
}

#[test]
fn act05_absolute_path_no_action() {
    let src = "{% extends \"/etc/passwd\" %}";
    let idx = extract(src);
    let diags = vec![e601(0, 0, "/etc/passwd")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "absolute path must not get a create action");
}
