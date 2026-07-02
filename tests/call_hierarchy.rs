// F16 — Call hierarchy tests: REQ-CALL-01 through REQ-CALL-04.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::call_hierarchy::{
    incoming_calls, outgoing_calls, prepare_call_hierarchy, ItemKind,
};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn ws(templates: &[(&str, &str)]) -> WorkspaceIndex {
    let mut w = WorkspaceIndex::default();
    for (path, src) in templates {
        w.index_inline(path, src);
    }
    w
}

fn reg() -> Registry {
    Registry::load_core()
}

fn col_of(src: &str, word: &str) -> u32 {
    src.find(word).unwrap_or(0) as u32
}

// ─── REQ-CALL-01: Prepare ────────────────────────────────────────────────────

#[test]
fn call01_prepare_at_definition_returns_item() {
    let src = "{% macro greet(name) %}hello{% endmacro %}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);
    let items = prepare_call_hierarchy(src, 0, col_of(src, "greet"), "t.html", &idx, &w, &reg());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "greet");
    assert_eq!(items[0].kind, ItemKind::Function);
    assert_eq!(items[0].uri, "t.html");
}

// ─── jinja-lsp-jls2: selection_range must anchor at the name, not the keyword ─

#[test]
fn call01_prepare_selection_range_anchors_at_macro_name_not_keyword() {
    let src = "{% macro greet(name) %}hello{% endmacro %}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);
    let items = prepare_call_hierarchy(src, 0, col_of(src, "greet"), "t.html", &idx, &w, &reg());
    let item = &items[0];
    let name_col = col_of(src, "greet");
    assert_eq!(
        item.selection_range.start_col, name_col,
        "selection_range must start at 'greet', not the 'macro' keyword"
    );
    assert_eq!(item.selection_range.end_col, name_col + "greet".len() as u32);
}

#[test]
fn call01_prepare_at_call_site_anchors_to_definition() {
    let macro_src = "{% macro greet(name) %}hello{% endmacro %}";
    let caller_src = "{{ greet('Bob') }}";
    let w = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let caller_idx = extract(caller_src);
    let items = prepare_call_hierarchy(
        caller_src,
        0,
        col_of(caller_src, "greet"),
        "caller.html",
        &caller_idx,
        &w,
        &reg(),
    );
    assert_eq!(items.len(), 1, "call site must resolve to definition");
    assert_eq!(items[0].name, "greet");
    assert_eq!(items[0].uri, "macro.html", "item must anchor to definition file");
}

#[test]
fn call01_prepare_at_non_macro_returns_empty() {
    // Plain variable — not a callable.
    let src = "{{ x }}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);
    let items = prepare_call_hierarchy(src, 0, 3, "t.html", &idx, &w, &reg());
    assert!(items.is_empty(), "plain variable must return empty");
}

#[test]
fn call01_prepare_at_block_returns_empty() {
    // Blocks are not macros; no item returned.
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);
    // cursor on "content" inside the block statement
    let items = prepare_call_hierarchy(src, 0, col_of(src, "content"), "t.html", &idx, &w, &reg());
    assert!(items.is_empty(), "block keyword is not a callable");
}

#[test]
fn call01_prepare_over_filter_returns_empty() {
    let src = "{{ x | truncate }}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);
    let items = prepare_call_hierarchy(src, 0, col_of(src, "truncate"), "t.html", &idx, &w, &reg());
    assert!(items.is_empty(), "filter is not a user macro");
}

#[test]
fn call01_prepare_via_from_import_anchors_to_definition() {
    // Cursor on the call `post_url(post)` in a file that from-imports post_url.
    let macro_src = "{% macro post_url(post) %}{{ post.slug }}{% endmacro %}";
    let caller_src = "{% from 'macros.html' import post_url %}{{ post_url(post) }}";
    let w = ws(&[("macros.html", macro_src), ("caller.html", caller_src)]);
    let caller_idx = extract(caller_src);
    // Find "post_url" in "{{ post_url(post) }}" — the call site (not the import line)
    let call_pos = caller_src.rfind("post_url").unwrap() as u32;
    let items = prepare_call_hierarchy(
        caller_src,
        0,
        call_pos,
        "caller.html",
        &caller_idx,
        &w,
        &reg(),
    );
    assert_eq!(items.len(), 1, "from-imported call must resolve to definition");
    assert_eq!(items[0].name, "post_url");
    assert_eq!(items[0].uri, "macros.html", "item anchors to definition file");
}

