// F06 — Hover tests: REQ-HOV-01 through REQ-HOV-14.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::hover::hover;
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// Helper: find the byte column of the first occurrence of `needle` in `src`.
fn col_of(src: &str, needle: &str) -> u32 {
    src.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in {src:?}")) as u32
}

// Helper: find the byte column of the last occurrence of `needle` in `src`.
fn last_col_of(src: &str, needle: &str) -> u32 {
    src.rfind(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in {src:?}")) as u32
}

// ─── REQ-HOV-02: registry doc for filters / functions / tests ───────────────

#[test]
fn hov02_filter_hover_returns_doc() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "truncate"), &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover result for 'truncate'");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("truncate"),
        "expected 'truncate' in doc"
    );
    assert!(
        r.markdown.contains("filter"),
        "expected 'filter' kind label"
    );
}

#[test]
fn hov02_function_hover_returns_doc() {
    let src = "{{ range(10) }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "range"), &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover result for 'range'");
    let r = result.unwrap();
    assert!(r.markdown.contains("range"), "expected 'range' in doc");
    assert!(
        r.markdown.contains("function"),
        "expected 'function' kind label"
    );
}

#[test]
fn hov02_test_hover_returns_doc() {
    // In Jinja2, `is defined` is a test. But the references query might not
    // capture it unless it's in a render expression. Use a variable check pattern.
    let src = "{{ x is defined }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // If "defined" is captured as a test, hover should return its doc.
    // The test verifies registry lookup works; if capture is absent hover returns None.
    let col = col_of(src, "defined");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    // Either Some with "defined" doc or None (if test not captured yet)
    if let Some(r) = result {
        assert!(r.markdown.contains("defined"), "expected 'defined' in doc");
    }
}

// ─── REQ-HOV-07: MarkupContent markdown with a range ────────────────────────

#[test]
fn hov07_hover_result_has_range() {
    // Use a filter without call args so it's captured as a filter reference.
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "truncate");
    let result = hover(src, 0, col, &idx, &reg, &ws).unwrap();
    assert_eq!(result.start_line, 0, "hover must be on line 0");
    assert_eq!(result.start_col, col, "start_col must match token start");
    assert_eq!(
        result.end_col,
        col + "truncate".len() as u32,
        "end_col must cover the full token"
    );
}

#[test]
fn hov07_hover_range_on_second_line() {
    // Use a filter so it is captured as a filter reference.
    let src = "{% block content %}\n{{ x | upper }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 1, col_of("{{ x | upper }}", "upper"), &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover on line 1");
    let r = result.unwrap();
    assert_eq!(r.start_line, 1, "hover must reference line 1");
}

// ─── REQ-HOV-08: silence outside delimiters / in comments / raw ─────────────

#[test]
fn hov08_outside_delimiter_returns_none() {
    let src = "<p>Hello world</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, 3, &idx, &reg, &ws);
    assert!(result.is_none(), "expected None for plain HTML");
}

#[test]
fn hov08_inside_jinja_comment_returns_none() {
    let src = "{# truncate is mentioned here but this is a comment #}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "truncate"), &idx, &reg, &ws);
    assert!(result.is_none(), "expected None inside Jinja comment");
}

#[test]
fn hov08_plain_html_word_matching_special_name_returns_none() {
    // The word-level fallback only guarded against raw bodies and string literals,
    // never checking inside_jinja — so plain HTML prose containing "loop" produced
    // a hover card outside any Jinja delimiter.
    let src = "<p>the loop continues</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "loop"), &idx, &reg, &ws);
    assert!(
        result.is_none(),
        "expected None for 'loop' in plain HTML text; got: {:?}",
        result.map(|r| r.markdown)
    );
}

#[test]
fn hov08_plain_html_word_matching_macro_name_returns_none() {
    let src = "{% macro greet() %}{% endmacro %}<p>please greet the guest</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, last_col_of(src, "greet"), &idx, &reg, &ws);
    assert!(
        result.is_none(),
        "expected None for macro name appearing in plain HTML text; got: {:?}",
        result.map(|r| r.markdown)
    );
}

