// F13 — Semantic token tests: REQ-SEM-01 through REQ-SEM-06.

use jinja_lsp::builtins::registry::Registry;
use jinja_lsp::features::semantic_tokens::{
    semantic_tokens_full, semantic_tokens_range, SemanticToken, TOKEN_MODIFIERS, TOKEN_TYPES,
    MOD_BUILTIN, MOD_DEFINED, MOD_UNKNOWN, MOD_USER, TT_BLOCK, TT_FILTER, TT_FUNCTION, TT_MACRO,
    TT_PARAMETER, TT_TEST, TT_VARIABLE,
};
use jinja_lsp::parsing::extract;
use jinja_lsp::workspace::index::WorkspaceIndex;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn tokens_of_type(tokens: &[SemanticToken], ty: u32) -> Vec<&SemanticToken> {
    tokens.iter().filter(|t| t.token_type == ty).collect()
}

fn full(src: &str) -> Vec<SemanticToken> {
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    semantic_tokens_full(src, &idx, &reg, &ws)
}

// ─── REQ-SEM-01: token-type legend order ─────────────────────────────────────

#[test]
fn sem01_token_type_legend_order() {
    assert_eq!(TOKEN_TYPES[0], "macro");
    assert_eq!(TOKEN_TYPES[1], "variable");
    assert_eq!(TOKEN_TYPES[2], "parameter");
    assert_eq!(TOKEN_TYPES[3], "filter");
    assert_eq!(TOKEN_TYPES[4], "function");
    assert_eq!(TOKEN_TYPES[5], "test");
    assert_eq!(TOKEN_TYPES[6], "block");
    assert_eq!(TOKEN_TYPES.len(), 7, "exactly 7 types; index 7 tombstoned (REQ-SEM-06)");
}

#[test]
fn sem01_no_keyword_type_declared() {
    assert!(
        !TOKEN_TYPES.contains(&"keyword"),
        "keyword type must not be declared (retired, tombstoned at index 7)"
    );
}

// ─── REQ-SEM-02: token-modifier legend order ─────────────────────────────────

#[test]
fn sem02_token_modifier_legend_order() {
    assert_eq!(TOKEN_MODIFIERS[0], "defined"); // bit 0
    assert_eq!(TOKEN_MODIFIERS[1], "unknown"); // bit 1
    assert_eq!(TOKEN_MODIFIERS[2], "builtin"); // bit 2
    assert_eq!(TOKEN_MODIFIERS[3], "user");    // bit 3
    assert_eq!(TOKEN_MODIFIERS.len(), 4);
}

// ─── REQ-SEM-06: append-only, tombstoned index 7 ────────────────────────────

#[test]
fn sem06_type_indices_are_stable() {
    assert_eq!(TT_MACRO, 0);
    assert_eq!(TT_VARIABLE, 1);
    assert_eq!(TT_PARAMETER, 2);
    assert_eq!(TT_FILTER, 3);
    assert_eq!(TT_FUNCTION, 4);
    assert_eq!(TT_TEST, 5);
    assert_eq!(TT_BLOCK, 6);
    // Index 7 is tombstoned — no constant defined there.
    assert_eq!(TOKEN_TYPES.len(), 7, "legend has exactly 7 live types; index 7 is the tombstone");
}

// ─── REQ-SEM-04: block definition → block token, zero modifiers ─────────────

#[test]
fn sem04_block_definition_emits_block_token() {
    let tokens = full("{% block content %}body{% endblock %}");
    let block_toks = tokens_of_type(&tokens, TT_BLOCK);
    assert!(!block_toks.is_empty(), "block definition must emit a block token");
}

#[test]
fn sem04_block_token_carries_no_modifiers() {
    let tokens = full("{% block content %}body{% endblock %}");
    for tok in tokens_of_type(&tokens, TT_BLOCK) {
        assert_eq!(tok.token_modifiers, 0, "block token must carry zero modifiers (REQ-SEM-02)");
    }
}

// ─── REQ-SEM-04: macro definition → macro {defined, user} ───────────────────

#[test]
fn sem04_macro_definition_emits_macro_token() {
    let tokens = full("{% macro greet(name) %}hello{% endmacro %}");
    assert!(
        tokens.iter().any(|t| t.token_type == TT_MACRO),
        "macro definition must emit a macro token"
    );
}

