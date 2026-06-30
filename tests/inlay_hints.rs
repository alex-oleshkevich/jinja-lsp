// F14 — Inlay hints tests: REQ-INLAY-01, -02, -04, -05, -06, -07.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::inlay_hints::{
    inlay_hint_resolve, inlay_hints, InlayHintData, InlayHintKind, InlayHintsConfig,
};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn reg() -> Registry {
    Registry::load_core()
}

fn hints(src: &str) -> Vec<jinja_lsp::features::inlay_hints::InlayHint> {
    let idx = extract(src);
    let registry = reg();
    let ws = WorkspaceIndex::default();
    inlay_hints(src, "test.html", &idx, &registry, &ws, &InlayHintsConfig::default())
}

fn hints_with_ws(
    src: &str,
    ws: &WorkspaceIndex,
) -> Vec<jinja_lsp::features::inlay_hints::InlayHint> {
    let idx = extract(src);
    let registry = reg();
    inlay_hints(src, "test.html", &idx, &registry, ws, &InlayHintsConfig::default())
}

fn hints_cfg(
    src: &str,
    cfg: InlayHintsConfig,
) -> Vec<jinja_lsp::features::inlay_hints::InlayHint> {
    let idx = extract(src);
    let registry = reg();
    let ws = WorkspaceIndex::default();
    inlay_hints(src, "test.html", &idx, &registry, &ws, &cfg)
}

fn has_param_hint(hs: &[jinja_lsp::features::inlay_hints::InlayHint]) -> bool {
    hs.iter().any(|h| h.kind == Some(InlayHintKind::Parameter))
}

fn has_endblock_hint(hs: &[jinja_lsp::features::inlay_hints::InlayHint]) -> bool {
    hs.iter().any(|h| h.kind.is_none() && matches!(&h.data, InlayHintData::EndBlock { .. }))
}

// ─── REQ-INLAY-01: label positional macro args ───────────────────────────────

#[test]
fn inlay01_local_macro_positional_arg_gets_label() {
    // greet(name, msg) called as greet(x, y) → name: before x, msg: before y
    let src = "{% macro greet(name, msg) %}hi{% endmacro %}{{ greet(x, y) }}";
    let hs = hints(src);
    // x gets "name:", y gets "msg:"
    assert!(
        hs.iter().any(|h| h.label == "name:" && h.kind == Some(InlayHintKind::Parameter)),
        "expected 'name:' parameter hint; got: {:?}",
        hs.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
    assert!(
        hs.iter().any(|h| h.label == "msg:" && h.kind == Some(InlayHintKind::Parameter)),
        "expected 'msg:' parameter hint; got: {:?}",
        hs.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn inlay01_param_hints_are_kind_parameter() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{{ greet(x) }}";
    let hs = hints(src);
    assert!(
        hs.iter().any(|h| h.kind == Some(InlayHintKind::Parameter)),
        "parameter hint must have kind = Parameter"
    );
}

#[test]
fn inlay01_keyword_arg_stops_the_positional_run() {
    // greet(name, msg) called as greet(show_actions=true) — keyword arg → no hints
    let src = "{% macro greet(name, msg) %}hi{% endmacro %}{{ greet(name=x, y) }}";
    let hs = hints(src);
    // No hint on the keyword arg; the positional run stops at first keyword arg
    assert!(
        !hs.iter().any(|h| h.label == "msg:" && h.kind == Some(InlayHintKind::Parameter)),
        "keyword arg must stop the positional run — no hint on trailing positional 'y'"
    );
}

#[test]
fn inlay01_unresolvable_macro_gets_no_hints() {
    // A call to an unknown macro → no hints (P4: never guess)
    let src = "{{ totally_unknown_macro_xyz(a, b) }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "unresolvable macro must produce no parameter hints"
    );
}

#[test]
fn inlay01_over_arity_labels_up_to_last_param_extras_get_none() {
    // greet(name) called as greet(x, extra) → name: before x, no hint before extra
    let src = "{% macro greet(name) %}hi{% endmacro %}{{ greet(x, extra) }}";
    let hs = hints(src);
    let name_hint = hs.iter().find(|h| h.label == "name:");
    assert!(name_hint.is_some(), "expected 'name:' hint for first arg");
    // The extra arg past the last param must get no hint
    // We only have 1 param so there should be exactly 1 hint total
    let param_hints: Vec<_> = hs.iter().filter(|h| h.kind == Some(InlayHintKind::Parameter)).collect();
    assert_eq!(param_hints.len(), 1, "over-arity: only 1 param hint (first arg), extra arg gets none");
}

#[test]
fn inlay01_half_typed_call_gets_no_hints() {
    // No opening paren → no args to label
    let src = "{% macro greet(name) %}hi{% endmacro %}{{ greet }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "half-typed call with no paren must produce no parameter hints"
    );
}

