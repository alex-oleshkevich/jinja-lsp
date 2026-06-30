// F12 — Folding range tests: REQ-FOLD2-01 through REQ-FOLD2-06.

use jinja_lsp::features::folding::{fold_ranges, FoldKind};

// ─── REQ-FOLD2-01: every block-statement node folds ──────────────────────────

#[test]
fn fold01_block_folds_as_region() {
    let src = "{% block content %}\nbody\n{% endblock %}";
    let ranges = fold_ranges(src);
    let r = ranges.iter().find(|r| r.kind == FoldKind::Region).expect("block must fold");
    assert_eq!(r.start_line, 0, "startLine is opener line (0-based)");
    assert_eq!(r.end_line, 2, "endLine is closer line (0-based)");
}

#[test]
fn fold01_for_folds_as_region() {
    let src = "{% for item in items %}\n{{ item }}\n{% endfor %}";
    let ranges = fold_ranges(src);
    assert!(
        ranges.iter().any(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2),
        "for loop must fold"
    );
}

#[test]
fn fold01_macro_folds_as_region() {
    let src = "{% macro greet(name) %}\nhello {{ name }}\n{% endmacro %}";
    let ranges = fold_ranges(src);
    assert!(
        ranges.iter().any(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2),
        "macro must fold"
    );
}

#[test]
fn fold01_if_folds_as_single_region_from_if_to_endif() {
    // elif/else are intermediate clauses — NOT separate folds (§2, §5.1).
    let src = "{% if x %}\na\n{% elif y %}\nb\n{% else %}\nc\n{% endif %}";
    let ranges = fold_ranges(src);
    let r = ranges
        .iter()
        .find(|r| r.kind == FoldKind::Region && r.start_line == 0)
        .expect("if block must fold");
    assert_eq!(r.end_line, 6, "region ends at endif (line 6)");
    // No per-branch sub-folds.
    assert!(
        !ranges.iter().any(|r| r.start_line == 2 || r.start_line == 4),
        "elif/else must not produce separate sub-folds"
    );
}

#[test]
fn fold01_custom_tag_folds_via_endname_convention() {
    let src = "{% cache 'key' %}\ncontent\n{% endcache %}";
    let ranges = fold_ranges(src);
    assert!(
        ranges.iter().any(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2),
        "custom cache tag must fold via end<name> convention"
    );
}

#[test]
fn fold01_nested_blocks_fold_independently() {
    // Outer for: line 0..4; inner if: line 1..3
    let src = "{% for x in xs %}\n{% if x %}\n{{ x }}\n{% endif %}\n{% endfor %}";
    let ranges = fold_ranges(src);
    let outer = ranges.iter().find(|r| r.start_line == 0 && r.end_line == 4);
    let inner = ranges.iter().find(|r| r.start_line == 1 && r.end_line == 3);
    assert!(outer.is_some(), "outer for must fold");
    assert!(inner.is_some(), "inner if must fold independently");
}

#[test]
fn fold01_single_line_block_yields_no_fold() {
    let src = "{% block content %}body{% endblock %}";
    let ranges = fold_ranges(src);
    // All on one line → startLine == endLine → no fold emitted.
    assert!(
        !ranges.iter().any(|r| r.kind == FoldKind::Region),
        "single-line block must not produce a fold"
    );
}

// ─── REQ-FOLD2-02: multi-line comments fold as Comment ───────────────────────

#[test]
fn fold02_multiline_comment_folds_as_comment() {
    let src = "{# this is a\nmulti-line comment #}";
    let ranges = fold_ranges(src);
    let r = ranges
        .iter()
        .find(|r| r.kind == FoldKind::Comment)
        .expect("multi-line comment must fold");
    assert_eq!(r.start_line, 0);
    assert_eq!(r.end_line, 1);
}

#[test]
fn fold02_single_line_comment_yields_no_fold() {
    let src = "{# one-liner #}";
    let ranges = fold_ranges(src);
    assert!(
        !ranges.iter().any(|r| r.kind == FoldKind::Comment),
        "one-line comment must not fold"
    );
}

// ─── REQ-FOLD2-03: multi-line tags fold as region ────────────────────────────

#[test]
fn fold03_multiline_expression_tag_folds() {
    let src = "{{ a\n+ b\n+ c }}";
    let ranges = fold_ranges(src);
    assert!(
        ranges
            .iter()
            .any(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2),
        "multi-line {{ }} must fold as region"
    );
}

#[test]
fn fold03_multiline_statement_tag_folds() {
    let src = "{% set nav = [\n  'a',\n  'b'\n] %}";
    let ranges = fold_ranges(src);
    assert!(
        ranges
            .iter()
            .any(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 3),
        "multi-line {{%...%}} must fold as region"
    );
}

#[test]
fn fold03_single_line_tag_yields_no_multiline_fold() {
    let src = "{{ post.title }}";
    let ranges = fold_ranges(src);
    assert!(ranges.is_empty(), "single-line {{ }} must yield no fold");
}

// ─── REQ-FOLD2-04: 0-based boundaries ────────────────────────────────────────

