// F15 — Code Lens tests: REQ-LENS-01 through REQ-LENS-05.

use jinja_lsp::features::code_lens::{
    CodeLensConfig, LensKind, LensSymbolKind, code_lens, code_lens_resolve, code_lens_targets,
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

// ─── REQ-LENS-01: reference-count lens on every macro and block ───────────────

#[test]
fn lens01_macro_gets_ref_count_lens() {
    let src = "{% macro greet(name) %}hello{% endmacro %}";
    let idx = extract(src);
    let lenses = code_lens("tpl.html", &idx, &CodeLensConfig::default());
    let ref_lenses: Vec<_> = lenses
        .iter()
        .filter(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .collect();
    assert!(
        !ref_lenses.is_empty(),
        "macro must have a reference-count lens"
    );
    assert_eq!(ref_lenses[0].data.symbol_name, "greet");
    assert_eq!(ref_lenses[0].data.symbol_kind, LensSymbolKind::Macro);
}

#[test]
fn lens01_block_gets_ref_count_lens() {
    let src = "{% block content %}body{% endblock %}";
    let idx = extract(src);
    let lenses = code_lens("tpl.html", &idx, &CodeLensConfig::default());
    let ref_lenses: Vec<_> = lenses
        .iter()
        .filter(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .collect();
    assert!(
        !ref_lenses.is_empty(),
        "block must have a reference-count lens"
    );
    assert_eq!(ref_lenses[0].data.symbol_name, "content");
    assert_eq!(ref_lenses[0].data.symbol_kind, LensSymbolKind::Block);
}

#[test]
fn lens01_ref_count_singular_grammar() {
    // A macro called once → "1 reference" (not "1 references").
    let macro_src = "{% macro greet(name) %}hello{% endmacro %}";
    let caller_src = "{{ greet('Bob') }}";
    let workspace = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macro.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens must exist");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("1 reference"),
        "singular grammar required"
    );
}

#[test]
fn lens01_ref_count_plural_grammar() {
    let macro_src = "{% macro greet(name) %}hello{% endmacro %}";
    let caller_src = "{{ greet('Bob') }}{{ greet('Ann') }}";
    let workspace = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macro.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens must exist");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("2 references"),
        "plural grammar required"
    );
}

#[test]
fn lens01_declaration_not_counted() {
    // The macro definition itself must not be counted in the reference count.
    let src = "{% macro greet(name) %}hello{% endmacro %}{{ greet('x') }}";
    let workspace = ws(&[("t.html", src)]);
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens must exist");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    // 1 call, not 2 (declaration excluded).
    assert_eq!(
        resolved.title.as_deref(),
        Some("1 reference"),
        "declaration must not be counted"
    );
}

#[test]
fn lens01_lens_anchored_to_definition_line() {
    // The lens line must correspond to the macro/block definition line.
    let src = "first line\n{% macro greet(name) %}hello{% endmacro %}";
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens must exist");
    assert_eq!(
        lens.line, 1,
        "lens must be anchored to the macro definition line (0-based)"
    );
}

#[test]
fn lens01_multiple_macros_each_get_lens() {
    let src = "{% macro a() %}{% endmacro %}\n{% macro b() %}{% endmacro %}";
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let ref_lenses: Vec<_> = lenses
        .iter()
        .filter(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .collect();
    assert_eq!(
        ref_lenses.len(),
        2,
        "each macro must get its own reference-count lens"
    );
}

// ─── REQ-LENS-02: inheritance lens on every block ────────────────────────────

#[test]
fn lens02_child_block_overrides_base() {
    let base_src = "{% block content %}base{% endblock %}";
    let child_src = "{% extends 'base.html' %}{% block content %}child{% endblock %}";
    let workspace = ws(&[("base.html", base_src), ("child.html", child_src)]);
    let child_idx = extract(child_src);
    let lenses = code_lens("child.html", &child_idx, &CodeLensConfig::default());
    let overrides_lens = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceOverrides && l.data.symbol_name == "content"
        })
        .expect("child block must have an InheritanceOverrides lens");
    let resolved = code_lens_resolve(overrides_lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("overrides base"),
        "child block title must be 'overrides base'"
    );
}

