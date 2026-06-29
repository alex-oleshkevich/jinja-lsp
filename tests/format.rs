// F18 — Formatter engine tests: REQ-FMT-01 delimiter spacing.

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