#[test]
fn sem04_macro_definition_has_defined_user_modifiers() {
    let tokens = full("{% macro greet(name) %}hello{% endmacro %}");
    assert!(
        tokens.iter().any(|t| t.token_type == TT_MACRO && t.token_modifiers == MOD_DEFINED | MOD_USER),
        "macro definition must have {{defined, user}} modifiers"
    );
}

// ─── REQ-SEM-04: macro call → macro {defined, user} ─────────────────────────

#[test]
fn sem04_macro_call_is_macro_defined_user() {
    let src = "{% macro greet() %}x{% endmacro %}{{ greet() }}";
    let tokens = full(src);
    let call_toks: Vec<_> = tokens
        .iter()
        .filter(|t| t.token_type == TT_MACRO && t.token_modifiers == MOD_DEFINED | MOD_USER)
        .collect();
    assert!(call_toks.len() >= 1, "macro call must be tokenized as macro {{defined, user}}");
}

// ─── REQ-SEM-04: parameter in signature → parameter, zero modifiers ──────────

#[test]
fn sem04_parameter_in_signature_emits_parameter_token() {
    let tokens = full("{% macro greet(name) %}hello{% endmacro %}");
    let param_toks = tokens_of_type(&tokens, TT_PARAMETER);
    assert!(!param_toks.is_empty(), "macro parameter must emit a parameter token");
}

#[test]
fn sem04_parameter_token_carries_no_modifiers() {
    let tokens = full("{% macro greet(name) %}hello{% endmacro %}");
    for tok in tokens_of_type(&tokens, TT_PARAMETER) {
        assert_eq!(tok.token_modifiers, 0, "parameter token must carry zero modifiers (REQ-SEM-02)");
    }
}

// ─── REQ-SEM-04: parameter body use → variable, not parameter ───────────────

#[test]
fn sem04_parameter_body_use_is_variable_not_parameter() {
    // "name" appears in signature (→ parameter) and in body (→ variable).
    let src = "{% macro greet(name) %}{{ name }}{% endmacro %}";
    let tokens = full(src);
    let param_count = tokens.iter().filter(|t| t.token_type == TT_PARAMETER).count();
    let var_count = tokens
        .iter()
        .filter(|t| t.token_type == TT_VARIABLE && t.length == "name".len() as u32)
        .count();
    assert!(param_count >= 1, "signature 'name' must be a parameter token");
    assert!(var_count >= 1, "body 'name' must be a variable token");
}

// ─── REQ-SEM-04: filter builtin → filter {builtin, defined} ─────────────────

#[test]
fn sem04_filter_builtin_is_filter_builtin_defined() {
    let tokens = full("{{ x | truncate }}");
    let filter_tok = tokens.iter().find(|t| t.token_type == TT_FILTER);
    assert!(filter_tok.is_some(), "known filter must emit a filter token");
    assert_eq!(
        filter_tok.unwrap().token_modifiers,
        MOD_BUILTIN | MOD_DEFINED,
        "builtin filter → {{builtin, defined}}"
    );
}

// ─── REQ-SEM-04: filter unknown → filter {unknown} ───────────────────────────

#[test]
fn sem04_filter_unknown_is_filter_unknown() {
    let tokens = full("{{ x | absolutely_unknown_filter_xyz }}");
    let filter_tok = tokens.iter().find(|t| t.token_type == TT_FILTER);
    assert!(filter_tok.is_some(), "unknown filter must still emit a filter token");
    assert_eq!(
        filter_tok.unwrap().token_modifiers,
        MOD_UNKNOWN,
        "unknown filter → {{unknown}}"
    );
}

// ─── REQ-SEM-04: test builtin → test {builtin, defined} ─────────────────────

#[test]
fn sem04_test_builtin_is_test_builtin_defined() {
    let tokens = full("{% if x is defined %}yes{% endif %}");
    let test_tok = tokens.iter().find(|t| t.token_type == TT_TEST);
    assert!(test_tok.is_some(), "builtin test must emit a test token");
    assert_eq!(
        test_tok.unwrap().token_modifiers,
        MOD_BUILTIN | MOD_DEFINED,
        "builtin test → {{builtin, defined}}"
    );
}

// ─── REQ-SEM-04: test unknown → test {unknown} ───────────────────────────────

#[test]
fn sem04_test_unknown_is_test_unknown() {
    let tokens = full("{% if x is xyz_totally_unknown_test %}yes{% endif %}");
    let test_tok = tokens.iter().find(|t| t.token_type == TT_TEST);
    assert!(test_tok.is_some(), "unknown test must still emit a test token");
    assert_eq!(
        test_tok.unwrap().token_modifiers,
        MOD_UNKNOWN,
        "unknown test → {{unknown}}"
    );
}

