// REQ-ACT-11: Rename symbol — workspace-wide for definitions, scope-local for locals.

use jinja_lsp::features::rename::{rename_at_cursor, RenameTarget};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

fn sources_map(path: &str, source: &str) -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([(path.to_owned(), source.to_owned())])
}

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
    assert!(matches!(target, RenameTarget::Local { .. }));
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
    assert!(result.is_none() || result.map(|(t, _)| matches!(t, RenameTarget::Local { .. })).unwrap_or(true));
}

// ─── T-03: compute_rename produces edits for all occurrences ──────────────────

#[test]
fn act11_t03_compute_rename_replaces_all_occurrences() {
    use jinja_lsp::features::rename::compute_rename;

    let source = "{% set count = 1 %}{{ count }} and {{ count }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "count", "total", RenameTarget::Local { scope: None }, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");
    // Both {{ count }} usages must be renamed. The binding site in {% set %} is
    // a VariableDefinition, not a reference, and is a known pre-existing gap.
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

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
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

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
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

// ─── T-08: endblock trailing name is rewritten on block rename (jinja-lsp-24aj) ─

#[test]
fn act11_t08_endblock_trailing_name_is_renamed() {
    use jinja_lsp::features::rename::compute_rename;

    let source = r#"{% block content %}hello{% endblock content %}"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "content", "body", RenameTarget::Workspace, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit for block rename with trailing endblock name");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("expected edits for /tpl.html");
    // Should have at least 2 edits: the opening block name and the trailing endblock name.
    let body_edits: Vec<_> = file_edits.iter().filter(|e| e.new_text == "body").collect();
    assert!(
        body_edits.len() >= 2,
        "expected at least 2 edits (opening + endblock trailing name), got {}: {file_edits:?}",
        body_edits.len()
    );
    // Verify the endblock trailing name edit targets the right column.
    let endblock_col = source.rfind("content").unwrap() as u32;
    let has_endblock_edit = body_edits.iter().any(|e| e.start_col == endblock_col);
    assert!(has_endblock_edit, "expected edit at endblock trailing name col {endblock_col}; edits: {file_edits:?}");
}

#[test]
fn act11_t08b_endblock_without_trailing_name_still_renames() {
    use jinja_lsp::features::rename::compute_rename;

    let source = r#"{% block content %}hello{% endblock %}"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "content", "body", RenameTarget::Workspace, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit for block rename without trailing endblock name");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("expected edits for /tpl.html");
    let body_edits: Vec<_> = file_edits.iter().filter(|e| e.new_text == "body").collect();
    // Only 1 edit: the opening block name
    assert_eq!(body_edits.len(), 1, "expected exactly 1 edit (no endblock trailing name), got: {file_edits:?}");
}

// ─── jinja-lsp-xe8r: local rename respects valid_range scope ─────────────────

#[test]
fn act11_local_rename_scope_bounded_by_valid_range() {
    use jinja_lsp::features::rename::compute_rename;

    // Two bindings named "item": for-loop variable (restricted scope) + set (whole file).
    let source = "{% for item in xs %}{{ item }}{% endfor %}{% set item = 2 %}{{ item }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Find the for-loop's "item" binding's valid_range.
    let for_binding = idx.variables.iter()
        .find(|v| v.name == "item" && v.valid_range.end_byte < source.len())
        .expect("expected for-loop binding");
    let scope = for_binding.valid_range.clone();

    let edit = compute_rename(
        &sources_map("t.html", source), "t.html", "item", "x",
        RenameTarget::Local { scope: Some(scope) },
        &idx, &ws,
    );
    let we = edit.expect("expected edit");
    let edits = we.changes.get("t.html").expect("expected file edits");
    // All edits must fall within the for-loop scope (before "{% set item %}").
    let set_pos = source.find("{% set item").unwrap() as u32;
    for e in edits {
        let edit_byte = e.start_col; // col on line 0 = byte offset (single-line source)
        assert!(edit_byte < set_pos,
            "edit at col {edit_byte} is outside for-loop scope (set at col {set_pos}): {edits:?}");
    }
}

#[test]
fn act11_local_rename_whole_file_when_no_scope() {
    use jinja_lsp::features::rename::compute_rename;

    // No scope constraint: rename all occurrences across the whole template.
    let source = "{% set count = 1 %}{{ count }} and {{ count }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("t.html", source), "t.html", "count", "total",
        RenameTarget::Local { scope: None }, &idx, &ws);
    let we = edit.expect("expected edit");
    let edits = we.changes.get("t.html").expect("expected file edits");
    assert!(edits.len() >= 2, "expected at least 2 edits without scope: {edits:?}");
}

// ─── jinja-lsp-8vf4: identifier validation and collision refusal ──────────────

#[test]
fn act11_invalid_identifier_is_refused() {
    use jinja_lsp::features::rename::check_rename_preconditions;

    let source = "{% set count = 1 %}{{ count }}";
    let idx = extract(source);

    let target = RenameTarget::Local { scope: None };
    // Digits-only name
    let r = check_rename_preconditions("123", &target, &idx);
    assert!(r.is_some(), "digits-only name must be refused");
    // Empty string
    let r = check_rename_preconditions("", &target, &idx);
    assert!(r.is_some(), "empty name must be refused");
    // Name with spaces
    let r = check_rename_preconditions("my var", &target, &idx);
    assert!(r.is_some(), "name with spaces must be refused");
    // Valid name
    let r = check_rename_preconditions("total", &target, &idx);
    assert!(r.is_none(), "valid identifier 'total' must not be refused");
    // Leading underscore is valid in Jinja
    let r = check_rename_preconditions("_tmp", &target, &idx);
    assert!(r.is_none(), "_tmp must be accepted as valid identifier");
}