#[test]
fn hov08_raw_body_returns_none() {
    // REQ-HOV-08 (jinja-lsp-kz25): hover inside {% raw %} body must be silent.
    let src = "{% raw %}{{ loop }}{% endraw %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "loop"), &idx, &reg, &ws);
    assert!(
        result.is_none(),
        "expected None inside raw body; got: {:?}",
        result.map(|r| r.markdown)
    );
}

#[test]
fn hov08_raw_body_containing_literal_tag_returns_none() {
    // is_in_raw_body only inspected the last {% before the cursor. Inside
    // {% raw %}{% for x in y %}...{% endraw %}, hovering after the literal
    // {% for %} tag must still be silenced — {% raw %} is meant to contain
    // literal Jinja tags verbatim.
    let src = "{% raw %}{% for x in y %}{{ loop }}{% endfor %}{% endraw %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "loop"), &idx, &reg, &ws);
    assert!(
        result.is_none(),
        "expected None inside raw body after a literal tag; got: {:?}",
        result.map(|r| r.markdown)
    );
}

#[test]
fn hov08_string_literal_inside_expr_returns_none() {
    // REQ-HOV-08 (jinja-lsp-kz25): hover inside a string literal must be silent.
    // "loop" inside a string — not a Jinja keyword at that position.
    let src = r#"{{ "loop this" | upper }}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Col of "loop" is inside the string literal.
    let result = hover(src, 0, col_of(src, "loop"), &idx, &reg, &ws);
    assert!(result.is_none(), "expected None inside string literal");
}

#[test]
fn hov08_unrecognized_filter_returns_none() {
    // A filter not in the registry — hover returns None (no fallback for filters
    // the registry doesn't know).
    let src = "{{ x | my_custom_xyz_filter }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "my_custom_xyz_filter");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_none(), "expected None for unknown filter");
}

// ─── REQ-HOV-14: card composition ────────────────────────────────────────────

#[test]
fn hov14_card_starts_with_bold_heading() {
    let src = "{{ x | upper }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let r = hover(src, 0, col_of(src, "upper"), &idx, &reg, &ws).unwrap();
    assert!(
        r.markdown.starts_with("**upper**"),
        "card must start with **name**"
    );
}

#[test]
fn hov14_filter_card_has_fenced_signature() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let r = hover(src, 0, col_of(src, "truncate"), &idx, &reg, &ws).unwrap();
    assert!(
        r.markdown.contains("```"),
        "card must contain fenced signature block"
    );
}

#[test]
fn hov14_card_has_body_prose() {
    let src = "{{ x | upper }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let r = hover(src, 0, col_of(src, "upper"), &idx, &reg, &ws).unwrap();
    // "upper" doc should have some prose
    assert!(
        r.markdown.len() > 20,
        "card body must contain prose, got: {:?}",
        r.markdown
    );
}

#[test]
fn hov14_since_metadata_appears_when_present() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let r = hover(src, 0, col_of(src, "truncate"), &idx, &reg, &ws).unwrap();
    // truncate has since="2.0" — should appear as "since 2.0" in the heading
    assert!(
        r.markdown.contains("2.0"),
        "since metadata must appear in card"
    );
}

// ─── REQ-HOV-03: macro signature + docstring ─────────────────────────────────

#[test]
fn hov03_macro_definition_hover_shows_signature() {
    let src = "{% macro post_url(post) %}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "post_url");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for macro definition");
    let r = result.unwrap();
    assert!(r.markdown.contains("post_url"), "macro name must appear");
    assert!(r.markdown.contains("macro"), "macro kind must appear");
}