#[test]
fn lens02_parent_block_extended_by_one() {
    let base_src = "{% block content %}base{% endblock %}";
    let child_src = "{% extends 'base.html' %}{% block content %}child{% endblock %}";
    let workspace = ws(&[("base.html", base_src), ("child.html", child_src)]);
    let base_idx = extract(base_src);
    let lenses = code_lens("base.html", &base_idx, &CodeLensConfig::default());
    let ext_lens = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceExtended && l.data.symbol_name == "content"
        })
        .expect("parent block must have InheritanceExtended lens");
    let resolved = code_lens_resolve(ext_lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("extended by 1"),
        "one overrider → 'extended by 1'"
    );
}

#[test]
fn lens02_parent_block_extended_by_n() {
    let base_src = "{% block content %}base{% endblock %}";
    let child1_src = "{% extends 'base.html' %}{% block content %}c1{% endblock %}";
    let child2_src = "{% extends 'base.html' %}{% block content %}c2{% endblock %}";
    let workspace = ws(&[
        ("base.html", base_src),
        ("child1.html", child1_src),
        ("child2.html", child2_src),
    ]);
    let base_idx = extract(base_src);
    let lenses = code_lens("base.html", &base_idx, &CodeLensConfig::default());
    let ext_lens = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceExtended && l.data.symbol_name == "content"
        })
        .expect("parent block must have InheritanceExtended lens");
    let resolved = code_lens_resolve(ext_lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("extended by 2"),
        "two overriders → 'extended by 2'"
    );
}

#[test]
fn lens02_standalone_block_gets_no_inheritance_title() {
    // A block with no parent template and no children gets empty inheritance titles.
    let src = "{% block standalone %}body{% endblock %}";
    let workspace = ws(&[("t.html", src)]);
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    for l in lenses.iter().filter(|l| {
        matches!(
            l.data.lens_kind,
            LensKind::InheritanceOverrides | LensKind::InheritanceExtended
        )
    }) {
        let resolved = code_lens_resolve(l.clone(), &workspace);
        assert!(
            resolved.title.as_deref().unwrap_or("").is_empty(),
            "standalone block must get no inheritance title; got {:?}",
            resolved.title
        );
    }
}

#[test]
fn lens02_3level_chain_base_extended_by_2() {
    // base → mid → child: base block is overridden by both mid AND child (deep chain rule).
    let base_src = "{% block content %}base{% endblock %}";
    let mid_src = "{% extends 'base.html' %}{% block content %}mid{% endblock %}";
    let child_src = "{% extends 'mid.html' %}{% block content %}child{% endblock %}";
    let workspace = ws(&[
        ("base.html", base_src),
        ("mid.html", mid_src),
        ("child.html", child_src),
    ]);
    let base_idx = extract(base_src);
    let lenses = code_lens("base.html", &base_idx, &CodeLensConfig::default());
    let ext_lens = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceExtended && l.data.symbol_name == "content"
        })
        .expect("base block must have InheritanceExtended lens");
    let resolved = code_lens_resolve(ext_lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some("extended by 2"),
        "base block must count ALL descendants (base→mid→child), not just direct children"
    );
}

#[test]
fn lens02_mid_chain_block_has_both_directions() {
    // mid block both overrides base AND is extended by child.
    let base_src = "{% block content %}base{% endblock %}";
    let mid_src = "{% extends 'base.html' %}{% block content %}mid{% endblock %}";
    let child_src = "{% extends 'mid.html' %}{% block content %}child{% endblock %}";
    let workspace = ws(&[
        ("base.html", base_src),
        ("mid.html", mid_src),
        ("child.html", child_src),
    ]);
    let mid_idx = extract(mid_src);
    let lenses = code_lens("mid.html", &mid_idx, &CodeLensConfig::default());

    let overrides = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceOverrides && l.data.symbol_name == "content"
        })
        .expect("mid block must have InheritanceOverrides lens");
    let resolved_overrides = code_lens_resolve(overrides.clone(), &workspace);
    assert_eq!(
        resolved_overrides.title.as_deref(),
        Some("overrides base"),
        "mid must override base"
    );

    let extended = lenses
        .iter()
        .find(|l| {
            l.data.lens_kind == LensKind::InheritanceExtended && l.data.symbol_name == "content"
        })
        .expect("mid block must have InheritanceExtended lens");
    let resolved_extended = code_lens_resolve(extended.clone(), &workspace);
    assert_eq!(
        resolved_extended.title.as_deref(),
        Some("extended by 1"),
        "mid must be extended by child"
    );
}