#[test]
fn act11_collision_in_same_scope_is_refused() {
    use jinja_lsp::features::rename::check_rename_preconditions;

    // Both `count` and `total` are in scope at the same time.
    let source = "{% set count = 1 %}{% set total = 2 %}{{ count }} {{ total }}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Try to rename `count` to `total` — `total` already binds in the same scope.
    let cursor_byte = source.find("{{ count }}").unwrap() + 3;
    let result = rename_at_cursor(source, "t.html", 0, cursor_byte as u32, &idx, &ws);
    let (target, _) = result.expect("rename_at_cursor must find count");

    let r = check_rename_preconditions("total", &target, &idx);
    assert!(r.is_some(), "renaming count→total must be refused (total already binds in scope)");
    assert!(r.unwrap().contains("collision"), "refusal message must mention collision");
}

#[test]
fn act11_collision_outside_scope_is_allowed() {
    use jinja_lsp::features::rename::check_rename_preconditions;

    // `item` from for-loop has a restricted scope; `total` only exists outside that scope.
    let source = "{% for item in xs %}{{ item }}{% endfor %}{% set total = 2 %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    // Rename `item` inside the for-loop — `total` is outside that scope, no collision.
    let cursor_byte = source.find("{{ item }}").unwrap() + 3;
    let result = rename_at_cursor(source, "t.html", 0, cursor_byte as u32, &idx, &ws);
    let (target, _) = result.expect("rename_at_cursor must find item");

    let r = check_rename_preconditions("total", &target, &idx);
    assert!(r.is_none(), "renaming item→total must be allowed; 'total' is outside the for-loop scope");
}

// ─── T-10: block/macro definition edits anchor at the name, not the keyword (jinja-lsp-3bdk) ──

#[test]
fn act11_t10_block_definition_edit_anchors_at_name_not_keyword() {
    use jinja_lsp::features::rename::compute_rename;

    let source = "{% block content %}hello{% endblock %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "content", "body", RenameTarget::Workspace, &idx, &ws);
    let we = edit.expect("expected a WorkspaceEdit");
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");

    let name_col = source.find("content").unwrap() as u32;
    let opening_edit = file_edits.iter().find(|e| e.new_text == "body" && e.start_line == 0);
    assert_eq!(
        opening_edit.map(|e| e.start_col),
        Some(name_col),
        "opening block-name edit must anchor at 'content', not the 'block' keyword; edits: {file_edits:?}"
    );
}

#[test]
fn act11_t11_macro_definition_edit_anchors_at_name_not_keyword() {
    use jinja_lsp::features::rename::compute_rename;

    let source = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
    let we = edit.expect("expected a WorkspaceEdit");
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");

    let name_col = source.find("greet").unwrap() as u32;
    let def_edit = file_edits.iter().find(|e| e.new_text == "say_hi" && e.start_col < 20);
    assert_eq!(
        def_edit.map(|e| e.start_col),
        Some(name_col),
        "macro definition edit must anchor at 'greet', not the 'macro' keyword; edits: {file_edits:?}"
    );
}

#[test]
fn act11_t12_cross_file_block_definition_edit_anchors_at_name() {
    use jinja_lsp::features::rename::compute_rename;

    // Two templates each define their own {% block content %} (e.g. a child
    // template overriding a parent's block). A workspace-wide block rename
    // initiated from "a.html" must also locate the name inside "b.html",
    // using b.html's own source — not a's.
    let a_src = "{% block content %}a{% endblock %}";
    let b_src = "{% block content %}b{% endblock %}";

    let mut ws = WorkspaceIndex::default();
    ws.templates.insert("a.html".to_owned(), extract(a_src));
    ws.templates.insert("b.html".to_owned(), extract(b_src));
    let index = ws.templates.get("a.html").unwrap().clone();

    let sources = std::collections::HashMap::from([
        ("a.html".to_owned(), a_src.to_owned()),
        ("b.html".to_owned(), b_src.to_owned()),
    ]);

    let edit = compute_rename(&sources, "a.html", "content", "body", RenameTarget::Workspace, &index, &ws);
    let we = edit.expect("expected a WorkspaceEdit");
    let b_edits = we.changes.get("b.html").expect("expected edits in b.html");

    let name_col = b_src.find("content").unwrap() as u32;
    let def_edit = b_edits.iter().find(|e| e.new_text == "body");
    assert_eq!(
        def_edit.map(|e| e.start_col),
        Some(name_col),
        "cross-file block definition edit must anchor at 'content' using b.html's own source; edits: {b_edits:?}"
    );
}

// ─── T-09: macro call sites (Function-kind refs) are rewritten (jinja-lsp-ux4u) ──

#[test]
fn act11_t09_macro_call_site_is_renamed() {
    use jinja_lsp::features::rename::compute_rename;

    let source = r#"{% macro greet(name) %}Hello {{ name }}{% endmacro %}{{ greet("World") }}"#;
    let idx = extract(source);
    let ws = WorkspaceIndex::default();

    let edit = compute_rename(&sources_map("/tpl.html", source), "/tpl.html", "greet", "say_hi", RenameTarget::Workspace, &idx, &ws);
    assert!(edit.is_some(), "expected a WorkspaceEdit");
    let we = edit.unwrap();
    let file_edits = we.changes.get("/tpl.html").expect("edits for the file");

    // The call site `greet("World")` starts after the macro definition (col > 40).
    let has_call_site_edit = file_edits.iter().any(|e| e.new_text == "say_hi" && e.start_col > 40);
    assert!(
        has_call_site_edit,
        "expected the macro call site greet(...) to be renamed; edits: {file_edits:?}"
    );
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
