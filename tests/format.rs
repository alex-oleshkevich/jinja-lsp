// F18 — Formatter engine tests: REQ-FMT-01/03/04 delimiter spacing, marker spacing, pipe spacing.

use jinja_lsp::format::{format, normalize_delimiter};

// ─── REQ-FMT-01: T-01 — Expression delimiter spacing ─────────────────────────

#[test]
fn fmt01_t01_expression_tight_delimiters() {
    assert_eq!(format("{{x}}"), "{{ x }}");
}

#[test]
fn fmt01_t02_statement_tight_delimiters() {
    assert_eq!(format("{%if x%}\n{%endif%}"), "{% if x %}\n{% endif %}");
}

#[test]
fn fmt01_t03_comment_tight_delimiters() {
    assert_eq!(format("{#note#}"), "{# note #}");
}

#[test]
fn fmt01_t04_collapse_multi_space_padding() {
    assert_eq!(format("{{  x  }}"), "{{ x }}");
}

#[test]
fn fmt01_t05_interior_expression_spacing_untouched() {
    // Delimiter padding normalized; interior operator spacing not owned by this pass.
    assert_eq!(format("{{ a+b }}"), "{{ a+b }}");
}

// ─── REQ-FMT-01: Additional — already-normalized is a no-op ─────────────────

#[test]
fn fmt01_already_normalized_noop() {
    let src = "{{ x }}\n{% if y %}\nhello\n{% endif %}\n{# comment #}";
    assert_eq!(format(src), src);
}

// ─── REQ-FMT-01: Additional — host-language bytes untouched ─────────────────

#[test]
fn fmt01_host_bytes_untouched() {
    let src = "<p>{{name}}</p>";
    assert_eq!(format(src), "<p>{{ name }}</p>");
}

// ─── REQ-FMT-01: Additional — multiple delimiters in one file ────────────────

#[test]
fn fmt01_multiple_delimiters() {
    let src = "{{a}} {{b}} {%if c%}x{%endif%}";
    assert_eq!(format(src), "{{ a }} {{ b }} {% if c %}x{% endif %}");
}

// ─── REQ-FMT-01: Additional — whitespace-control markers preserved ────────────

#[test]
fn fmt01_whitespace_control_markers_preserved() {
    // The marker itself is preserved; spacing around content normalized.
    assert_eq!(normalize_delimiter("{%-if x-%}"), "{%- if x -%}");
    assert_eq!(normalize_delimiter("{{-name-}}"), "{{- name -}}");
    assert_eq!(normalize_delimiter("{#-note-#}"), "{#- note -#}");
}

// ─── REQ-FMT-01: Additional — one-sided markers ──────────────────────────────

#[test]
fn fmt01_one_sided_marker() {
    assert_eq!(normalize_delimiter("{%- if x %}"), "{%- if x %}");
    assert_eq!(normalize_delimiter("{% if x -%}"), "{% if x -%}");
}

// ─── REQ-FMT-01: Additional — syntax error → no change ──────────────────────

#[test]
fn fmt01_syntax_error_passthrough() {
    // An unclosed delimiter is a syntax error; the formatter must not corrupt it.
    let bad = "{{ unclosed";
    assert_eq!(format(bad), bad);
}

// ─── REQ-FMT-03: T-11 — Statement marker spacing ─────────────────────────────

#[test]
fn fmt03_t11_statement_marker_spacing() {
    // {%-if x-%} → {%- if x -%}
    assert_eq!(format("{%-if x-%}\n{%-endif-%}"), "{%- if x -%}\n{%- endif -%}");
}

#[test]
fn fmt03_t13_one_sided_marker_preserved() {
    // One-sided marker preserved; spacing normalized
    assert_eq!(format("{%- if x %}body{% endif %}"), "{%- if x %}body{% endif %}");
    assert_eq!(format("{% if x -%}body{%- endif %}"), "{% if x -%}body{%- endif %}");
}

