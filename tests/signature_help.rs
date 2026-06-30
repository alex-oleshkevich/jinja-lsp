// F07 — Signature help tests: REQ-SIG-01 through REQ-SIG-05.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::signature_help::signature_help;
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── REQ-SIG-01: trigger on ( and , ; null outside calls / delimiters ─────────

#[test]
fn sig01_returns_none_outside_jinja() {
    let src = "<p>Hello(world)</p>";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let result = signature_help(src, 0, 9, &idx, &reg, &ws);
    assert!(result.is_none(), "plain HTML must return None");
}

#[test]
fn sig01_returns_none_not_inside_call() {
    let src = "{{ truncate }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor on "truncate" but not inside parens
    let col = src.find("truncate").unwrap() as u32;
    let result = signature_help(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_none(), "not inside a call must return None");
}

#[test]
fn sig01_returns_none_unknown_callee() {
    let src = "{{ no_such_func_xyz( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let result = signature_help(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_none(), "unknown callee must return None");
}

// ─── REQ-SIG-02: signatures from macro params and registry ────────────────────

#[test]
fn sig02_macro_signature_from_params() {
    let src = "{% macro greet(name, msg='hi') %}{% endmacro %}{{ greet( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind('(').unwrap() as u32 + 1;
    let result = signature_help(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "macro call must return signature");
    let sh = result.unwrap();
    assert!(sh.label.contains("greet"), "label must contain macro name");
    assert!(!sh.params.is_empty(), "macro must have params");
    let param_labels: Vec<&str> = sh.params.iter().map(|p| p.label.as_str()).collect();
    assert!(param_labels.iter().any(|l| l.contains("name")), "must have 'name' param: {param_labels:?}");
}

#[test]
fn sig02_function_signature_from_registry() {
    let src = "{{ range( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let result = signature_help(src, 0, col, &idx, &reg, &ws);
    assert!(result.is_some(), "range() call must return signature");
    let sh = result.unwrap();
    assert!(sh.label.contains("range"), "label must contain 'range'");
    // range has params: start, stop, step
    assert!(!sh.params.is_empty(), "range must have params");
}

// ─── REQ-SIG-03: filter call — receiver is NOT included in params ─────────────

#[test]
fn sig03_filter_call_first_explicit_arg_is_index_0() {
    // The registry omits the implicit receiver — params start at the first explicit arg.
    // For "{{ x | truncate(", comma_count=0, so active must be 0 (= length).
    let src = "{{ x | truncate( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(sh.label.contains("truncate"), "label must contain 'truncate'");
    assert_eq!(
        sh.active_parameter, Some(0),
        "filter first explicit arg is param[0] (receiver not in registry params): {:?}",
        sh.active_parameter
    );
}

#[test]
fn sig03_filter_call_shows_explicit_params() {
    // Registry params for truncate: [length, killwords, end, leeway] — no receiver.
    let src = "{{ x | truncate( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(sh.params.len() >= 2, "truncate must have at least 2 params");
    let labels: Vec<&str> = sh.params.iter().map(|p| p.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.contains("length")), "must include 'length' param: {labels:?}");
}

// ─── REQ-SIG-04: active parameter re-resolves across commas ──────────────────

#[test]
fn sig04_first_arg_active_param_is_zero() {
    let src = "{{ range( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert_eq!(sh.active_parameter, Some(0), "first arg must have active_parameter=0");
}

#[test]
fn sig04_comma_advances_active_param() {
    // After one comma in range(start, <here>) the active param advances to 1.
    let src = "{{ range(1, }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find(',').unwrap() as u32 + 2; // cursor after ", "
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert_eq!(sh.active_parameter, Some(1), "after one comma active_parameter must be 1");
}

#[test]
fn sig04_nested_comma_not_counted() {
    // The comma inside max(1,2) is nested, must not advance the active param.
    let src = "{{ range(max(1,2), }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor after the outer comma (after "max(1,2), ")
    let outer_comma = src.rfind(',').unwrap() as u32 + 2;
    let sh = signature_help(src, 0, outer_comma, &idx, &reg, &ws).unwrap();
    assert_eq!(sh.active_parameter, Some(1), "nested commas must not bump the active param");
}

