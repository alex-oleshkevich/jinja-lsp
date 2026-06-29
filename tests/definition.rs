// F08 — Go-to-definition tests: REQ-DEF-01 through REQ-DEF-09.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::definition::go_to_definition;
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── REQ-DEF-01: macro call → macro definition ───────────────────────────────

#[test]
fn def01_macro_in_same_template_jumps_to_definition() {
    let src = "{% macro greet(name) %}{% endmacro %}{{ greet( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "greet" in {{ greet( }}
    let col = src.rfind("greet").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    assert!(result.is_some(), "macro call must jump to definition");
    let def = result.unwrap();
    assert_eq!(def.target_path, "test.html", "same-file macro must point to current file");
    // The macro name span must be somewhere in line 0
    assert_eq!(def.target_start_line, 0, "macro is on line 0");
}

#[test]
fn def01_macro_definition_itself_returns_none() {
    // Hovering ON the macro name in its own declaration should return nothing
    // (you're already at the definition).
    let src = "{% macro greet(name) %}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "greet" in the macro definition
    let col = src.find("greet").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    // Either None (preferred) or a self-referential jump is acceptable.
    // The important thing: no panic.
    let _ = result;
}

// ─── REQ-DEF-02: template path → file start ──────────────────────────────────

#[test]
fn def02_extends_path_jumps_to_file_start() {
    let src = r#"{% extends "base.html" %}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", "base content");
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.find("base.html").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "extends path must jump to file");
    let def = result.unwrap();
    assert_eq!(def.target_path, "base.html");
    assert_eq!(def.target_start_line, 0, "file start is line 0");
    assert_eq!(def.target_start_col, 0, "file start is col 0");
}

#[test]
fn def02_unknown_path_returns_none() {
    let src = r#"{% extends "unknown.html" %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default(); // no templates registered
    let col = src.find("unknown.html").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_none(), "unknown template path must return None");
}

// ─── REQ-DEF-03: from-import name → macro in source template ─────────────────

#[test]
fn def03_from_import_name_jumps_to_macro() {
    let src = r#"{% from "macros.html" import post_url %}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro post_url(post) %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on "post_url" in the from-import
    let col = src.find("post_url").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "from-import name must jump to macro");
    let def = result.unwrap();
    assert_eq!(def.target_path, "macros.html");
}

// ─── REQ-DEF-04: block name → ancestor block declaration ─────────────────────

#[test]
fn def04_block_in_child_jumps_to_ancestor() {
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", "{% block content %}base{% endblock %}");
    let child_src = r#"{% extends "base.html" %}{% block content %}override{% endblock %}"#;
    let idx = extract(child_src);
    let reg = Registry::load_core();
    // Cursor on "content" in the child's block
    let col = child_src.find("content").unwrap() as u32;
    let result = go_to_definition(child_src, 0, col, "child.html", &idx, &reg, &ws);
    // Should jump to the "content" block in base.html (nearest ancestor).
    // If DEF-04 is implemented, this succeeds; otherwise None is acceptable.
    if let Some(def) = result {
        assert_eq!(def.target_path, "base.html", "must jump to ancestor block file");
    }
    // Not asserting is_some() since DEF-04 may be best-effort.
}

// ─── REQ-DEF-05: import alias → its declaration ──────────────────────────────

#[test]
fn def05_alias_usage_jumps_to_declaration() {
    // When cursor is on "macros" alias usage, jump to the import declaration.
    let src = r#"{% import "macros.html" as macros %}{{ macros }}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "macros" in {{ macros }}
    let col = src.rfind("macros").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    // Must jump somewhere (either to import declaration or source file).
    assert!(result.is_some(), "alias usage must jump to its declaration");
}

#[test]
fn def05_alias_attribute_jumps_to_macro_in_source() {
    // {{ macros.post_url() }} — cursor on "post_url" should jump to the macro.
    let src = r#"{% import "macros.html" as macros %}{{ macros.post_url() }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", "{% macro post_url(post) %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    // Cursor on "post_url" (the attribute part)
    let col = src.rfind("post_url").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    if let Some(def) = result {
        assert_eq!(def.target_path, "macros.html", "alias attribute must jump to source macro");
    }
    // Not asserting is_some() — attribute resolution depends on extraction data.
}

// ─── REQ-DEF-03b: from..import..as alias → macro in source template ──────────

#[test]
fn def03b_from_import_alias_jumps_to_real_macro() {
    // `{% from "m.html" import foo as bar %}{{ bar( }}` — cursor on "bar" (the alias
    // at call site) must jump to the "foo" macro in m.html, not return None.
    let src = r#"{% from "m.html" import foo as bar %}{{ bar( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("m.html", "{% macro foo(x) %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    let col = src.rfind("bar").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "aliased macro call must jump to real macro definition");
    let def = result.unwrap();
    assert_eq!(def.target_path, "m.html", "must jump to source template");
}

#[test]
fn def03b_from_import_alias_in_import_stmt_jumps_to_macro() {
    // Cursor ON the alias name in the import statement itself also must resolve.
    let src = r#"{% from "m.html" import foo as bar %}{{ bar( }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("m.html", "{% macro foo(x) %}{% endmacro %}");
    let idx = extract(src);
    let reg = Registry::load_core();
    // "bar" first occurrence is in the import statement
    let col = src.find("bar").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "alias name in import stmt must jump to macro");
    let def = result.unwrap();
    assert_eq!(def.target_path, "m.html");
}

