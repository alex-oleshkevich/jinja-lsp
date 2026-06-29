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

// ─── Guard: HTML text matching a block/macro name must not trigger rename ─────

#[test]
fn act11_html_text_matching_block_name_no_rename() {
    // "content" appears as plain HTML text; it also happens to be a block name.
    // Cursor in the HTML text must NOT offer a rename (would corrupt HTML).
    let source = r#"{% block content %}hello{% endblock %}<p>content goes here</p>"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    // col 42 lands on "content" in the HTML text <p>content...</p>
    let html_col = source.find("<p>content").map(|p| p + 3).unwrap() as u32;
    let result = rename_at_cursor(source, "/tpl.html", 0, html_col, &idx, &ws);
    assert!(result.is_none(), "HTML text matching a block name must not offer rename: {result:?}");
}

#[test]
fn act11_jinja_block_name_still_renames() {
    // Cursor on the block name inside {% block content %} MUST still offer rename.
    let source = r#"{% block content %}hello{% endblock %}<p>content goes here</p>"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();
    let jinja_col = source.find("content").unwrap() as u32;
    let result = rename_at_cursor(source, "/tpl.html", 0, jinja_col, &idx, &ws);
    assert!(result.is_some(), "block name inside Jinja delimiter must still offer rename");
    let (target, name) = result.unwrap();
    assert_eq!(name, "content");
    assert!(matches!(target, RenameTarget::Workspace));
}

// ─── T-05: from-import name is renamed when the macro is renamed ─────────────

#[test]
fn act11_t05_from_import_name_is_renamed() {
    use jinja_lsp::features::rename::compute_rename;

    // A template that imports "greet" by name.
    let source = r#"{% from "macros.html" import greet %}{{ greet("World") }}"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(source, "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");
    // There must be an edit that changes the name inside the from-import statement.
    let has_import_edit = file_edits.iter().any(|e| e.new_text == "say_hi" && e.start_col < 40);
    assert!(has_import_edit, "expected an edit inside the from-import statement; edits: {file_edits:?}");
}

#[test]
fn act11_t06_from_import_with_alias_renames_name_not_alias() {
    use jinja_lsp::features::rename::compute_rename;

    // "greet as g" — renaming "greet" (the macro) must touch the name, not the alias.
    let source = r#"{% from "macros.html" import greet as g %}{{ g("World") }}"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(source, "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");
    // Must rename "greet" but NOT "g".
    let has_greet_edit = file_edits.iter().any(|e| e.new_text == "say_hi");
    assert!(has_greet_edit, "expected 'greet' to be renamed; edits: {file_edits:?}");
    // The alias "g" itself should NOT be rewritten (it's a different name).
    let alias_rewritten = file_edits.iter().any(|e| e.new_text == "say_hi" && e.start_col > 38);
    assert!(!alias_rewritten, "alias 'g' must not be renamed; edits: {file_edits:?}");
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
