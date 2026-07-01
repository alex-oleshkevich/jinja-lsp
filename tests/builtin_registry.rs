// F02 — Builtin Registry tests: REQ-BLTN-01 through REQ-BLTN-07.

use jinja_lsp::builtins::registry::{AttrDoc, Category, DocEntry, Registry, Source};

// ---------- REQ-BLTN-01: Registry keyed by (category, name) -----------------

#[test]
fn registry_lookup_by_category_and_name() {
    let reg = Registry::load_core();
    // `upper` exists as a filter
    let filter_upper = reg.get(Category::Filter, "upper");
    assert!(filter_upper.is_some(), "filter 'upper' must be in registry");
    assert_eq!(filter_upper.unwrap().category, Category::Filter);

    // `upper` also exists as a test — they are distinct entries
    let test_upper = reg.get(Category::Test, "upper");
    assert!(test_upper.is_some(), "test 'upper' must be in registry");
    assert_eq!(test_upper.unwrap().category, Category::Test);
}

#[test]
fn registry_filter_and_test_upper_are_distinct() {
    let reg = Registry::load_core();
    let filter = reg.get(Category::Filter, "upper").unwrap();
    let test = reg.get(Category::Test, "upper").unwrap();
    assert_ne!(
        filter.body, test.body,
        "filter/upper and test/upper must be distinct doc entries"
    );
}

#[test]
fn registry_lookup_nonexistent_returns_none() {
    let reg = Registry::load_core();
    assert!(reg.get(Category::Filter, "no_such_filter_xyz").is_none());
}

#[test]
fn registry_scan_by_name_across_categories() {
    let reg = Registry::load_core();
    // `upper` appears in both filter and test — scan should find both
    let hits: Vec<&DocEntry> = reg.scan_by_name("upper");
    assert!(
        hits.len() >= 2,
        "scan_by_name('upper') should return at least 2 entries (filter + test)"
    );
}

// ---------- REQ-BLTN-03/06: all 94 core docs parse; counts match ------------

#[test]
fn core_registry_has_expected_total_entries() {
    let reg = Registry::load_core();
    let total = reg.entry_count();
    assert_eq!(total, 94, "core registry must contain exactly 94 entries");
}

#[test]
fn core_registry_filter_count() {
    let reg = Registry::load_core();
    let count = reg.count_by_category(Category::Filter);
    assert_eq!(count, 50, "core registry must have 50 filter entries");
}

#[test]
fn core_registry_function_count() {
    let reg = Registry::load_core();
    let count = reg.count_by_category(Category::Function);
    assert_eq!(count, 8, "core registry must have 8 function entries");
}

#[test]
fn core_registry_test_count() {
    let reg = Registry::load_core();
    let count = reg.count_by_category(Category::Test);
    assert_eq!(count, 31, "core registry must have 31 test entries");
}

#[test]
fn core_registry_variable_count() {
    let reg = Registry::load_core();
    let count = reg.count_by_category(Category::Variable);
    assert_eq!(count, 5, "core registry must have 5 variable entries");
}

// ---------- REQ-BLTN-03: frontmatter fields ----------------------------------

#[test]
fn truncate_filter_has_signature_and_params() {
    let reg = Registry::load_core();
    let entry = reg.get(Category::Filter, "truncate").unwrap();
    assert!(
        entry.signature.is_some(),
        "truncate must have a signature"
    );
    assert!(
        !entry.params.is_empty(),
        "truncate must have params"
    );
}

#[test]
fn core_entries_have_source_core() {
    let reg = Registry::load_core();
    let entry = reg.get(Category::Filter, "upper").unwrap();
    assert_eq!(entry.source, Source::Core);
}

#[test]
fn doc_entry_body_is_nonempty() {
    let reg = Registry::load_core();
    let entry = reg.get(Category::Filter, "truncate").unwrap();
    assert!(!entry.body.trim().is_empty(), "body must have prose");
}

// ---------- REQ-BLTN-04: serde_yaml, malformed frontmatter degrades ---------

#[test]
fn malformed_frontmatter_is_skipped() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    // Unclosed flow sequence — definitely invalid YAML
    let bad = "---\nname: [unclosed\n---\nsome body";
    let result = parse_doc_str(bad, Source::Custom);
    assert!(result.is_none(), "malformed YAML frontmatter must be skipped");
}

#[test]
fn missing_required_name_field_is_skipped() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let no_name = "---\ncategory: filter\n---\nsome body";
    let result = parse_doc_str(no_name, Source::Custom);
    assert!(result.is_none(), "doc with no 'name' must be skipped");
}

#[test]
fn missing_required_category_field_is_skipped() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let no_cat = "---\nname: foo\n---\nsome body";
    let result = parse_doc_str(no_cat, Source::Custom);
    assert!(result.is_none(), "doc with no 'category' must be skipped");
}

// ---------- REQ-BLTN-05: attribute docs keyed (parent, attr) ----------------