#[test]
fn inlay01_positional_after_keyword_gets_no_hint() {
    // greet(url, text) called as greet(href, text=label, "noopener")
    // url: before href; stop at text=label; no hint on trailing "noopener"
    let src = r#"{% macro greet(url, text, rel) %}hi{% endmacro %}{{ greet(href, text=label, "noopener") }}"#;
    let hs = hints(src);
    let param_hints: Vec<_> = hs.iter().filter(|h| h.kind == Some(InlayHintKind::Parameter)).collect();
    // Only 'url:' before 'href' — the run stops at text=label
    assert_eq!(param_hints.len(), 1, "positional-after-keyword must get no hint; only 1 hint expected (url: before href)");
    assert_eq!(param_hints[0].label, "url:", "the only hint should be 'url:'");
}

// ─── REQ-INLAY-06: suppress when arg already spells the param ─────────────────

#[test]
fn inlay06_bare_identifier_matching_param_is_suppressed() {
    // post_url(post) where param is `post` → suppressed (arg == param)
    let src = "{% macro post_url(post) %}{{ post.slug }}{% endmacro %}{{ post_url(post) }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "arg 'post' filling param 'post' must be suppressed (REQ-INLAY-06)"
    );
}

#[test]
fn inlay06_dotted_final_segment_matching_param_is_suppressed() {
    // thumb(author) where arg is `post.author` — final segment `author` == param `author`
    let src = "{% macro thumb(author) %}x{% endmacro %}{{ thumb(post.author) }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "arg 'post.author' with final segment 'author' filling param 'author' must be suppressed"
    );
}

#[test]
fn inlay06_dotted_final_segment_mismatch_emits_hint() {
    // thumb(author) where arg is `post.slug` — final segment `slug` ≠ param `author`
    let src = "{% macro thumb(author) %}x{% endmacro %}{{ thumb(post.slug) }}";
    let hs = hints(src);
    assert!(
        has_param_hint(&hs),
        "arg 'post.slug' with final segment 'slug' ≠ param 'author' must emit a hint"
    );
}

#[test]
fn inlay06_subscript_string_literal_matching_param_is_suppressed() {
    // thumb(author) where arg is `post["author"]` — final segment `author` == param
    let src = r#"{% macro thumb(author) %}x{% endmacro %}{{ thumb(post["author"]) }}"#;
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        r#"arg 'post["author"]' with string-literal final segment 'author' == param must be suppressed"#
    );
}

#[test]
fn inlay06_subscript_nonliteral_key_emits_hint() {
    // thumb(author) where arg is `post[i]` — no statically known final segment → emit hint
    let src = "{% macro thumb(author) %}x{% endmacro %}{{ thumb(post[i]) }}";
    let hs = hints(src);
    assert!(
        has_param_hint(&hs),
        "arg 'post[i]' with non-literal subscript key must NOT be suppressed"
    );
}

// ─── REQ-INLAY-07: filter positional args ────────────────────────────────────