#[test]
fn fold04_pinned_0based_example() {
    // Spec §5.4 worked example: startLine=0, endLine=2, kind=Region.
    let src = "{% block content %}\n  <h1>{{ post.title }}</h1>\n{% endblock %}";
    let ranges = fold_ranges(src);
    let r = ranges
        .iter()
        .find(|r| r.kind == FoldKind::Region)
        .expect("block must fold");
    assert_eq!(r.start_line, 0);
    assert_eq!(r.end_line, 2);
}

// ─── REQ-FOLD2-05: Jinja-only folds ──────────────────────────────────────────

#[test]
fn fold05_html_content_yields_no_jinja_fold() {
    let src = "<section>\n  <h1>Hello</h1>\n</section>";
    let ranges = fold_ranges(src);
    assert!(ranges.is_empty(), "pure HTML must yield no Jinja folds");
}

// ─── REQ-FOLD2-06: incomplete/unmatched nodes yield no range ─────────────────

#[test]
fn fold06_unclosed_block_yields_no_fold() {
    let src = "{% block content %}\nbody\n(no endblock)";
    let ranges = fold_ranges(src);
    assert!(
        !ranges.iter().any(|r| r.kind == FoldKind::Region && r.start_line == 0),
        "unclosed block must not fold"
    );
}

#[test]
fn fold06_stray_endfor_yields_no_fold() {
    let src = "{% endfor %}";
    let ranges = fold_ranges(src);
    assert!(
        !ranges.iter().any(|r| r.kind == FoldKind::Region),
        "stray closer must not fold"
    );
}

#[test]
fn fold06_unclosed_does_not_suppress_well_formed_pair() {
    // Lines: 0=block, 1=body, 2=endblock, 3=for, 4=no-endfor
    let src = "{% block content %}\nbody\n{% endblock %}\n{% for x in xs %}\n(no endfor)";
    let ranges = fold_ranges(src);
    let block_fold = ranges
        .iter()
        .find(|r| r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2);
    let for_fold = ranges.iter().find(|r| r.kind == FoldKind::Region && r.start_line == 3);
    assert!(block_fold.is_some(), "well-formed block/endblock pair must fold");
    assert!(for_fold.is_none(), "unclosed for must not fold");
}

// ─── REQ-FOLD2-01 / v08w: {% raw %} body tags must not produce folds ─────────

#[test]
fn fold01_raw_inner_multiline_tags_produce_no_nested_fold() {
    // {% raw %} body spans lines 1..3; inner {% for %}…{% endfor %} span lines 2..4.
    // Only the raw pair (lines 1..5) must fold — NOT the inner for/endfor.
    // Lines: 0=before, 1=raw, 2=for, 3=content, 4=endfor, 5=endraw, 6=after
    let src = "before\n{% raw %}\n{% for x in xs %}\n{{ x }}\n{% endfor %}\n{% endraw %}\nafter";
    let ranges = fold_ranges(src);
    let regions: Vec<_> = ranges.iter().filter(|r| r.kind == FoldKind::Region).collect();
    assert_eq!(regions.len(), 1,
        "only raw pair must fold; inner for/endfor must not produce a nested fold: {regions:?}");
    assert_eq!(regions[0].start_line, 1, "raw fold starts on raw line");
    assert_eq!(regions[0].end_line, 5, "raw fold ends on endraw line");
}

#[test]
fn fold01_raw_folds_as_single_region_multiline() {
    // Multiline raw block folds from opener to closer.
    let src = "{% raw %}\nsome content\n{% endraw %}";
    let ranges = fold_ranges(src);
    let regions: Vec<_> = ranges.iter().filter(|r| r.kind == FoldKind::Region).collect();
    assert_eq!(regions.len(), 1, "raw block must fold: {regions:?}");
    assert_eq!(regions[0].start_line, 0);
    assert_eq!(regions[0].end_line, 2);
}

// ─── REQ-FOLD2-03: multi-line block openers also get a tag fold ──────────────

#[test]
fn fold03_multiline_macro_opener_emits_tag_fold() {
    // REQ-FOLD2-03: a multi-line `{% macro %}` opener must fold the tag itself
    // (in addition to the pair fold that covers the whole macro body).
    // Source: line 0 = macro opener (2 lines), line 2 = body, line 3 = endmacro
    let src = "{% macro render(\n  post\n) %}\nbody\n{% endmacro %}";
    let ranges = fold_ranges(src);
    // There must be a Region fold just for the opener tag: lines 0..2 (or 0..1 etc.)
    let opener_tag_fold = ranges.iter().any(|r| {
        r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2
    });
    assert!(opener_tag_fold, "multi-line macro opener must emit a tag fold (REQ-FOLD2-03): {ranges:?}");
}

#[test]
fn fold03_multiline_block_opener_emits_tag_fold() {
    // Same for `{% block %}` — multi-line opener must fold the tag itself.
    let src = "{% block\n  content\n%}\nbody\n{% endblock %}";
    let ranges = fold_ranges(src);
    let opener_tag_fold = ranges.iter().any(|r| {
        r.kind == FoldKind::Region && r.start_line == 0 && r.end_line == 2
    });
    assert!(opener_tag_fold, "multi-line block opener must emit a tag fold (REQ-FOLD2-03): {ranges:?}");
}