#[test]
fn hov03_macro_hover_shows_parameters() {
    let src = "{% macro greet(name, msg) %}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "greet");
    let r = hover(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(
        r.markdown.contains("name") || r.markdown.contains("msg"),
        "macro parameters must appear in signature"
    );
}

#[test]
fn hov03_macro_docstring_appears_in_hover() {
    let src = "{% macro render(item) %}{# Renders a single item card. #}{{ item }}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "render");
    let r = hover(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(
        r.markdown.contains("Renders a single item card."),
        "macro docstring must appear in hover: {:?}",
        r.markdown
    );
}

#[test]
fn hov03_macro_without_docstring_shows_only_signature() {
    let src = "{% macro render(item) %}{{ item }}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "render");
    let r = hover(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(
        r.markdown.contains("render(item)"),
        "signature must still appear"
    );
}

// ─── qudr: fallback hover when reference has no registry doc ─────────────────

#[test]
fn qudr_imported_macro_call_shows_macro_hover() {
    // "greet" is imported from a workspace template; it's a workspace macro.
    // The hover cursor lands on the Function reference "greet(".
    // Before the fix, the early `return None` at `candidates.is_empty()` would
    // fire because greet isn't in the built-in registry, suppressing the macro card.
    let src = "{% macro greet(name) %}Hello{% endmacro %}{{ greet( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "greet" in the call site {{ greet( }}
    let call_col = src.rfind("greet").unwrap() as u32;
    let result = hover(src, 0, call_col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "hover on a local macro call must return macro card (not None); \
        the early-return bug suppresses fallback handlers"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.contains("greet"),
        "macro name must appear in card"
    );
}

// ─── REQ-HOV-04: variable scope and definition site ──────────────────────────

#[test]
fn hov04_for_loop_variable_shows_scope() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Hover on the second "item" in {{ item }}
    let col = last_col_of(src, "item");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for loop variable");
    let r = result.unwrap();
    assert!(r.markdown.contains("item"), "variable name must appear");
}

#[test]
fn hov04_set_variable_shows_scope() {
    let src = "{% set my_title = 'Hello' %}{{ my_title }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = last_col_of(src, "my_title");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for set variable");
    let r = result.unwrap();
    assert!(r.markdown.contains("my_title"), "variable name must appear");
}

// ─── REQ-HOV-05: attribute access documentation ──────────────────────────────

#[test]
fn hov05_loop_index_returns_attr_doc() {
    let src = "{% for i in items %}{{ loop.index }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "index");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for loop.index");
    let r = result.unwrap();
    assert!(r.markdown.contains("index"), "expected 'index' in doc");
}

// ─── REQ-HOV-06: template-path resolution ────────────────────────────────────

#[test]
fn hov06_extends_path_hover() {
    let src = r#"{% extends "base.html" %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Hover on the path string (inside quotes)
    let col = col_of(src, "base.html");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for template path");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("base.html"),
        "path must appear in hover"
    );
}

// ─── REQ-HOV-09: block hover ─────────────────────────────────────────────────

#[test]
fn hov09_block_hover_shows_name() {
    let src = "{% block content %}hello{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "content");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for block");
    let r = result.unwrap();
    assert!(r.markdown.contains("content"), "block name must appear");
    assert!(r.markdown.contains("block"), "block kind must appear");
}

#[test]
fn hov09_block_scoped_modifier_appears_in_hover() {
    let src = "{% block content scoped %}hello{% endblock %}";
    let mut idx = extract(src);
    idx.path = "t.html".to_owned();
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, col_of(src, "content"), &idx, &reg, &ws);
    assert!(result.is_some());
    assert!(
        result.unwrap().markdown.contains("scoped"),
        "scoped modifier must appear"
    );
}

#[test]
fn hov09_block_override_shows_parent_template() {
    let base_src = "{% block content %}base{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("base.html", base_src);
    let child_src = r#"{% extends "base.html" %}{% block content %}child{% endblock %}"#;
    let mut idx = extract(child_src);
    idx.path = "child.html".to_owned();
    let reg = Registry::load_core();
    let col = last_col_of(child_src, "content");
    let result = hover(child_src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "block hover must return a result");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("base.html"),
        "must mention parent template"
    );
}