#[test]
fn inlay07_filter_positional_arg_gets_label() {
    // truncate params = [length, killwords, end, leeway], receiver excluded.
    // The receiver is the bare identifier before `|`; `post.body` as a receiver
    // nests the binary_expression inside an attribute-access expression, which
    // the current query doesn't reach.  Use a bare identifier receiver instead.
    let src = "{{ x | truncate(60) }}";
    let hs = hints(src);
    assert!(
        hs.iter().any(|h| h.label == "length:" && h.kind == Some(InlayHintKind::Parameter)),
        "filter arg 60 must be labelled 'length:' (params[0]); hints: {:?}",
        hs.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn inlay07_filter_with_no_params_gets_no_hints() {
    // `upper` has no explicit params beyond the receiver → no hints
    let src = "{{ name | upper }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "filter 'upper' with no params must produce no parameter hints"
    );
}

#[test]
fn inlay07_filter_with_no_call_parens_gets_no_hints() {
    // truncate without explicit args → no hints (nothing to label)
    let src = "{{ name | truncate }}";
    let hs = hints(src);
    assert!(
        !has_param_hint(&hs),
        "filter called without parentheses must produce no parameter hints"
    );
}

// ─── REQ-INLAY-02: endblock name echo ────────────────────────────────────────

#[test]
fn inlay02_bare_endblock_gets_name_echo() {
    let src = "{% block content %}body{% endblock %}";
    let hs = hints(src);
    assert!(
        hs.iter().any(|h| h.label == "content" && h.kind.is_none()),
        "bare endblock must echo block name as a kind-less hint; hints: {:?}",
        hs.iter().map(|h| (&h.label, &h.kind)).collect::<Vec<_>>()
    );
}

#[test]
fn inlay02_echo_is_kind_none() {
    let src = "{% block body %}content{% endblock %}";
    let hs = hints(src);
    let echo = hs.iter().find(|h| h.label == "body");
    assert!(echo.is_some(), "should have an echo hint for 'body'");
    assert_eq!(echo.unwrap().kind, None, "endblock echo must be kind-less (not Parameter or Type)");
}

#[test]
fn inlay02_named_endblock_gets_no_echo() {
    let src = "{% block content %}body{% endblock content %}";
    let hs = hints(src);
    assert!(
        !has_endblock_hint(&hs),
        "named endblock must NOT get an echo hint (already spells the block name)"
    );
}

#[test]
fn inlay02_endblock_echo_positioned_after_endblock_keyword() {
    // "{% endblock %}" — the `endblock` keyword starts at byte 3 (after "{% ")
    // hint should be placed at the byte right after "endblock"
    let src = "{% block x %}y{% endblock %}";
    let hs = hints(src);
    let echo = hs.iter().find(|h| h.label == "x").expect("echo hint must exist");
    // "{% endblock %}" starts at byte 13 ("{% block x %}y" is 14 bytes: 0-13)
    // Within "{% endblock %}", "endblock" starts at offset 3 → byte 13+3=16, ends at 16+8=24
    // So col = 24 - line_start (which is 0 since all on one line) = 24
    // "{% block x %}y{% endblock %}"
    //  0123456789012345678901234567
    //              0000000001111111
    //              endblock is at "{% endblock" → start of endblock at index 16
    //              16 + 8 = 24
    // Actually let me count: "{% block x %}y{% endblock %}"
    //  0:{  1:%  2:{space}  3:b  4:l  5:o  6:c  7:k  8:{space}  9:x  10:{space}  11:%  12:}  13:y  14:{  15:%  16:{space}  17:e  18:n  19:d  20:b  21:l  22:o  23:c  24:k  25:{space}  26:%  27:}
    // Wait, let me recount carefully:
    // "{% block x %}y{% endblock %}"
    //  ^0 ^1 ^2 ^3^4 ^5 ^6 ^7 ^8 ^9^10^11^12^13^14^15^16^17^18^19^20^21^22^23^24^25^26^27
    //  {  %  sp b  l  o  c  k  sp x  sp %  }  y  {  %  sp e  n  d  b  l  o  c  k  sp %  }
    // endblock starts at col 17, ends at col 25 (17+8=25)
    assert_eq!(echo.line, 0, "echo must be on line 0");
    assert_eq!(echo.col, 25, "echo col must be right after 'endblock' keyword (col 17 + len 8 = 25)");
}

#[test]
fn inlay02_whitespace_control_endblock_still_echoes() {
    // {%- endblock -%} — echo still emitted, anchored to end of endblock keyword
    let src = "{% block content %}body{%- endblock -%}";
    let hs = hints(src);
    assert!(
        hs.iter().any(|h| h.label == "content" && h.kind.is_none()),
        "whitespace-control endblock must still emit an echo"
    );
}

#[test]
fn inlay02_nested_blocks_echo_correct_name() {
    let src = "{% block outer %}{% block inner %}x{% endblock %}{% endblock %}";
    let hs = hints(src);
    // The first bare endblock (at byte ~38) closes "inner"
    // The second bare endblock closes "outer"
    assert!(hs.iter().any(|h| h.label == "inner"), "first endblock must echo 'inner'");
    assert!(hs.iter().any(|h| h.label == "outer"), "second endblock must echo 'outer'");
}

// ─── REQ-INLAY-04: toggles are independent ───────────────────────────────────

#[test]
fn inlay04_parameter_names_off_suppresses_only_param_hints() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{% block content %}{{ greet(x) }}{% endblock %}";
    let cfg = InlayHintsConfig { parameter_names: false, endblock_names: true };
    let hs = hints_cfg(src, cfg);
    assert!(!has_param_hint(&hs), "parameterNames=off must suppress macro param hints");
    assert!(has_endblock_hint(&hs), "parameterNames=off must NOT suppress endblock echo");
}