#[test]
fn loop_variable_has_attribute_docs() {
    let reg = Registry::load_core();
    // loop.index must be in the attribute map
    let attr = reg.get_attr("loop", "index");
    assert!(attr.is_some(), "loop.index must be in attribute map");
}

#[test]
fn loop_variable_first_last_attrs() {
    let reg = Registry::load_core();
    assert!(reg.get_attr("loop", "first").is_some(), "loop.first must exist");
    assert!(reg.get_attr("loop", "last").is_some(), "loop.last must exist");
}

// ---------- REQ-BLTN-02: four-source merge, highest priority wins -----------

#[test]
fn merge_higher_priority_wins() {
    let mut reg = Registry::load_core();
    // Insert a Custom entry that overrides filter/upper
    let override_entry = DocEntry {
        name: "upper".to_owned(),
        category: Category::Filter,
        signature: None,
        since: None,
        params: vec![],
        body: "custom upper override".to_owned(),
        source: Source::Custom,
        ty: None,
        template: None,
    };
    reg.insert(override_entry);
    let entry = reg.get(Category::Filter, "upper").unwrap();
    assert_eq!(entry.source, Source::Custom, "Custom must override Core");
    assert_eq!(entry.body, "custom upper override");
}

#[test]
fn merge_lower_priority_does_not_override() {
    let mut reg = Registry::load_core();
    // Inserting Core should NOT override an existing Core entry (no-op for same priority,
    // but more importantly a Core entry cannot override Custom)
    reg.insert(DocEntry {
        name: "upper".to_owned(),
        category: Category::Filter,
        signature: None,
        since: None,
        params: vec![],
        body: "custom body".to_owned(),
        source: Source::Custom,
        ty: None,
        template: None,
    });
    // Now try to insert Core again — must not override Custom
    reg.insert(DocEntry {
        name: "upper".to_owned(),
        category: Category::Filter,
        signature: None,
        since: None,
        params: vec![],
        body: "core body".to_owned(),
        source: Source::Core,
        ty: None,
        template: None,
    });
    let entry = reg.get(Category::Filter, "upper").unwrap();
    assert_eq!(entry.source, Source::Custom, "Core must not override Custom");
}

// ---------- REQ-BLTN-07: custom_builtins loads disk docs non-fatally --------

#[test]
fn custom_builtins_dir_loads_docs() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let doc = "---\nname: my_filter\ncategory: filter\n---\nDoes something custom.";
    fs::write(dir.path().join("my_filter.md"), doc).unwrap();

    let mut reg = Registry::load_core();
    reg.load_custom_builtins(dir.path());

    let entry = reg.get(Category::Filter, "my_filter");
    assert!(entry.is_some(), "custom builtin must be loaded into registry");
    assert_eq!(entry.unwrap().source, Source::Custom);
}

#[test]
fn custom_builtins_malformed_skips_siblings_still_load() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    // bad doc
    fs::write(dir.path().join("bad.md"), "---\nnot valid:\n---\nbody").unwrap();
    // good doc
    fs::write(
        dir.path().join("good.md"),
        "---\nname: good_filter\ncategory: filter\n---\nGood doc.",
    )
    .unwrap();

    let mut reg = Registry::load_core();
    // Must not panic; sibling must load
    reg.load_custom_builtins(dir.path());
    assert!(
        reg.get(Category::Filter, "good_filter").is_some(),
        "sibling doc must load even if another is malformed"
    );
}

// ─── jinja-lsp-hai3: insert_attr priority merge ──────────────────────────────

#[test]
fn hai3_insert_attr_lower_priority_does_not_overwrite_higher() {
    // A Pack-sourced attr (priority 2) must not be overwritten by a Custom-sourced
    // attr (priority 1) that is inserted afterwards.
    let mut reg = Registry::load_core();
    reg.insert_attr(AttrDoc {
        parent: "loop".to_owned(),
        attr: "index".to_owned(),
        ty: Some("pack-type".to_owned()),
        source: Source::Pack("starlette".to_owned()),
    });
    reg.insert_attr(AttrDoc {
        parent: "loop".to_owned(),
        attr: "index".to_owned(),
        ty: Some("custom-type".to_owned()),
        source: Source::Custom,
    });
    let attr = reg.get_attr("loop", "index").expect("loop.index must exist");
    assert_eq!(attr.ty.as_deref(), Some("pack-type"), "pack (priority 2) must beat custom (priority 1)");
}

#[test]
fn hai3_insert_attr_higher_priority_overwrites_lower() {
    let mut reg = Registry::load_core();
    reg.insert_attr(AttrDoc {
        parent: "loop".to_owned(),
        attr: "index".to_owned(),
        ty: Some("custom-type".to_owned()),
        source: Source::Custom,
    });
    reg.insert_attr(AttrDoc {
        parent: "loop".to_owned(),
        attr: "index".to_owned(),
        ty: Some("pack-type".to_owned()),
        source: Source::Pack("starlette".to_owned()),
    });
    let attr = reg.get_attr("loop", "index").expect("loop.index must exist");
    assert_eq!(attr.ty.as_deref(), Some("pack-type"), "pack (priority 2) must win even when inserted second");
}