#[test]
fn lens02_block_has_both_ref_count_and_inheritance_lenses() {
    // A block can carry BOTH reference-count and inheritance lenses.
    let base_src = "{% block content %}base{% endblock %}";
    let base_idx = extract(base_src);
    let lenses = code_lens("base.html", &base_idx, &CodeLensConfig::default());

    assert!(
        lenses.iter().any(
            |l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "content"
        ),
        "block must have a reference-count lens"
    );
    assert!(
        lenses
            .iter()
            .any(|l| l.data.lens_kind == LensKind::InheritanceExtended
                && l.data.symbol_name == "content"),
        "block must also have an InheritanceExtended lens"
    );
}

// ─── REQ-LENS-03: independent toggles ────────────────────────────────────────

#[test]
fn lens03_references_off_no_ref_count_lenses() {
    let src = "{% macro greet() %}{% endmacro %}{% block content %}{% endblock %}";
    let idx = extract(src);
    let config = CodeLensConfig {
        references: false,
        inheritance: true,
    };
    let lenses = code_lens("t.html", &idx, &config);
    assert!(
        lenses
            .iter()
            .all(|l| l.data.lens_kind != LensKind::ReferenceCount),
        "references=false must omit all reference-count lenses"
    );
}

#[test]
fn lens03_inheritance_off_no_inheritance_lenses() {
    let src = "{% block content %}{% endblock %}";
    let idx = extract(src);
    let config = CodeLensConfig {
        references: true,
        inheritance: false,
    };
    let lenses = code_lens("t.html", &idx, &config);
    assert!(
        lenses.iter().all(|l| {
            l.data.lens_kind != LensKind::InheritanceOverrides
                && l.data.lens_kind != LensKind::InheritanceExtended
        }),
        "inheritance=false must omit all inheritance lenses"
    );
}

#[test]
fn lens03_both_off_empty_response() {
    let src = "{% macro greet() %}{% endmacro %}{% block content %}{% endblock %}";
    let idx = extract(src);
    let config = CodeLensConfig {
        references: false,
        inheritance: false,
    };
    let lenses = code_lens("t.html", &idx, &config);
    assert!(lenses.is_empty(), "both kinds disabled → empty response");
}

#[test]
fn lens03_both_on_by_default() {
    let src = "{% macro greet() %}{% endmacro %}{% block content %}{% endblock %}";
    let idx = extract(src);
    let config = CodeLensConfig::default();
    let lenses = code_lens("t.html", &idx, &config);
    assert!(
        lenses
            .iter()
            .any(|l| l.data.lens_kind == LensKind::ReferenceCount),
        "default config must include reference-count lenses"
    );
    assert!(
        lenses.iter().any(|l| {
            l.data.lens_kind == LensKind::InheritanceOverrides
                || l.data.lens_kind == LensKind::InheritanceExtended
        }),
        "default config must include inheritance lenses"
    );
}

// ─── REQ-LENS-04: lazy resolve ────────────────────────────────────────────────

#[test]
fn lens04_initial_has_no_title() {
    let src = "{% macro greet(name) %}hello{% endmacro %}";
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    for lens in &lenses {
        assert!(
            lens.title.is_none(),
            "initial lens must have no title (Anchored state); got {:?}",
            lens.title
        );
    }
}

#[test]
fn lens04_resolve_fills_title() {
    let macro_src = "{% macro greet(name) %}hello{% endmacro %}";
    let caller_src = "{{ greet('Bob') }}";
    let workspace = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macro.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .expect("greet must have a reference-count lens");
    assert!(lens.title.is_none(), "must start without title");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert!(resolved.title.is_some(), "resolve must fill in title");
    assert!(
        !resolved.title.as_deref().unwrap_or("").is_empty(),
        "resolved title must not be empty"
    );
}