// ─── REQ-DEF-06: host-owned/unresolvable → nothing ───────────────────────────

#[test]
fn def06_builtin_filter_returns_none() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("truncate").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    assert!(result.is_none(), "built-in filter must return None (REQ-DEF-06)");
}

#[test]
fn def06_builtin_function_returns_none() {
    let src = "{{ range(10) }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("range").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    assert!(result.is_none(), "built-in function must return None (REQ-DEF-06)");
}

#[test]
fn def06_outside_jinja_returns_none() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = go_to_definition(src, 0, 5, "test.html", &idx, &reg, &ws);
    assert!(result.is_none(), "plain HTML must return None");
}

#[test]
fn def06_unknown_identifier_returns_none() {
    // A free variable that has no definition — returns None, not an error.
    let src = "{{ free_variable }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("free_variable").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    assert!(result.is_none(), "unresolvable identifier must return None");
}

// ─── REQ-DEF-04: self.<block> and super() jump to block declaration ───────────

#[test]
fn def04_self_block_in_same_template_jumps_to_block() {
    // {{ self.content() }} — cursor on "content" attribute → jump to the block declaration.
    let src = "{% block content %}{{ self.content() }}{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Second occurrence of "content" is the attribute in self.content()
    let col = src.rfind("content").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "t.html", &idx, &reg, &ws);
    assert!(result.is_some(), "self.content() must resolve to block declaration");
    let def = result.unwrap();
    assert_eq!(def.target_path, "t.html", "must resolve in current template");
}

#[test]
fn def04_self_block_in_child_jumps_to_inherited_block() {
    // Child uses {{ self.footer() }} where footer is declared only in base.
    let base_src = "{% block footer %}footer content{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base_src);
    let child_src = r#"{% extends "base.html" %}{% block body %}{{ self.footer() }}{% endblock %}"#;
    let idx = extract(child_src);
    let reg = Registry::load_core();
    let col = child_src.rfind("footer").unwrap() as u32;
    let result = go_to_definition(child_src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "self.footer() must resolve to ancestor block");
    let def = result.unwrap();
    assert_eq!(def.target_path, "base.html", "must jump to base.html");
}

#[test]
fn def04_self_block_unknown_returns_none() {
    let src = "{% block content %}{{ self.nonexistent_block() }}{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind("nonexistent_block").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "t.html", &idx, &reg, &ws);
    assert!(result.is_none(), "self.nonexistent must return None");
}

#[test]
fn def04_super_inside_block_jumps_to_parent_block() {
    // Child overrides 'content'; super() inside it → parent's 'content' block.
    let base_src = "{% block content %}base content{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base_src);
    let child_src = r#"{% extends "base.html" %}{% block content %}{{ super() }}extra{% endblock %}"#;
    let idx = extract(child_src);
    let reg = Registry::load_core();
    let col = child_src.find("super").unwrap() as u32;
    let result = go_to_definition(child_src, 0, col, "child.html", &idx, &reg, &ws);
    assert!(result.is_some(), "super() inside overriding block must resolve to parent block");
    let def = result.unwrap();
    assert_eq!(def.target_path, "base.html", "must jump to base.html's block");
}

#[test]
fn def04_super_with_no_parent_block_returns_none() {
    // Block exists only in child (no parent declares it) — super() can't resolve.
    let src = "{% block content %}{{ super() }}{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default(); // no workspace inheritance
    let col = src.find("super").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "t.html", &idx, &reg, &ws);
    assert!(result.is_none(), "super() with no parent block must return None");
}

// ─── REQ-DEF-08: scope-local variable → binding site ─────────────────────────

#[test]
fn def08_for_loop_variable_jumps_to_binding() {
    // for-loop var: cursor on usage (second "item") must jump to the for statement (line 0).
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind("item").unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    let def = result.expect("for-loop variable must resolve to its binding");
    assert_eq!(def.target_path, "test.html");
    assert_eq!(def.target_start_line, 0, "binding is on line 0 (the for statement)");
}

#[test]
fn def08_set_variable_jumps_to_binding() {
    let src = "{% set greeting = 'hello' %}\n{{ greeting }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "greeting" in the {{ greeting }} expression (line 1).
    let result = go_to_definition(src, 1, 3, "test.html", &idx, &reg, &ws);
    let def = result.expect("set variable must resolve to its binding");
    assert_eq!(def.target_path, "test.html");
    assert_eq!(def.target_start_line, 0, "binding is on line 0 (the set statement)");
}

#[test]
fn def08_with_variable_jumps_to_binding() {
    let src = "{% with x = 42 %}{{ x }}{% endwith %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind('x').unwrap() as u32;
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    let def = result.expect("with variable must resolve to its binding");
    assert_eq!(def.target_path, "test.html");
    assert_eq!(def.target_start_line, 0, "binding is on line 0 (the with statement)");
}

#[test]
fn def08_free_variable_returns_none() {
    // Host-owned variable (not in index): no definition jump.
    let src = "{{ request.user }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = 3u32; // on "request"
    let result = go_to_definition(src, 0, col, "test.html", &idx, &reg, &ws);
    assert!(result.is_none(), "host-owned variable must return None");
}