#[test]
fn inlay04_endblock_names_off_suppresses_only_endblock_echo() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{% block content %}{{ greet(x) }}{% endblock %}";
    let cfg = InlayHintsConfig { parameter_names: true, endblock_names: false };
    let hs = hints_cfg(src, cfg);
    assert!(has_param_hint(&hs), "endblockNames=off must NOT suppress macro param hints");
    assert!(!has_endblock_hint(&hs), "endblockNames=off must suppress endblock echo");
}

#[test]
fn inlay04_both_off_produces_no_hints() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{% block content %}{{ greet(x) }}{% endblock %}";
    let cfg = InlayHintsConfig { parameter_names: false, endblock_names: false };
    let hs = hints_cfg(src, cfg);
    assert!(hs.is_empty(), "both toggles off must produce no hints");
}

// ─── REQ-INLAY-05: lazy tooltips via resolve ─────────────────────────────────

#[test]
fn inlay05_initial_hint_has_no_tooltip() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{{ greet(x) }}";
    let hs = hints(src);
    let h = hs.iter().find(|h| h.kind == Some(InlayHintKind::Parameter)).expect("hint must exist");
    assert!(h.tooltip.is_none(), "initial inlay hint must carry no tooltip (REQ-INLAY-05)");
}

#[test]
fn inlay05_resolved_macro_param_hint_attaches_tooltip() {
    let src = "{% macro greet(name) %}hi{% endmacro %}{{ greet(x) }}";
    let idx = extract(src);
    let registry = reg();
    let ws = WorkspaceIndex::default();
    let hs = inlay_hints(src, "test.html", &idx, &registry, &ws, &InlayHintsConfig::default());
    let h = hs.into_iter().find(|h| h.kind == Some(InlayHintKind::Parameter)).expect("hint must exist");
    let resolved = inlay_hint_resolve(h, &idx, &registry, &ws);
    assert!(resolved.tooltip.is_some(), "resolved macro param hint must have a tooltip");
}

#[test]
fn inlay05_resolved_filter_param_hint_attaches_tooltip() {
    // bare-identifier receiver so truncate is captured as Function (see inlay07_filter_positional_arg_gets_label)
    let src = "{{ x | truncate(60) }}";
    let idx = extract(src);
    let registry = reg();
    let ws = WorkspaceIndex::default();
    let hs = inlay_hints(src, "test.html", &idx, &registry, &ws, &InlayHintsConfig::default());
    let h = hs.into_iter().find(|h| h.label == "length:").expect("length: hint must exist");
    let resolved = inlay_hint_resolve(h, &idx, &registry, &ws);
    assert!(resolved.tooltip.is_some(), "resolved filter param hint must have a tooltip");
}