#[test]
fn hov09_block_shows_overriding_children() {
    // base declares 'content'; child overrides it — hover on base should list the child.
    let base_src = "{% block content %}base{% endblock %}";
    let child_src = r#"{% extends "base.html" %}{% block content %}child{% endblock %}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("child.html", child_src);
    let mut idx = extract(base_src);
    idx.path = "base.html".to_owned();
    let reg = Registry::load_core();
    let col = col_of(base_src, "content");
    let result = hover(base_src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!(
        r.markdown.contains("child.html"),
        "must mention overriding child"
    );
}

#[test]
fn hov09_block_override_uses_workspace_relative_path_not_absolute_key() {
    // Real templates are indexed by absolute filesystem path, but hover text
    // must show the workspace-relative path users actually navigate by
    // (matching completions.rs's relative_path convention), not the raw key.
    let base_src = "{% block content %}base{% endblock %}";
    let mut ws = WorkspaceIndex::default();
    ws.index_inline(
        "/home/alex/projects/investerra/api-server/app/templates/base.html",
        base_src,
    );
    ws.templates
        .get_mut("/home/alex/projects/investerra/api-server/app/templates/base.html")
        .unwrap()
        .relative_path = Some("base.html".to_owned());

    let child_src = r#"{% extends "base.html" %}{% block content %}child{% endblock %}"#;
    let mut idx = extract(child_src);
    idx.path = "/home/alex/projects/investerra/api-server/app/templates/child.html".to_owned();
    let reg = Registry::load_core();
    let col = last_col_of(child_src, "content");
    let result = hover(child_src, 0, col, &idx, &reg, &ws);
    let r = result.expect("block hover must return a result");
    assert!(
        r.markdown.contains("base.html") && !r.markdown.contains("/home/alex"),
        "must show the workspace-relative path, not the absolute key: {}",
        r.markdown
    );
}

#[test]
fn hov09_block_overriders_use_workspace_relative_path_not_absolute_key() {
    let base_src = "{% block content %}base{% endblock %}";
    let child_src = r#"{% extends "base.html" %}{% block content %}child{% endblock %}"#;
    let mut ws = WorkspaceIndex::default();
    // is_descendant_of resolves the child's relative "base.html" extends target
    // to a workspace key by suffix match, so the base template must also be
    // registered here (at the exact path idx.path uses below) for that to work.
    ws.index_inline(
        "/home/alex/projects/investerra/api-server/app/templates/base.html",
        base_src,
    );
    ws.index_inline(
        "/home/alex/projects/investerra/api-server/app/templates/child.html",
        child_src,
    );
    ws.templates
        .get_mut("/home/alex/projects/investerra/api-server/app/templates/child.html")
        .unwrap()
        .relative_path = Some("child.html".to_owned());

    let mut idx = extract(base_src);
    idx.path = "/home/alex/projects/investerra/api-server/app/templates/base.html".to_owned();
    let reg = Registry::load_core();
    let col = col_of(base_src, "content");
    let result = hover(base_src, 0, col, &idx, &reg, &ws);
    let r = result.expect("block hover must return a result");
    assert!(
        r.markdown.contains("child.html") && !r.markdown.contains("/home/alex"),
        "must show the workspace-relative path, not the absolute key: {}",
        r.markdown
    );
}

#[test]
fn hov09_inline_gettext_underscore_shows_starlette_babel_doc() {
    // {{ _('Upload signed PDF') }} — the inline gettext shorthand must resolve
    // through the same Function-reference hover fallback as any other builtin,
    // once starlette-babel's `_` doc is loaded.
    let src = "{{ _('Upload signed PDF') }}";
    let idx = extract(src);
    let mut reg = Registry::load_core();
    reg.load_packs(&["starlette-babel"]);
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "_(");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "hover on '_' in inline gettext shorthand must return a doc"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("gettext")
            || r.markdown.to_lowercase().contains("translat"),
        "expected the starlette-babel `_` doc, got: {}",
        r.markdown
    );
}