#[test]
fn fmt03_t14_no_marker_added() {
    // Markers are never invented — a markerless tag stays markerless
    let src = "{% if x %}body{% endif %}";
    assert_eq!(format(src), src);
}

// ─── REQ-FMT-01: Idempotence ─────────────────────────────────────────────────

#[test]
fn fmt01_idempotent() {
    let inputs = [
        "{{x}}",
        "{%if x%}\n{%endif%}",
        "{#note#}",
        "<p>{{name}}</p>",
        "{{ a+b }}",
        "{%- if x -%}content{%- endif -%}",
    ];
    for src in inputs {
        let once = format(src);
        let twice = format(&once);
        assert_eq!(once, twice, "format must be idempotent for: {src:?}");
    }
}

// ─── REQ-FMT-04: T-15 — Single pipe spacing ──────────────────────────────────

#[test]
fn fmt04_t15_single_pipe_spacing() {
    assert_eq!(format("{{ x|e }}"), "{{ x | e }}");
}

#[test]
fn fmt04_t16_chained_pipes() {
    assert_eq!(format("{{ name|upper|trim }}"), "{{ name | upper | trim }}");
}

#[test]
fn fmt04_t17_is_test_spacing() {
    assert_eq!(format("{{ post is  defined }}"), "{{ post is defined }}");
}

#[test]
fn fmt04_t18_filter_call_arg_commas() {
    assert_eq!(format("{{ x | truncate( 20,true ) }}"), "{{ x | truncate(20, true) }}");
}

#[test]
fn fmt04_t19_non_pipe_operators_untouched() {
    // == is not a pipe or is-test — left as-is (formatter, not beautifier)
    let src = "{{ a==b }}";
    assert_eq!(format(src), src);
}

// ─── REQ-FMT-04: T-44 — Non-filter commas untouched ─────────────────────────

#[test]
fn fmt04_t44_dict_literal_commas_untouched() {
    let src = "{{ {'a': 1,'b': 2} }}";
    assert_eq!(format(src), src);
}

#[test]
fn fmt04_t44_non_filter_call_commas_untouched() {
    // post_url is a plain function call (not via |) — its commas are untouched
    let src = "{{ post_url(post,absolute=true) }}";
    assert_eq!(format(src), src);
}

#[test]
fn fmt04_combined_pipe_and_delimiter() {
    // Both FMT-01 (tight delimiter) and FMT-04 (pipe spacing) applied together
    assert_eq!(format("{{name|upper}}"), "{{ name | upper }}");
}

#[test]
fn fmt04_marker_plus_pipe() {
    // Marker spacing (FMT-03) + pipe spacing (FMT-04)
    assert_eq!(format("{{- name|trim -}}"), "{{- name | trim -}}");
}

// ─── REQ-FMT-04: Idempotence extension ───────────────────────────────────────

#[test]
fn fmt04_idempotent() {
    let inputs = [
        "{{ x|e }}",
        "{{ name|upper|trim }}",
        "{{ post is  defined }}",
        "{{ x | truncate( 20,true ) }}",
        "{{ {'a': 1,'b': 2} }}",
        "{{ post_url(post,absolute=true) }}",
        "{{name|upper}}",
    ];
    for src in inputs {
        let once = format(src);
        let twice = format(&once);
        assert_eq!(once, twice, "format must be idempotent for: {src:?}");
    }
}

#[test]
#[ignore]
fn dump_filter_tree() {
    let lang = tree_sitter_jinja::language();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let cases = [
        "{{ name|upper }}",
        "{{ name | upper | trim }}",
        "{{ name|upper(20, true) }}",
        "{{ post is defined }}",
        "{{ post is  defined }}",
        "{{ {'a': 1,'b': 2} }}",
        "{{ post_url(post,absolute=true) }}",
    ];
    for src in cases {
        let tree = parser.parse(src, None).unwrap();
        eprintln!("{src}\n→ {}\n", tree.root_node().to_sexp());
    }
}
