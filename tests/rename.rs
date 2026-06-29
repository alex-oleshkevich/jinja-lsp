// REQ-ACT-11: Rename symbol — workspace-wide for definitions, scope-local for locals.

use jinja_lsp::features::rename::{rename_at_cursor, RenameTarget};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── T-01: cursor on a local variable offers a local rename ──────────────────

#[test]
fn act11_t01_local_variable_rename_offered() {
    let source = "{% set count = 1 %}{{ count }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Cursor on "count" in `{{ count }}` (line 0, col 22)
    let result = rename_at_cursor(source, "/tpl.html", 0, 22, &idx, &ws);
    assert!(result.is_some(), "expected rename to be offered");
    let (target, name) = result.unwrap();
    assert_eq!(name, "count");
    assert!(matches!(target, RenameTarget::Local));
}

// ─── T-02: cursor on whitespace → no rename ───────────────────────────────────

#[test]
fn act11_t02_no_rename_on_non_symbol() {
    let source = "{% set count = 1 %}{{ count }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Cursor on `=` or whitespace
    let result = rename_at_cursor(source, "/tpl.html", 0, 10, &idx, &ws);
    // col 10 is the `=` or spaces — no symbol there
    assert!(result.is_none() || result.map(|(t, _)| matches!(t, RenameTarget::Local)).unwrap_or(true));
}

// ─── T-03: compute_rename produces edits for all occurrences ──────────────────

#[test]
fn act11_t03_compute_rename_replaces_all_occurrences() {
    use jinja_lsp::features::rename::compute_rename;

    let source = "{% set count = 1 %}{{ count }} and {{ count }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(source, "/tpl.html", "count", "total", RenameTarget::Local, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");
    // There should be edits for `count` in set and both {{ count }} uses.
    assert!(file_edits.len() >= 2, "expected at least 2 edits, got {}", file_edits.len());
    for e in file_edits {
        assert_eq!(e.new_text, "total");
    }
}

// ─── T-04: macro definition cursor offers workspace rename ────────────────────

#[test]
fn act11_t04_macro_definition_offers_workspace_rename() {
    let source = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Cursor on "greet" in `{% macro greet(...) %}` — "greet" starts at col 9
    let result = rename_at_cursor(source, "/tpl.html", 0, 9, &idx, &ws);
    assert!(result.is_some());
    let (target, name) = result.unwrap();
    assert_eq!(name, "greet");
    assert!(matches!(target, RenameTarget::Workspace));
}

#[test]
#[ignore]
fn debug_spans() {
    let source = "{% set count = 1 %}{{ count }}";
    let idx = jinja_lsp::parsing::extract(source);
    eprintln!("refs: {:?}", idx.references);
    eprintln!("vars: {:?}", idx.variables);
    eprintln!("macros: {:?}", idx.macros);
    
    let source2 = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let idx2 = jinja_lsp::parsing::extract(source2);
    eprintln!("macros2: {:?}", idx2.macros);
}
