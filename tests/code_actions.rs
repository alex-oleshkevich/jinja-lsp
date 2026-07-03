// F17 — Code action tests: REQ-ACT-01..REQ-ACT-08.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
use jinja_lsp::features::code_actions::{code_actions, selection_code_actions, ActionKind, CodeAction};
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

// ─── REQ-ACT-02: server-mode workspace (absolute keys) inserts relative path ──

#[test]
fn act02_import_fix_uses_relative_path_when_workspace_keyed_absolute() {
    // Mirrors build_workspace_abs: templates keyed by absolute filesystem path,
    // with relative_path recording the templates-root-relative form.
    let src = "{% extends \"base.html\" %}\n{{ post_url(post) }}";
    let macro_src = "{% macro post_url(post) %}url{% endmacro %}";
    let mut ws = WorkspaceIndex::default();
    let mut idx = extract(macro_src);
    idx.path = "/srv/templates/blog/macros.html".to_owned();
    idx.relative_path = Some("blog/macros.html".to_owned());
    ws.templates.insert("/srv/templates/blog/macros.html".to_owned(), idx);

    let idx = extract(src);
    let diags = vec![e103(1, 3, "post_url")];
    let actions = code_actions(src, "/srv/templates/blog/post.html", &diags, &idx, &ws, &reg());
    let import_action = actions.iter().find(|a| a.title.contains("Import")).unwrap();
    assert!(
        import_action.title.contains("blog/macros.html")
            && !import_action.title.contains("/srv/templates"),
        "quick-fix title must show the template-relative path, not the absolute key: {}",
        import_action.title
    );
    let result = apply_by_title(src, "/srv/templates/blog/post.html", &actions, "Import");
    assert_eq!(
        result,
        "{% extends \"base.html\" %}\n{% from \"blog/macros.html\" import post_url %}\n{{ post_url(post) }}",
        "inserted import must use the relative path so Jinja can resolve it"
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

// ─── vv5j: path-traversal guard must cover backslash and bare '..' ────────────

#[test]
fn vv5j_bare_dotdot_final_segment_no_action() {
    let src = r#"{% extends "templates/.." %}"#;
    let idx = extract(src);
    let diags = vec![e601(0, 0, "templates/..")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "bare '..' final segment must be rejected");
}

#[test]
fn vv5j_backslash_traversal_no_action() {
    let src = "{% extends \"..\\\\secret.html\" %}";
    let idx = extract(src);
    let diags = vec![e601(0, 0, "..\\secret.html")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert!(actions.is_empty(), "backslash '..' traversal must be rejected");
}

// ─── REQ-ACT-06 helpers ──────────────────────────────────────────────────────

fn w301(line: u32, name: &str) -> Diagnostic {
    Diagnostic { file: "t.html".to_owned(), line, col: 0, code: "JINJA-W301".to_owned(),
        slug: "duplicate-block".to_owned(), severity: DiagnosticSeverity::Warning,
        message: format!("duplicate block '{name}'") }
}
fn w302(line: u32, name: &str) -> Diagnostic {
    Diagnostic { file: "t.html".to_owned(), line, col: 0, code: "JINJA-W302".to_owned(),
        slug: "duplicate-macro".to_owned(), severity: DiagnosticSeverity::Warning,
        message: format!("duplicate macro '{name}'") }
}
fn w303(line: u32, name: &str) -> Diagnostic {
    Diagnostic { file: "t.html".to_owned(), line, col: 0, code: "JINJA-W303".to_owned(),
        slug: "duplicate-import-alias".to_owned(), severity: DiagnosticSeverity::Warning,
        message: format!("duplicate import alias '{name}'") }
}
fn w304(line: u32, name: &str) -> Diagnostic {
    Diagnostic { file: "t.html".to_owned(), line, col: 0, code: "JINJA-W304".to_owned(),
        slug: "duplicate-from-import".to_owned(), severity: DiagnosticSeverity::Warning,
        message: format!("duplicate from-import '{name}'") }
}
fn w305(line: u32, col: u32, name: &str) -> Diagnostic {
    Diagnostic { file: "t.html".to_owned(), line, col, code: "JINJA-W305".to_owned(),
        slug: "name-shadowing".to_owned(), severity: DiagnosticSeverity::Warning,
        message: format!("variable '{name}' shadows outer binding") }
}

/// Apply all edits in the action's first file entry (bottom-to-top to preserve offsets).
fn apply_all(source: &str, file: &str, action: &CodeAction) -> String {
    let edit = action.edit.as_ref().expect("action must have an edit");
    let edits = edit.changes.get(file).expect("edits for file");
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.char_indices().filter(|(_, c)| *c == '\n').map(|(i, _)| i + 1))
        .collect();
    let mut byte_edits: Vec<(usize, usize, String)> = edits.iter().map(|e| {
        let s = line_starts.get(e.start_line as usize).copied().unwrap_or(source.len()) + e.start_col as usize;
        let en = line_starts.get(e.end_line as usize).copied().unwrap_or(source.len()) + e.end_col as usize;
        (s, en, e.new_text.clone())
    }).collect();
    // Apply bottom-to-top so earlier byte offsets are not invalidated.
    byte_edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut result = source.to_owned();
    for (s, en, new_text) in byte_edits {
        result = format!("{}{}{}", &result[..s], new_text, &result[en..]);
    }
    result
}

// ─── REQ-ACT-06: T-13 — Remove duplicate block ───────────────────────────────

#[test]
fn act06_t13_remove_duplicate_block() {
    let src = "{% block content %}first{% endblock %}\n{% block content %}second{% endblock %}";
    let idx = extract(src);
    // W301 points to the duplicate (second) block at line 1
    let diags = vec![w301(1, "content")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1, "must offer remove-duplicate action");
    assert!(actions[0].title.contains("content"));
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{% block content %}first{% endblock %}", "later block removed");
}

// ─── REQ-ACT-06: T-14 — Remove duplicate macro ───────────────────────────────

#[test]
fn act06_t14_remove_duplicate_macro() {
    let src = "{% macro foo() %}first{% endmacro %}\n{% macro foo() %}second{% endmacro %}";
    let idx = extract(src);
    let diags = vec![w302(1, "foo")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{% macro foo() %}first{% endmacro %}", "later macro removed");
}

// ─── REQ-ACT-06: T-15 — Remove duplicate import alias ────────────────────────

#[test]
fn act06_t15_remove_duplicate_import_alias() {
    let src = "{% import \"shared.html\" as shared %}\n{% import \"other.html\" as shared %}";
    let idx = extract(src);
    let diags = vec![w303(1, "shared")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{% import \"shared.html\" as shared %}", "later import alias removed");
}

// ─── REQ-ACT-06: T-16 — Remove duplicate from-import ────────────────────────

#[test]
fn act06_t16_remove_duplicate_from_import() {
    let src = "{% from \"x.html\" import foo %}\n{% from \"x.html\" import foo %}";
    let idx = extract(src);
    let diags = vec![w304(1, "foo")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{% from \"x.html\" import foo %}", "later from-import removed");
}

// ─── REQ-ACT-06: T-17 — Rename shadowing variable ────────────────────────────

#[test]
fn act06_t17_rename_shadowing_variable() {
    // for-loop variable "post" shadows an outer context variable.
    // The rename replaces the definition + all in-scope identifier references.
    let src = "{% for post in posts %}{{ post }}{% endfor %}";
    let idx = extract(src);
    // W305 diagnostic at the for-loop variable definition position
    let col = src.find("for post").map(|i| i + "for ".len()).unwrap() as u32;
    let diags = vec![w305(0, col, "post")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1, "must offer rename action");
    assert!(actions[0].title.contains("post_2"), "suggestion suffixes _2");
    let result = apply_all(src, "t.html", &actions[0]);
    assert!(result.contains("post_2"), "post renamed to post_2");
    assert!(!result.contains("{% for post "), "definition renamed");
}

// ─── REQ-ACT-06: T-18 — Identical duplicate macro — remove the later one ─────

#[test]
fn act06_t18_identical_duplicate_macro_removes_later() {
    let src = "{% macro foo() %}body{% endmacro %}\n{% macro foo() %}body{% endmacro %}\n{{ bar }}";
    let idx = extract(src);
    // W302 points to the later (second) macro
    let diags = vec![w302(1, "foo")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(result, "{% macro foo() %}body{% endmacro %}\n{{ bar }}", "only the later macro removed");
}

// ─── REQ-ACT-10: T-36 — Quick-fixes have kind=QuickFix and diagnostics set ───

#[test]
fn act10_t36_quickfix_kind_and_diagnostics_set() {
    // Every diagnostic-driven action must carry kind=QuickFix and the originating diagnostic.
    let src = "{% import \"shared.html\" as shared %}\n{{ content }}";
    let idx = extract(src);
    let diag = Diagnostic {
        file: "t.html".to_owned(), line: 0, col: 0,
        code: "JINJA-W203".to_owned(), slug: "unused-import".to_owned(),
        severity: DiagnosticSeverity::Warning,
        message: "unused import 'shared'".to_owned(),
    };
    let actions = code_actions(src, "t.html", &[diag.clone()], &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0].kind, ActionKind::QuickFix), "must be QuickFix");
    assert!(!actions[0].diagnostics.is_empty(), "diagnostics must be set");
    assert_eq!(actions[0].diagnostics[0].code, "JINJA-W203", "must reference the originating diagnostic");
}

// ─── REQ-ACT-10: T-37 — Import fix isPreferred; did-you-mean suggestions are not ──

#[test]
fn act10_t37_import_fix_is_preferred_over_suggestions() {
    // Workspace has exact-match macro; import fix must be isPreferred, suggestions must not.
    let ws = ws_with(&[("blog/macros.html", "{% macro post_url(post) %}url{% endmacro %}")]);
    let src = "{{ post_url(post) }}";
    let idx = extract(src);
    let diag = e103(0, 3, "post_url");
    let actions = code_actions(src, "t.html", &[diag], &idx, &ws, &reg());
    let import_action = actions.iter().find(|a| a.title.contains("Import")).expect("import fix must be offered");
    assert!(import_action.is_preferred, "import fix must be isPreferred");
    for action in actions.iter().filter(|a| a.title.contains("Did you mean")) {
        assert!(!action.is_preferred, "did-you-mean suggestions must not be isPreferred");
    }
}

// ─── REQ-ACT-08: wrap-selection ──────────────────────────────────────────────

#[test]
fn act08_t01_wrap_if_produces_refactor_rewrite_action() {
    let src = "<p>hello</p>\n<p>world</p>";
    let actions = selection_code_actions(src, "t.html", 0, 0);
    let wrap_if = actions.iter().find(|a| a.title.contains("if")).expect("wrap-if action must exist");
    assert!(matches!(wrap_if.kind, ActionKind::RefactorRewrite), "wrap must be RefactorRewrite");
    assert!(wrap_if.edit.is_some(), "wrap action must have an edit");
}

#[test]
fn act08_t02_wrap_for_produces_refactor_rewrite_action() {
    let src = "{{ item }}";
    let actions = selection_code_actions(src, "t.html", 0, 0);
    let wrap_for = actions.iter().find(|a| a.title.contains("for")).expect("wrap-for action must exist");
    assert!(matches!(wrap_for.kind, ActionKind::RefactorRewrite));
}

#[test]
fn act08_t03_wrap_block_produces_refactor_rewrite_action() {
    let src = "{{ item }}";
    let actions = selection_code_actions(src, "t.html", 0, 0);
    let wrap_block = actions.iter().find(|a| a.title.contains("block")).expect("wrap-block action must exist");
    assert!(matches!(wrap_block.kind, ActionKind::RefactorRewrite));
    // REQ-ACT-07: wrap-block uses executeCommand so the editor can prompt for a name.
    assert!(wrap_block.edit.is_none(), "wrap-block must not carry an inline edit");
    let (cmd_id, args) = wrap_block.command.as_ref().expect("wrap-block must carry a command");
    assert_eq!(cmd_id, "jinja-lsp.wrap-block");
    assert_eq!(args["path"], "t.html");
}

#[test]
fn act08_t04_wrap_if_edit_inserts_tags_around_selection() {
    let src = "<p>hello</p>\n<p>world</p>\n<footer/>";
    // Wrap lines 0..=1
    let actions = selection_code_actions(src, "t.html", 0, 1);
    let wrap_if = actions.iter().find(|a| a.title.contains("if")).unwrap();
    let edit = wrap_if.edit.as_ref().unwrap();
    let edits = edit.changes.get("t.html").unwrap();
    // Single replacement edit spanning the whole selection.
    assert_eq!(edits.len(), 1, "wrap produces exactly 1 edit");
    assert!(edits[0].new_text.contains("if condition"), "edit must contain opening tag");
    assert!(edits[0].new_text.contains("endif"), "edit must contain closing tag");
    assert!(edits[0].new_text.contains("  <p>hello</p>"), "body must be indented one level");
    assert!(edits[0].new_text.contains("  <p>world</p>"), "both body lines must be indented");
}

// ─── T-24: split-selection negative — no wrap offered ────────────────────────

#[test]
fn act08_t24_split_tag_no_wrap_offered() {
    // Selection contains `{%` with no matching `%}` — splits a statement tag.
    let src = "before\n{% if x";
    let actions = selection_code_actions(src, "t.html", 0, 1);
    assert!(
        actions.iter().all(|a| !a.title.contains("Wrap")),
        "wrap must not be offered when selection splits a tag"
    );
}

#[test]
fn act08_t24b_split_tag_no_extract_offered() {
    let src = "{% if x";
    let actions = selection_code_actions(src, "t.html", 0, 0);
    assert!(actions.is_empty(), "no actions when selection splits a tag");
}

#[test]
fn act08_t24c_balanced_tags_wraps_offered() {
    // Selection has balanced {%/%} — wraps should be offered.
    let src = "{% if x %}\nhello\n{% endif %}";
    let actions = selection_code_actions(src, "t.html", 0, 2);
    assert!(
        actions.iter().any(|a| a.title.contains("Wrap")),
        "wraps must be offered for a well-formed selection"
    );
}

// ─── REQ-ACT-07: extract-to-macro ────────────────────────────────────────────

#[test]
fn act07_t01_extract_macro_produces_refactor_extract_action() {
    let src = "<p>hello</p>\n<p>world</p>";
    let actions = selection_code_actions(src, "t.html", 0, 0);
    let extract_action = actions.iter().find(|a| a.title.contains("macro")).expect("extract-macro action must exist");
    assert!(matches!(extract_action.kind, ActionKind::RefactorExtract), "extract must be RefactorExtract");
    // REQ-ACT-08: emits a server command (editor prompts for name), no inline edit.
    assert!(extract_action.edit.is_none(), "extract-macro must not carry an inline edit");
    let (cmd_id, args) = extract_action.command.as_ref().expect("extract-macro must carry a command");
    assert_eq!(cmd_id, "jinja-lsp.extract-macro");
    assert_eq!(args["path"], "t.html");
    assert_eq!(args["start_line"], 0);
    assert_eq!(args["end_line"], 0);
}

#[test]
fn act07_t02_extract_macro_command_carries_expected_args() {
    let src = "<p>hello</p>\n<p>world</p>";
    let actions = selection_code_actions(src, "t.html", 0, 1);
    let extract_action = actions.iter().find(|a| a.title.contains("macro")).unwrap();
    let (cmd_id, args) = extract_action.command.as_ref().unwrap();
    assert_eq!(cmd_id, "jinja-lsp.extract-macro");
    // path, start_line, end_line, name are all present.
    assert!(args.get("path").is_some());
    assert!(args.get("start_line").is_some());
    assert!(args.get("end_line").is_some());
    assert!(args.get("name").is_some());
}

// ─── jinja-lsp-gspz: remove_name_from_import_line must skip the quoted path ──

#[test]
fn gspz_import_keyword_inside_path_not_mistaken_for_import_keyword() {
    // Path contains "import " substring — must not corrupt the rebuilt line.
    let src = "{% from \"import helpers.html\" import a, b %}\n{{ a }}";
    let idx = extract(src);
    let diags = vec![w203(0, "b")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(
        result,
        "{% from \"import helpers.html\" import a %}\n{{ a }}",
        "path containing 'import ' must be skipped when locating the import keyword"
    );
}

#[test]
fn gspz_remove_first_name_from_import_with_path_containing_import() {
    // Removing "a" (not the last name) from a path that contains "import " —
    // this exposes the bug where import_kw lands inside the quoted path.
    let src = "{% from \"import helpers.html\" import a, b %}\n{{ b }}";
    let idx = extract(src);
    let diags = vec![w203(0, "a")];
    let actions = code_actions(src, "t.html", &diags, &idx, &no_ws(), &reg());
    assert_eq!(actions.len(), 1);
    let result = apply(src, "t.html", &actions);
    assert_eq!(
        result,
        "{% from \"import helpers.html\" import b %}\n{{ b }}",
        "first name must be removed cleanly even with 'import ' in the path"
    );
}