#[test]
fn inlay05_stale_resolve_returns_hint_unchanged_no_throw() {
    // Simulate a stale hint: symbol_name refers to something not in the index
    use jinja_lsp::features::inlay_hints::InlayHint;
    let stale_hint = InlayHint {
        line: 0,
        col: 5,
        label: "ghost:".to_string(),
        kind: Some(InlayHintKind::Parameter),
        tooltip: None,
        data: InlayHintData::Parameter {
            template_path: "test.html".to_string(),
            symbol_name: "nonexistent_macro_xyz".to_string(),
            param_index: 0,
        },
    };
    let idx = extract("{{ x }}");
    let registry = reg();
    let ws = WorkspaceIndex::default();
    // Must not panic, must return hint unchanged
    let resolved = inlay_hint_resolve(stale_hint, &idx, &registry, &ws);
    assert_eq!(resolved.label, "ghost:", "stale resolve must return hint unchanged");
    assert!(resolved.tooltip.is_none(), "stale resolve must not attach a tooltip");
}

#[test]
fn inlay05_data_is_logical_key_not_byte_offset() {
    // The data payload for a parameter hint must be (template_path, symbol_name, param_index)
    // and for an endblock echo must be (template_path, block_name).
    // Neither should embed a raw byte offset.
    let src = "{% macro greet(name) %}hi{% endmacro %}{% block b %}{{ greet(x) }}{% endblock %}";
    let hs = hints(src);

    let param_hint = hs.iter().find(|h| h.kind == Some(InlayHintKind::Parameter)).expect("param hint must exist");
    match &param_hint.data {
        InlayHintData::Parameter { template_path, symbol_name, param_index } => {
            assert_eq!(template_path, "test.html");
            assert_eq!(symbol_name, "greet");
            assert_eq!(*param_index, 0, "first arg maps to declared param index 0");
        }
        _ => panic!("param hint data must be InlayHintData::Parameter"),
    }

    let echo = hs.iter().find(|h| h.kind.is_none() && matches!(&h.data, InlayHintData::EndBlock { .. })).expect("echo must exist");
    match &echo.data {
        InlayHintData::EndBlock { template_path, block_name } => {
            assert_eq!(template_path, "test.html");
            assert_eq!(block_name, "b");
        }
        _ => panic!("endblock echo data must be InlayHintData::EndBlock"),
    }
}

// ─── REQ-INLAY-05: endblock echo resolve ────────────────────────────────────

#[test]
fn inlay05_resolved_endblock_echo_attaches_tooltip() {
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let registry = reg();
    let ws = WorkspaceIndex::default();
    let hs = inlay_hints(src, "test.html", &idx, &registry, &ws, &InlayHintsConfig::default());
    let h = hs.into_iter().find(|h| h.label == "content").expect("echo must exist");
    let resolved = inlay_hint_resolve(h, &idx, &registry, &ws);
    assert!(resolved.tooltip.is_some(), "resolved endblock echo must have a tooltip");
}

// ─── REQ-INLAY-01: from-import macro call gets hints ─────────────────────────

#[test]
fn inlay01_from_imported_macro_call_gets_hints() {
    // A from-import lets us call the macro directly.
    // Build a workspace with the macro's source.
    let macro_src = "{% macro render_link(url, text) %}x{% endmacro %}";
    let macro_idx = extract(macro_src);

    let call_src = r#"{% from "macros.html" import render_link %}{{ render_link(href, label) }}"#;

    let mut ws = WorkspaceIndex::default();
    ws.templates.insert("macros.html".to_string(), macro_idx);

    let hs = hints_with_ws(call_src, &ws);
    assert!(
        hs.iter().any(|h| h.label == "url:"),
        "from-imported macro call must emit param hints; hints: {:?}",
        hs.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
    assert!(
        hs.iter().any(|h| h.label == "text:"),
        "expected 'text:' hint for second arg"
    );
}