// ─── REQ-CALL-02: Incoming calls ─────────────────────────────────────────────

#[test]
fn call02_incoming_empty_when_no_callers() {
    let src = "{% macro unused() %}{% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let idx = extract(src);
    let mut items =
        prepare_call_hierarchy(src, 0, col_of(src, "unused"), "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    assert!(incoming_calls(&item, &w).is_empty(), "no callers → empty list");
}

#[test]
fn call02_incoming_two_calls_from_template_merged_with_two_ranges() {
    let macro_src = "{% macro greet() %}{% endmacro %}";
    let caller_src = "{{ greet() }}{{ greet() }}";
    let w = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let macro_idx = extract(macro_src);
    let mut items = prepare_call_hierarchy(
        macro_src,
        0,
        col_of(macro_src, "greet"),
        "macro.html",
        &macro_idx,
        &w,
        &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);
    assert_eq!(calls.len(), 1, "two calls from same template → one IncomingCall");
    assert_eq!(calls[0].from_ranges.len(), 2, "two call sites → two fromRanges");
}

#[test]
fn call02_incoming_template_level_caller_is_module_item() {
    // Call at template top level (not inside a macro) → from item is the template (Module).
    let macro_src = "{% macro ping() %}{% endmacro %}";
    let caller_src = "{{ ping() }}";
    let w = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let macro_idx = extract(macro_src);
    let mut items = prepare_call_hierarchy(
        macro_src,
        0,
        col_of(macro_src, "ping"),
        "macro.html",
        &macro_idx,
        &w,
        &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].from.kind, ItemKind::Module, "template-level call → Module from");
    assert!(calls[0].from.uri.contains("caller.html"), "from uri points to calling template");
}

#[test]
fn call02_incoming_enclosing_macro_is_from_item() {
    // Call inside a wrapper macro body → the from item is the enclosing macro, not the template.
    let macro_src = "{% macro greet() %}{% endmacro %}";
    let caller_src = "{% macro wrapper() %}{{ greet() }}{% endmacro %}";
    let w = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let macro_idx = extract(macro_src);
    let mut items = prepare_call_hierarchy(
        macro_src,
        0,
        col_of(macro_src, "greet"),
        "macro.html",
        &macro_idx,
        &w,
        &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].from.kind, ItemKind::Function, "enclosing macro → Function from");
    assert_eq!(calls[0].from.name, "wrapper");
}

#[test]
fn call02_incoming_two_macros_in_one_template_are_two_entries() {
    // T08b: macros a() and b() in the same template both call ping() → two IncomingCalls.
    let macro_src = "{% macro ping() %}{% endmacro %}";
    let caller_src =
        "{% macro a() %}{{ ping() }}{% endmacro %}{% macro b() %}{{ ping() }}{% endmacro %}";
    let w = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let macro_idx = extract(macro_src);
    let mut items = prepare_call_hierarchy(
        macro_src,
        0,
        col_of(macro_src, "ping"),
        "macro.html",
        &macro_idx,
        &w,
        &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);
    assert_eq!(calls.len(), 2, "two enclosing macros → two IncomingCalls");
    let names: Vec<&str> = calls.iter().map(|c| c.from.name.as_str()).collect();
    assert!(names.contains(&"a"), "a must be in from items");
    assert!(names.contains(&"b"), "b must be in from items");
}

// ─── REQ-CALL-03: Outgoing calls ─────────────────────────────────────────────

#[test]
fn call03_outgoing_empty_for_leaf_macro() {
    let src = "{% macro leaf() %}plain text{% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let idx = extract(src);
    let mut items =
        prepare_call_hierarchy(src, 0, col_of(src, "leaf"), "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    assert!(outgoing_calls(&item, &w, &reg()).is_empty(), "leaf macro → empty outgoing");
}

#[test]
fn call03_outgoing_local_macro_call_is_function_edge() {
    let src = "{% macro inner() %}{% endmacro %}{% macro outer() %}{{ inner() }}{% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let idx = extract(src);
    let outer_col = src.rfind("outer").unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, outer_col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].to.name, "inner");
    assert_eq!(calls[0].to.kind, ItemKind::Function);
}