#[test]
fn sig04_past_last_param_shows_no_active() {
    // range has 3 params; cursor after 3 commas = past last.
    let src = "{{ range(1, 2, 3, }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind(',').unwrap() as u32 + 2;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    // Should be Some with index >= params.len(), or None for active.
    // Spec: "no active highlight rather than erroring" → active_parameter = None
    assert!(
        sh.active_parameter.is_none() || sh.active_parameter.unwrap() >= sh.params.len(),
        "past-last param must return None or out-of-range active index"
    );
}

#[test]
fn sig04_filter_comma_advances_active_param() {
    // truncate(60, <here>): comma_count=1 → active=1 (= killwords, the second explicit arg).
    // Receiver is NOT in params, so no +1 offset.
    let src = "{{ x | truncate(60, }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind(',').unwrap() as u32 + 2;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert_eq!(sh.active_parameter, Some(1), "second explicit arg in filter call = param[1] (killwords)");
}

// ─── REQ-SIG-05: response shape — active parameter index ─────────────────────

#[test]
fn sig05_response_has_label_and_params() {
    let src = "{{ range( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(!sh.label.is_empty(), "label must not be empty");
    assert!(!sh.params.is_empty(), "params must not be empty");
}

#[test]
fn sig05_param_has_label() {
    let src = "{{ range( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.find('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    for p in &sh.params {
        assert!(!p.label.is_empty(), "each param must have a non-empty label");
    }
}

// ─── trc3: dict-literal commas must not count as argument separators ─────────

#[test]
fn trc3_dict_literal_commas_not_counted_as_arg_separators() {
    // foo({"a": 1, "b": 2}, bar) — cursor after the comma following the dict.
    // active_parameter should be 1 (second arg = bar), not 2 (wrong count from dict comma).
    let src = r#"{% macro foo(d, x) %}{% endmacro %}{{ foo({"a": 1, "b": 2}, "bar") }}"#;
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor right after the closing `}` of the dict, before `"bar"`.
    let cursor = src.rfind(", \"bar\"").unwrap() as u32 + 2; // past the comma+space
    let sh = signature_help(src, 0, cursor, &idx, &reg, &ws).unwrap();
    assert_eq!(
        sh.active_parameter,
        Some(1),
        "cursor after dict arg should be param[1]; got {:?} (dict commas being miscounted)",
        sh.active_parameter
    );
}

#[test]
fn sig05_macro_param_default_in_label() {
    // macro greet(name, msg='hi') — the default 'hi' should appear in param label.
    let src = "{% macro greet(name, msg='hi') %}{% endmacro %}{{ greet( }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind('(').unwrap() as u32 + 1;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    let has_default = sh.params.iter().any(|p| p.label.contains('='));
    assert!(has_default, "param with default must include '=' in label: {:?}", sh.params);
}

// ─── jinja-lsp-gmy7: nested call shows innermost callee ──────────────────────

#[test]
fn sig_gmy7_nested_call_shows_innermost_callee() {
    // Cursor inside `inner(2, ` — signature help must show `inner`, not `range`.
    // `inner` is a local macro so it's definitely resolvable.
    let src = "{% macro inner(a, b) %}{% endmacro %}{{ range(1, inner(2, }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    // Cursor after the last comma+space (inside inner's arg list).
    let col = src.rfind(',').unwrap() as u32 + 2;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert!(
        sh.label.contains("inner"),
        "nested call: label must be 'inner', not 'range': {}",
        sh.label
    );
}

#[test]
fn sig_gmy7_nested_call_active_param_is_local_to_inner() {
    // After `inner(2, `, comma_count inside inner is 1 → active_parameter = Some(1).
    let src = "{% macro inner(a, b) %}{% endmacro %}{{ range(1, inner(2, }}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let col = src.rfind(',').unwrap() as u32 + 2;
    let sh = signature_help(src, 0, col, &idx, &reg, &ws).unwrap();
    assert_eq!(
        sh.active_parameter, Some(1),
        "inside inner(2, cursor), local comma count is 1 so active param is Some(1): {:?}",
        sh.active_parameter
    );
}