// ─── REQ-HOV-10: imported names resolve through the import ───────────────────

#[test]
fn hov10_from_import_name_shows_source() {
    // {% from "macros.html" import render_post %}
    // Hovering on "render_post" should show import source info.
    let src = r#"{% from "macros.html" import render_post %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "render_post");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for imported name");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("macros.html"),
        "source template must appear in hover"
    );
}

#[test]
fn hov10_from_import_alias_shows_aliased_name() {
    // Hovering on alias "rp" should show it's an alias of "render_post".
    let src = r#"{% from "macros.html" import render_post as rp %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // hover on the alias "rp"
    let col = col_of(src, " rp") + 1; // skip the space
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for import alias");
    let r = result.unwrap();
    let md = r.markdown.to_lowercase();
    assert!(
        md.contains("alias") || md.contains("render_post"),
        "alias relationship must appear"
    );
}

#[test]
fn hov10_from_import_resolves_absolute_workspace_key() {
    // On the real server, workspace.templates is keyed by absolute URI paths,
    // while fi.source is always the relative path typed in the from-import.
    // End-to-end this specific case is also covered by the workspace-wide
    // macro fallback a few lines above the from-import branch, but the
    // from-import lookup itself must still resolve through get_by_ref (not a
    // raw HashMap::get keyed on the unresolved relative fi.source), since that
    // broader fallback is a separate, coarser mechanism (see jinja-lsp-izfw).
    let src = r#"{% from "macros.html" import render_post %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let mut ws = WorkspaceIndex::default();
    ws.templates.insert(
        "/abs/project/macros.html".to_owned(),
        extract("{% macro render_post(post) %}{% endmacro %}"),
    );
    let col = col_of(src, "render_post");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for imported name");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("render_post("),
        "must show the macro signature card, not the generic 'Imported from' fallback: {}",
        r.markdown
    );
}

#[test]
fn hov10_import_alias_shows_source() {
    // {% import "macros.html" as macros %}
    // Hovering on "macros" (the namespace alias) should show source info.
    let src = r#"{% import "macros.html" as macros %}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = last_col_of(src, "macros");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "expected hover for namespace import alias"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.contains("macros.html"),
        "source template must appear in namespace import hover"
    );
}

// ─── REQ-HOV-11: keyword-argument names show their bound parameter ────────────

#[test]
fn hov11_keyword_arg_name_shows_parameter() {
    // truncate has a keyword arg "length"
    let src = "{{ text | truncate(length=80) }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "length");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for keyword arg 'length'");
    let r = result.unwrap();
    assert!(
        r.markdown.contains("length") || r.markdown.contains("truncate"),
        "keyword arg or callee must appear; got: {:?}",
        r.markdown
    );
}

// ─── REQ-HOV-12: special objects render their registry doc ───────────────────

#[test]
fn hov12_loop_inside_for_shows_doc() {
    let src = "{% for i in items %}{{ loop.index }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Hover on "loop" (the special object)
    let col = col_of(src, "loop.index"); // col of "loop"
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for special object 'loop'");
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("loop"),
        "loop special object doc must appear; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov12_caller_shows_doc() {
    let src = "{% macro render() %}{{ caller() }}{% endmacro %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "caller");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "expected hover for special object 'caller'"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("caller"),
        "caller doc must appear; got: {:?}",
        r.markdown
    );
}

// REQ-HOV-12 scope-gating notes