#[test]
fn call03_outgoing_include_is_module_edge() {
    let src = "{% macro m() %}{% include 'base.html' %}{% endmacro %}";
    let w = ws(&[("t.html", src), ("base.html", "<!DOCTYPE html>")]);
    let idx = extract(src);
    let m_col = src.find("macro m").map(|i| i + 6).unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, m_col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert!(
        calls.iter().any(|c| c.to.kind == ItemKind::Module && c.to.name.contains("base.html")),
        "include inside macro body → Module outgoing edge; got: {:?}",
        calls.iter().map(|c| &c.to.name).collect::<Vec<_>>()
    );
}

#[test]
fn call03_outgoing_import_is_module_edge() {
    let src = "{% macro m() %}{% import 'helpers.html' as h %}{% endmacro %}";
    let w = ws(&[("t.html", src), ("helpers.html", "")]);
    let idx = extract(src);
    let m_col = src.find("macro m").map(|i| i + 6).unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, m_col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert!(
        calls.iter().any(|c| c.to.kind == ItemKind::Module && c.to.name.contains("helpers.html")),
        "import inside macro body → Module outgoing edge"
    );
}

#[test]
fn call03_template_level_include_not_attributed_to_macro() {
    // The include is OUTSIDE the macro body → should not appear in macro's outgoing.
    let src = "{% include 'header.html' %}{% macro m() %}plain{% endmacro %}";
    let w = ws(&[("t.html", src), ("header.html", "")]);
    let idx = extract(src);
    let m_col = src.find("macro m").map(|i| i + 6).unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, m_col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert!(
        calls.iter().all(|c| !c.to.name.contains("header.html")),
        "template-level include must NOT appear in macro outgoing calls"
    );
}

// ─── REQ-CALL-04: One level per request; cycle termination ───────────────────

#[test]
fn call04_outgoing_is_one_level_only() {
    // outer → inner → core: outgoing from outer yields only inner, not core.
    let src = "{% macro core() %}{% endmacro %}\
               {% macro inner() %}{{ core() }}{% endmacro %}\
               {% macro outer() %}{{ inner() }}{% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let idx = extract(src);
    let outer_col = src.rfind("outer").unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, outer_col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert_eq!(calls.len(), 1, "one level only — core must not appear directly");
    assert_eq!(calls[0].to.name, "inner");
}

#[test]
fn call04_cycle_incoming_terminates() {
    // a calls b, b calls a — incoming on a must terminate.
    let a_src = "{% macro a() %}{{ b() }}{% endmacro %}";
    let b_src = "{% macro b() %}{{ a() }}{% endmacro %}";
    let w = ws(&[("a.html", a_src), ("b.html", b_src)]);
    let a_idx = extract(a_src);
    let col = a_src.find("macro a").map(|i| i + 6).unwrap() as u32;
    let mut items =
        prepare_call_hierarchy(a_src, 0, col, "a.html", &a_idx, &w, &reg());
    let item = items.remove(0);
    // must terminate and return exactly one level
    let calls = incoming_calls(&item, &w);
    // b.html calls a, so there's one caller
    assert_eq!(calls.len(), 1, "b.html calls a → one IncomingCall");
    assert!(calls[0].from.name == "b" || calls[0].from.uri.contains("b.html"));
}

#[test]
fn call04_cycle_outgoing_terminates() {
    // a calls b, b calls a — outgoing from a returns only b (one level).
    let src =
        "{% macro a() %}{{ b() }}{% endmacro %}{% macro b() %}{{ a() }}{% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let idx = extract(src);
    let col = src.find("macro a").map(|i| i + 6).unwrap() as u32;
    let mut items = prepare_call_hierarchy(src, 0, col, "t.html", &idx, &w, &reg());
    let item = items.remove(0);
    let calls = outgoing_calls(&item, &w, &reg());
    assert_eq!(calls.len(), 1, "one level only from a");
    assert_eq!(calls[0].to.name, "b");
}

// ─── jinja-lsp-8b7x: aliased from-import macro ───────────────────────────────

#[test]
fn call_hierarchy_prepare_resolves_aliased_from_import() {
    // {% from "macros.html" import greet as salute %} — cursor on "salute()" call should
    // resolve to the "greet" macro in macros.html.
    let macro_src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}";
    // Use a unique alias ("salute") so col_of finds the call site, not the declaration.
    let caller_src = r#"{% from "macros.html" import greet as salute %}{{ salute("World") }}"#;
    let mut ws = WorkspaceIndex::default();
    ws.index_inline("macros.html", macro_src);
    let caller_idx = extract(caller_src);
    // col_of finds the first "salute" which is in the declaration; use the call site.
    let call_col = caller_src.find("salute(").unwrap() as u32;
    let items = prepare_call_hierarchy(caller_src, 0, call_col, "caller.html", &caller_idx, &ws, &reg());
    assert!(!items.is_empty(), "prepare must resolve aliased from-import to greet macro");
    assert_eq!(items[0].name, "greet", "resolved name must be the real macro name, not the alias");
    assert_eq!(items[0].uri, "macros.html");
}