#[test]
fn lens04_data_stable_by_name_and_kind() {
    // Inserting text above/below the definition does NOT change the symbol id or resolve result.
    let src = "{% macro greet(name) %}hello{% endmacro %}";
    let shifted_src = "extra line\n{% macro greet(name) %}hello{% endmacro %}";
    let caller_src = "{{ greet('Bob') }}";

    // Lens was listed against original source.
    let idx_orig = extract(src);
    let lenses_orig = code_lens("t.html", &idx_orig, &CodeLensConfig::default());
    let lens = lenses_orig
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens");

    // Now workspace has shifted source; resolve must still find greet by (kind, name).
    let mut ws_shifted = WorkspaceIndex::default();
    ws_shifted.index_inline("t.html", shifted_src);
    ws_shifted.index_inline("caller.html", caller_src);

    let resolved = code_lens_resolve(lens.clone(), &ws_shifted);
    assert_eq!(
        resolved.title.as_deref(),
        Some("1 reference"),
        "resolve must find symbol by (kind, name), not exact byte position"
    );
}

#[test]
fn lens04_stale_symbol_returns_empty_title() {
    // Symbol is listed, then the template changes and the macro is gone.
    let src_with_macro = "{% macro greet(name) %}hello{% endmacro %}";
    let workspace_empty = ws(&[("t.html", "just plain text")]); // macro gone

    let idx = extract(src_with_macro);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "greet")
        .expect("greet lens");

    let resolved = code_lens_resolve(lens.clone(), &workspace_empty);
    // P3: returns lens with empty title rather than throwing.
    assert_eq!(
        resolved.title.as_deref(),
        Some(""),
        "stale symbol must resolve to empty title (P3)"
    );
}

// ─── REQ-LENS-05: suppress reference-count lens at count 0 ───────────────────

#[test]
fn lens05_zero_usage_macro_resolves_to_empty_title() {
    let src = "{% macro unused() %}{% endmacro %}";
    let workspace = ws(&[("t.html", src)]); // no callers
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "unused")
        .expect("unused must have a ref-count lens (emitted; suppressed at resolve)");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some(""),
        "zero-usage macro ref-count lens must resolve to empty title (suppressed)"
    );
}