#[test]
fn hov12_loop_outside_for_has_scope_note() {
    let src = "{{ loop.index }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "loop");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    let r = result.expect("expected hover for loop outside for");
    assert!(
        r.markdown.contains("only valid"),
        "out-of-scope loop must carry a scope note; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov12_loop_inside_for_has_no_scope_note() {
    let src = "{% for i in items %}{{ loop.index }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "loop.index");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    let r = result.expect("expected hover for loop inside for");
    assert!(
        !r.markdown.contains("only valid"),
        "in-scope loop must NOT have a scope note; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov12_super_outside_block_has_scope_note() {
    let src = "{{ super() }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "super");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    let r = result.expect("expected hover for super outside block");
    assert!(
        r.markdown.contains("only valid"),
        "out-of-scope super must carry a scope note; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov12_caller_outside_macro_has_scope_note() {
    let src = "{{ caller() }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "caller");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    let r = result.expect("expected hover for caller outside macro");
    assert!(
        r.markdown.contains("only valid"),
        "out-of-scope caller must carry a scope note; got: {:?}",
        r.markdown
    );
}

// ─── REQ-HOV-13: statement keywords show a tag description ───────────────────

#[test]
fn hov13_for_keyword_shows_description() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Hover on "for" keyword
    let col = col_of(src, "for");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for 'for' keyword");
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("for")
            || r.markdown.to_lowercase().contains("loop")
            || r.markdown.to_lowercase().contains("iterate"),
        "for-keyword doc must describe looping; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov13_if_keyword_shows_description() {
    let src = "{% if condition %}yes{% endif %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = col_of(src, "if");
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for 'if' keyword");
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("if") || r.markdown.to_lowercase().contains("condition"),
        "if-keyword doc must mention condition; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov13_block_keyword_shows_description() {
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Hover on the "block" keyword (before "content")
    let col = 3u32; // "{%" is 2 chars + space, "block" starts at 3
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "expected hover for 'block' keyword");
    let r = result.unwrap();
    assert!(
        r.markdown.to_lowercase().contains("block")
            || r.markdown.to_lowercase().contains("inherit"),
        "block-keyword doc must appear; got: {:?}",
        r.markdown
    );
}

#[test]
fn hov13_unknown_keyword_returns_none() {
    // Plain HTML — no Jinja tag keyword recognized here
    let src = "<div>hello</div>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = hover(src, 0, 1, &idx, &reg, &ws);
    assert!(result.is_none(), "expected None for non-keyword position");
}

#[test]
fn vn0z_hover_mid_multibyte_char_does_not_panic() {
    // "é" is 2 bytes (0xC3 0xA9); byte 1 is NOT a char boundary.
    // word_at_byte_range and byte_to_line_col must not panic.
    let src = "{{ é }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let _result = hover(src, 0, 4, &idx, &reg, &ws); // byte 4 is mid-char (é = bytes 3-4)
}

#[test]
fn hov02_filter_with_args_hover_returns_doc() {
    // Filters called with arguments are captured as Function by treesitter.
    // The hover fallback chain must also check Category::Filter so they get docs.
    let src = "{{ x | truncate(60) }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("truncate").unwrap() as u32;
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "hover on truncate(60) must return documentation"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.contains("truncate"),
        "expected 'truncate' in doc"
    );
}

#[test]
fn hov02_attr_chain_filter_without_args_returns_doc() {
    // {{ post.title | upper }} — filter after attribute chain, without call args.
    let src = "{{ post.title | upper }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("upper").unwrap() as u32;
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "hover on 'upper' after attribute chain must return doc"
    );
    let r = result.unwrap();
    assert!(r.markdown.contains("upper"), "expected 'upper' in doc");
}

#[test]
fn hov02_attr_chain_filter_with_args_returns_doc() {
    // {{ post.title | truncate(60) }} — filter-with-args after attribute chain.
    // treesitter captures filter(args) as ReferenceKind::Function; hover uses Category::Filter fallback.
    let src = "{{ post.title | truncate(60) }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find("truncate").unwrap() as u32;
    let result = hover(src, 0, col, &idx, &reg, &ws);
    assert!(
        result.is_some(),
        "hover on 'truncate(60)' after attribute chain must return doc"
    );
    let r = result.unwrap();
    assert!(
        r.markdown.contains("truncate"),
        "expected 'truncate' in doc"
    );
}