#[test]
fn call04_nested_macro_body_does_not_steal_enclosing_endmacro() {
    // outer defines inner inside its body; a reference to ping() inside outer
    // but after inner's body must still belong to outer, not be attributed to
    // inner (verifies that the nesting depth counter works correctly).
    let src = "{% macro ping() %}{% endmacro %}\
               {% macro outer() %}\
               {% macro inner() %}{% endmacro %}\
               {{ ping() }}\
               {% endmacro %}";
    let w = ws(&[("t.html", src)]);
    let ping_idx = extract("{% macro ping() %}{% endmacro %}");
    let ping_src = "{% macro ping() %}{% endmacro %}";
    let mut items = prepare_call_hierarchy(
        ping_src, 0, col_of(ping_src, "ping"), "t.html",
        &ping_idx, &w, &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);
    // ping() is called inside outer's body (not inner's), so the enclosing macro is outer.
    assert_eq!(calls.len(), 1, "one IncomingCall from outer");
    assert_eq!(calls[0].from.name, "outer", "ping() is enclosed by outer, not inner");
    assert_eq!(calls[0].from.kind, ItemKind::Function);
}

// ─── jinja-lsp-6cbt: global_item detail/URI shape ────────────────────────────

#[test]
fn call6cbt_global_function_detail_and_uri_use_pack_prefix() {
    // A registry function from a named pack must produce:
    //   detail = "global - <pack> pack"
    //   uri    = "jinja-builtin:<pack>/<name>"
    use jinja_lsp::builtins::registry::{Category, DocEntry, Source};

    let src = "{% macro caller() %}{{ url_for('index') }}{% endmacro %}";
    let idx = extract(src);
    let w = ws(&[("t.html", src)]);

    // Build a registry with url_for coming from a "starlette" pack.
    let mut r = reg();
    r.insert(DocEntry {
        name: "url_for".to_owned(),
        category: Category::Function,
        source: Source::Pack("starlette".to_owned()),
        signature: None,
        since: None,
        params: vec![],
        body: String::new(),
        ty: None,
        template: None,
    });

    let item = prepare_call_hierarchy(src, 0, col_of(src, "caller"), "t.html", &idx, &w, &r)
        .into_iter().next().expect("must prepare item");
    let calls = outgoing_calls(&item, &w, &r);
    let url_for_edge = calls.iter().find(|c| c.to.name == "url_for").expect("url_for must be a outgoing edge");

    assert_eq!(url_for_edge.to.detail, "global - starlette pack", "detail must name the pack");
    assert_eq!(url_for_edge.to.uri, "jinja-builtin:starlette/url_for", "URI must include pack prefix");
}

// ─── jinja-lsp-1dzt: incoming_calls must respect macro shadowing ────────────

#[test]
fn call02_incoming_calls_excludes_shadowing_template_local_macro() {
    // shadow.html defines its OWN local "greet" macro and calls it — those
    // calls belong to the local macro, not the one defined in macro.html.
    let macro_src = "{% macro greet() %}{% endmacro %}";
    let caller_src = "{{ greet() }}";
    let shadow_src = "{% macro greet() %}{% endmacro %}{{ greet() }}";
    let w = ws(&[
        ("macro.html", macro_src),
        ("caller.html", caller_src),
        ("shadow.html", shadow_src),
    ]);
    let macro_idx = extract(macro_src);
    let mut items = prepare_call_hierarchy(
        macro_src, 0, col_of(macro_src, "greet"), "macro.html", &macro_idx, &w, &reg(),
    );
    let item = items.remove(0);
    let calls = incoming_calls(&item, &w);

    let uris: Vec<&str> = calls.iter().map(|c| c.from.uri.as_str()).collect();
    assert!(uris.contains(&"caller.html"), "caller.html must call the real macro.html greet: {uris:?}");
    assert!(
        !uris.contains(&"shadow.html"),
        "shadow.html's own local greet() call must NOT be attributed to macro.html's greet: {uris:?}"
    );
}