#[test]
fn lens05_zero_usage_block_resolves_to_empty_ref_count_title() {
    // A block with no child overrides gets a suppressed (empty) reference-count lens.
    let src = "{% block orphan %}body{% endblock %}";
    let workspace = ws(&[("t.html", src)]);
    let idx = extract(src);
    let lenses = code_lens("t.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "orphan")
        .expect("orphan block must have a ref-count lens stub");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_eq!(
        resolved.title.as_deref(),
        Some(""),
        "zero-override block ref-count lens must be suppressed"
    );
}

#[test]
fn lens05_smallest_visible_count_is_1() {
    // The lens is visible starting from count 1 (not 0).
    let macro_src = "{% macro f() %}{% endmacro %}";
    let caller_src = "{{ f() }}";
    let workspace = ws(&[("macro.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macro.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "f")
        .expect("f lens");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_ne!(
        resolved.title.as_deref(),
        Some(""),
        "count=1 must produce a visible title (not suppressed)"
    );
    assert_eq!(resolved.title.as_deref(), Some("1 reference"));
}

// ─── §10 edge cases ───────────────────────────────────────────────────────────

#[test]
fn lens_cycle_in_extends_chain_does_not_hang() {
    // a.html extends b.html, b.html extends a.html — cycle guard must terminate.
    let a_src = "{% extends 'b.html' %}{% block x %}{% endblock %}";
    let b_src = "{% extends 'a.html' %}{% block x %}{% endblock %}";
    let workspace = ws(&[("a.html", a_src), ("b.html", b_src)]);
    let idx = extract(a_src);
    let lenses = code_lens("a.html", &idx, &CodeLensConfig::default());
    // Must not infinite-loop; just verify all resolves complete.
    for lens in &lenses {
        let _ = code_lens_resolve(lens.clone(), &workspace);
    }
}

#[test]
fn lens01_ref_count_cross_file() {
    // Macro defined in one file, called in another.
    let macro_src = "{% macro ping() %}{% endmacro %}";
    let caller_src = "{{ ping() }}{{ ping() }}{{ ping() }}";
    let workspace = ws(&[("macros.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macros.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "ping")
        .expect("ping lens");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    assert_eq!(resolved.title.as_deref(), Some("3 references"));
}

#[test]
fn lens01_same_name_macro_in_another_file_not_counted() {
    // A macro named `btn` in macros.html and an unrelated macro also named `btn`
    // in other.html — the ref count for macros.html's btn must NOT include
    // other.html's internal calls to its own btn macro.
    let macro_src = "{% macro btn() %}{% endmacro %}";
    let other_src = "{% macro btn() %}{% endmacro %}{{ btn() }}{{ btn() }}";
    let workspace = ws(&[("macros.html", macro_src), ("other.html", other_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macros.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount && l.data.symbol_name == "btn")
        .expect("btn lens");
    let resolved = code_lens_resolve(lens.clone(), &workspace);
    // other.html defines its own btn — those 2 calls must not be attributed to macros.html.
    // Count=0 is suppressed to empty string (REQ-LENS-05).
    assert_eq!(resolved.title.as_deref(), Some(""));
}

// ─── jinja-lsp-qpc6: navigation targets for resolved lenses ───────────────────

#[test]
fn lens_target_macro_ref_count_points_at_call_sites() {
    let macro_src = "{% macro ping() %}{% endmacro %}";
    let caller_src = "{{ ping() }}";
    let workspace = ws(&[("macros.html", macro_src), ("caller.html", caller_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macros.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .expect("ping lens");
    let targets = code_lens_targets(&lens.data, &workspace);
    assert_eq!(targets.len(), 1, "one call site: {targets:?}");
    assert_eq!(targets[0].path, "caller.html");
}

#[test]
fn lens_target_macro_ref_count_excludes_shadowing_macro_in_other_file() {
    // Mirrors lens01_same_name_macro_in_another_file_not_counted: the count and
    // the navigation targets must never drift apart.
    let macro_src = "{% macro btn() %}{% endmacro %}";
    let other_src = "{% macro btn() %}{% endmacro %}{{ btn() }}{{ btn() }}";
    let workspace = ws(&[("macros.html", macro_src), ("other.html", other_src)]);
    let idx = extract(macro_src);
    let lenses = code_lens("macros.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::ReferenceCount)
        .expect("btn lens");
    let targets = code_lens_targets(&lens.data, &workspace);
    assert!(
        targets.is_empty(),
        "other.html's calls to its own btn must not appear as targets: {targets:?}"
    );
}

#[test]
fn lens_target_block_overrides_points_at_nearest_ancestor() {
    let base_src = "{% block content %}base{% endblock %}";
    let mid_src = "{% extends 'base.html' %}{% block content %}mid{% endblock %}";
    let child_src = "{% extends 'mid.html' %}{% block content %}child{% endblock %}";
    let workspace = ws(&[
        ("base.html", base_src),
        ("mid.html", mid_src),
        ("child.html", child_src),
    ]);
    let idx = extract(child_src);
    let lenses = code_lens("child.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::InheritanceOverrides)
        .expect("overrides lens");
    let targets = code_lens_targets(&lens.data, &workspace);
    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0].path, "mid.html",
        "must point at the nearest ancestor override, not the root base.html"
    );
}

#[test]
fn lens_target_block_extended_points_at_every_descendant_override() {
    let base_src = "{% block content %}base{% endblock %}";
    let mid_src = "{% extends 'base.html' %}{% block content %}mid{% endblock %}";
    let leaf_src = "{% extends 'mid.html' %}{% block content %}leaf{% endblock %}";
    let workspace = ws(&[
        ("base.html", base_src),
        ("mid.html", mid_src),
        ("leaf.html", leaf_src),
    ]);
    let idx = extract(base_src);
    let lenses = code_lens("base.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::InheritanceExtended)
        .expect("extended lens");
    let targets = code_lens_targets(&lens.data, &workspace);
    let mut paths: Vec<&str> = targets.iter().map(|t| t.path.as_str()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec!["leaf.html", "mid.html"],
        "must include every descendant override, not just immediate children (REQ-LENS-02)"
    );
}

#[test]
fn lens_target_block_overrides_none_when_no_ancestor_defines_it() {
    let src = "{% block content %}only{% endblock %}";
    let workspace = ws(&[("solo.html", src)]);
    let idx = extract(src);
    let lenses = code_lens("solo.html", &idx, &CodeLensConfig::default());
    let lens = lenses
        .iter()
        .find(|l| l.data.lens_kind == LensKind::InheritanceOverrides)
        .expect("overrides lens");
    let targets = code_lens_targets(&lens.data, &workspace);
    assert!(targets.is_empty());
}
