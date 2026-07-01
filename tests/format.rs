// F18 — Formatter engine tests: REQ-FMT-01/03/04 delimiter spacing, marker spacing, pipe spacing.

use jinja_lsp::format::{format, format_with_config, normalize_delimiter, FormatterConfig};

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

// ─── REQ-FMT-04: T-15 — Pipe spacing (default: compact) ─────────────────────

#[test]
fn fmt04_t15_compact_pipe_default() {
    // Default: space_around_pipe=false → compact notation `x|filter`.
    assert_eq!(format("{{ x|e }}"), "{{ x|e }}");
    assert_eq!(format("{{ x | e }}"), "{{ x|e }}");
}

#[test]
fn fmt04_t15_spaced_pipe_with_config() {
    // Opt-in: space_around_pipe=true → spaced notation `x | filter`.
    let cfg = FormatterConfig { space_around_pipe: true, newline_at_eof: false, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    assert_eq!(format_with_config("{{ x|e }}", &cfg), "{{ x | e }}");
}

#[test]
fn fmt04_t16_chained_pipes() {
    assert_eq!(format("{{ name|upper|trim }}"), "{{ name|upper|trim }}");
    assert_eq!(format("{{ name | upper | trim }}"), "{{ name|upper|trim }}");
}

#[test]
fn fmt04_t17_is_test_spacing() {
    // `is` is a keyword operator — always normalized to single space on each side.
    assert_eq!(format("{{ post is  defined }}"), "{{ post is defined }}");
}

#[test]
fn fmt04_t18_filter_call_arg_commas() {
    // space_after_comma=true (default): filter call args get a space after comma.
    let cfg = FormatterConfig { space_around_pipe: true, newline_at_eof: false, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    assert_eq!(format_with_config("{{ x | truncate( 20,true ) }}", &cfg), "{{ x | truncate(20, true) }}");
}

#[test]
fn fmt04_t18_compact_filter_call_arg_commas() {
    // With default (compact pipes): filter args still get spaces, pipe stays compact.
    assert_eq!(format("{{ x | truncate( 20,true ) }}"), "{{ x|truncate(20, true) }}");
}

#[test]
fn fmt04_space_after_comma_false() {
    // space_after_comma=false: args joined without space.
    let cfg = FormatterConfig { space_after_comma: false, space_around_pipe: true, newline_at_eof: false, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    assert_eq!(format_with_config("{{ x | truncate( 20,true ) }}", &cfg), "{{ x | truncate(20,true) }}");
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
    // FMT-01 (tight delimiter) + FMT-04 compact pipe (default).
    assert_eq!(format("{{name|upper}}"), "{{ name|upper }}");
}

#[test]
fn fmt04_marker_plus_pipe() {
    // Marker spacing (FMT-03) + compact pipe (default).
    assert_eq!(format("{{- name|trim -}}"), "{{- name|trim -}}");
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

// ─── REQ-FMT-02: Block-body re-indentation ───────────────────────────────────

#[test]
fn fmt02_t01_block_body_indented() {
    // A single block: body Jinja-tag lines get +4 spaces (default indent_size=4).
    let src = "{% block content %}\n{% if x %}\nhello\n{% endif %}\n{% endblock %}";
    let want = "{% block content %}\n    {% if x %}\nhello\n    {% endif %}\n{% endblock %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t02_nested_blocks_compound() {
    // Nested blocks: depth 1 = 4 spaces, depth 2 = 8 spaces.
    let src = "{% block outer %}\n{% block inner %}\nhello\n{% endblock %}\n{% endblock %}";
    let want = "{% block outer %}\n    {% block inner %}\nhello\n    {% endblock %}\n{% endblock %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t03_for_loop_body_indented() {
    let src = "{% for item in list %}\n{% if item %}\nx\n{% endif %}\n{% endfor %}";
    let want = "{% for item in list %}\n    {% if item %}\nx\n    {% endif %}\n{% endfor %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t04_already_indented_is_noop() {
    // Already formatted with 4 spaces — must not double-indent.
    let src = "{% block content %}\n    {% if x %}\nhello\n    {% endif %}\n{% endblock %}";
    assert_eq!(format(src), src);
}

#[test]
fn fmt02_t05_host_lines_untouched() {
    // Host-language lines (non-Jinja-tag lines) keep their own indentation.
    let src = "{% block content %}\n  <p>hello</p>\n{% endblock %}";
    assert_eq!(format(src), src);
}

#[test]
fn fmt02_t06_macro_body_indented() {
    let src = "{% macro btn(label) %}\n{% if label %}\n<button>{{ label }}</button>\n{% endif %}\n{% endmacro %}";
    let want = "{% macro btn(label) %}\n    {% if label %}\n<button>{{ label }}</button>\n    {% endif %}\n{% endmacro %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t07_inline_set_not_opener() {
    // {% set x = value %} is an inline statement — must NOT increase depth.
    let src = "{% block content %}\n{% set x = 1 %}\n{% if x %}\nhello\n{% endif %}\n{% endblock %}";
    let want = "{% block content %}\n    {% set x = 1 %}\n    {% if x %}\nhello\n    {% endif %}\n{% endblock %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t08_elif_else_realign() {
    // elif/else re-align with the opener (depth stays consistent for blocks).
    let src = "{% block content %}\n{% if x %}\nhello\n{% elif y %}\nworld\n{% else %}\nfoo\n{% endif %}\n{% endblock %}";
    let want = "{% block content %}\n    {% if x %}\nhello\n    {% elif y %}\nworld\n    {% else %}\nfoo\n    {% endif %}\n{% endblock %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t09_single_line_block_no_depth_leak() {
    // A single-line paired block (opener and closer on the same line) must leave
    // depth unchanged so subsequent Jinja-tag lines are not over-indented.
    let src = "{% block t %}x{% endblock %}\n{% if y %}z{% endif %}";
    let want = "{% block t %}x{% endblock %}\n{% if y %}z{% endif %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t10_single_line_if_endif_no_depth_leak() {
    // Same invariant as t09 for if/endif.
    let src = "{% if x %}z{% endif %}\nafter\n{% if b %}c{% endif %}";
    let want = "{% if x %}z{% endif %}\nafter\n{% if b %}c{% endif %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t11_single_line_in_outer_block() {
    // A single-line inner block inside a multi-line outer block must still
    // produce depth=1 indentation for subsequent siblings.
    let src = "{% block outer %}\n{% block inner %}x{% endblock %}\n{% set y = 1 %}\n{% endblock %}";
    let want = "{% block outer %}\n    {% block inner %}x{% endblock %}\n    {% set y = 1 %}\n{% endblock %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t12_autoescape_indents_inner_tags() {
    // {% autoescape %} is an opener — inner {% if %} gets +1 depth.
    let src = "{% autoescape true %}\n{% if x %}\nok\n{% endif %}\n{% endautoescape %}";
    let want = "{% autoescape true %}\n    {% if x %}\nok\n    {% endif %}\n{% endautoescape %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t13_trans_indents_inner_tags() {
    // {% trans %} is an opener — inner {% if %} gets +1 depth.
    let src = "{% trans %}\n{% if x %}a{% endif %}\n{% endtrans %}";
    let want = "{% trans %}\n    {% if x %}a{% endif %}\n{% endtrans %}";
    assert_eq!(format(src), want);
}

#[test]
fn fmt02_t14_block_set_indents_via_reindent() {
    // Block set (no =) is an opener — inner block gets +1 depth.
    // Called via reindent directly because the grammar treats block-set as an error.
    let src = "{% set nav %}\n{% block nav %}x{% endblock %}\n{% endset %}";
    let want = "{% set nav %}\n  {% block nav %}x{% endblock %}\n{% endset %}";
    assert_eq!(jinja_lsp::format::reindent(src, "  "), want);
}

#[test]
fn fmt02_idempotent() {
    let inputs = [
        "{% block content %}\n{% if x %}\nhello\n{% endif %}\n{% endblock %}",
        "{% for item in list %}\n{% if item %}\nx\n{% endif %}\n{% endfor %}",
        "{% block content %}\n  <p>hello</p>\n{% endblock %}",
        // Single-line paired tags must be stable under repeated formatting.
        "{% block t %}x{% endblock %}\n{% if y %}z{% endif %}",
        "{% block outer %}\n{% block inner %}x{% endblock %}\n{% set y = 1 %}\n{% endblock %}",
    ];
    for src in inputs {
        let once = format(src);
        let twice = format(&once);
        assert_eq!(once, twice, "fmt02 must be idempotent for: {src:?}");
    }
}

// ─── REQ-FMT-05: T-20 — Host-language bytes emitted byte-for-byte ─────────────

#[test]
fn fmt05_t20_html_bytes_untouched() {
    let src = "<p class=\"lead\">{{name}}</p>\n<a href=\"url\">link</a>";
    assert_eq!(format(src), "<p class=\"lead\">{{ name }}</p>\n<a href=\"url\">link</a>");
}

#[test]
fn fmt05_t21_attribute_values_untouched() {
    // Spaces inside host attribute values never trimmed
    let src = "<a href=\"  x  \">{{ text }}</a>";
    assert_eq!(format(src), src);
}

#[test]
fn fmt05_t22_blank_line_preserved() {
    // Blank host lines inside a block are reproduced exactly
    let src = "{% if x %}\n\n<p>text</p>\n\n{% endif %}";
    assert_eq!(format(src), src);
}

// ─── REQ-FMT-06: T-24/T-25 — Idempotence and syntax-error passthrough ────────

#[test]
fn fmt06_t24_idempotent_across_all_passes() {
    let inputs = [
        "{{x}}\n{%if y%}\nhello\n{%endif%}",
        "{{ name|upper|trim }}",
        "{{ x | truncate( 20,true ) }}",
        "<p>{{title}}</p>",
        "{{ post is  defined }}",
        "{%- if x -%}body{%- endif -%}",
        "{{ {'a': 1,'b': 2} }}",
    ];
    for src in inputs {
        let once = format(src);
        let twice = format(&once);
        assert_eq!(once, twice, "not idempotent: {src:?}");
    }
}

#[test]
fn fmt06_t27_syntax_error_passthrough() {
    // A file with a syntax error is returned byte-for-byte
    let bad = "{{ unclosed\n{% broken";
    assert_eq!(format(bad), bad);
}

// ─── FormatterConfig — newline_at_eof and trim_trailing_whitespace ────────────

#[test]
fn fmt_config_newline_at_eof_adds_newline() {
    let cfg = FormatterConfig { newline_at_eof: true, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    let result = format_with_config("{{ x }}", &cfg);
    assert!(result.ends_with('\n'), "newline_at_eof=true must append newline");
}

#[test]
fn fmt_config_newline_at_eof_idempotent() {
    let cfg = FormatterConfig { newline_at_eof: true, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    let once = format_with_config("{{ x }}", &cfg);
    let twice = format_with_config(&once, &cfg);
    assert_eq!(once, twice, "newline_at_eof must be idempotent");
}

#[test]
fn fmt_config_trim_trailing_whitespace() {
    let cfg = FormatterConfig { trim_trailing_whitespace: true, newline_at_eof: false, ..FormatterConfig::default() };
    let src = "{% if x %}  \nhello   \n{% endif %}";
    let result = format_with_config(src, &cfg);
    for line in result.lines() {
        assert!(!line.ends_with(' '), "trailing whitespace must be stripped: {line:?}");
    }
}

#[test]
fn fmt_config_indent_size_2() {
    let cfg = FormatterConfig { indent_size: 2, newline_at_eof: false, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    let src = "{% block content %}\n{% if x %}\nhello\n{% endif %}\n{% endblock %}";
    let want = "{% block content %}\n  {% if x %}\nhello\n  {% endif %}\n{% endblock %}";
    assert_eq!(format_with_config(src, &cfg), want);
}

#[test]
fn fmt_config_use_tabs() {
    let cfg = FormatterConfig { use_tabs: true, newline_at_eof: false, trim_trailing_whitespace: false, ..FormatterConfig::default() };
    let src = "{% block content %}\n{% if x %}\nhello\n{% endif %}\n{% endblock %}";
    let result = format_with_config(src, &cfg);
    assert!(result.contains('\t'), "use_tabs=true must use tabs for indentation");
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