// ─── REQ-SEM-04: unresolved call → variable {unknown}, NOT function {unknown} ─

#[test]
fn sem04_unresolved_call_is_variable_unknown_not_function_unknown() {
    // §5.3.1 step 3: no macro, no registry function → variable {unknown}
    let tokens = full("{{ totally_unknown_func_xyz() }}");
    assert!(
        !tokens.iter().any(|t| t.token_type == TT_FUNCTION && t.token_modifiers == MOD_UNKNOWN),
        "unresolved call must NOT be function {{unknown}} per §5.3.1"
    );
    assert!(
        tokens.iter().any(|t| t.token_type == TT_VARIABLE && t.token_modifiers == MOD_UNKNOWN),
        "unresolved call must be variable {{unknown}}"
    );
}

// ─── REQ-SEM-03: full and range ──────────────────────────────────────────────

#[test]
fn sem03_range_decoded_tuples_are_subset_of_full() {
    // Block on line 0, filter on line 1, endblock on line 2.
    let src = "{% block content %}\n{{ x | truncate }}\n{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let full_tokens = semantic_tokens_full(src, &idx, &reg, &ws);
    // Range: just line 1 (the filter line).
    let range_tokens = semantic_tokens_range(src, 1, 1, &idx, &reg, &ws);
    for rt in &range_tokens {
        assert!(
            full_tokens.iter().any(|ft| {
                ft.line == rt.line
                    && ft.start_char == rt.start_char
                    && ft.token_type == rt.token_type
                    && ft.token_modifiers == rt.token_modifiers
            }),
            "range token (line={}, char={}, type={}, mods={}) must be in full set",
            rt.line,
            rt.start_char,
            rt.token_type,
            rt.token_modifiers
        );
    }
}

#[test]
fn sem03_range_excludes_out_of_range_tokens() {
    let src = "{% block content %}\n{{ x | truncate }}\n{% endblock %}";
    let idx = extract(src);
    let reg = Registry::load_core();
    let ws = WorkspaceIndex::default();
    let range_tokens = semantic_tokens_range(src, 0, 0, &idx, &reg, &ws);
    assert!(
        range_tokens.iter().all(|t| t.line == 0),
        "range(0,0) must only include line 0 tokens"
    );
}

// ─── REQ-SEM-05: full/delta is deferred ──────────────────────────────────────

#[test]
fn sem05_legend_has_exactly_7_types_full_delta_deferred() {
    // The legend has 7 types — nothing extra for delta.
    assert_eq!(TOKEN_TYPES.len(), 7);
}

// ─── REQ-SEM-04: statement keywords emit no semantic token ───────────────────

#[test]
fn sem04_for_keyword_emits_no_token() {
    // "for" at col 3 inside "{% for item in items %}" must not be tokenized.
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let tokens = full(src);
    // "for" keyword is at byte 3 (col 3). Check no token at that position.
    let has_for_tok = tokens.iter().any(|t| t.line == 0 && t.start_char == 3 && t.length == 3);
    assert!(!has_for_tok, "statement keyword 'for' must not emit a semantic token");
}

// ─── REQ-SEM-04: variable from for-loop → variable {user, defined} ───────────

#[test]
fn sem04_for_loop_variable_is_variable_user_defined() {
    let src = "{% for item in items %}{{ item }}{% endfor %}";
    let tokens = full(src);
    // "item" usage in {{ item }} should be variable {user, defined}
    let item_tok = tokens.iter().find(|t| {
        t.token_type == TT_VARIABLE
            && t.token_modifiers == MOD_USER | MOD_DEFINED
            && t.length == "item".len() as u32
    });
    assert!(item_tok.is_some(), "for-loop variable 'item' usage must be variable {{user, defined}}");
}

// ─── REQ-SEM-04: tokens are sorted by position ───────────────────────────────

#[test]
fn sem04_tokens_sorted_by_line_then_char() {
    let src = "{% block a %}\n{% block b %}x{% endblock %}\n{% endblock %}";
    let tokens = full(src);
    for window in tokens.windows(2) {
        let a = &window[0];
        let b = &window[1];
        assert!(
            (a.line, a.start_char) <= (b.line, b.start_char),
            "tokens must be sorted: ({},{}) should precede ({},{})",
            a.line,
            a.start_char,
            b.line,
            b.start_char
        );
    }
}
