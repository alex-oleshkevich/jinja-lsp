// F03 — Extension Packs tests: REQ-EXT-01 through REQ-EXT-05.

use jinja_lsp::builtins::packs::{KNOWN_PACK_NAMES, KnownPack, PackError};
use jinja_lsp::builtins::registry::{Category, Registry, Source};

// ---------- REQ-EXT-01: extras activates packs; unknown names are errors -----

#[test]
fn known_pack_names_are_the_four_frameworks() {
    assert!(KNOWN_PACK_NAMES.contains(&"flask"), "flask must be known");
    assert!(
        KNOWN_PACK_NAMES.contains(&"starlette"),
        "starlette must be known"
    );
    assert!(
        KNOWN_PACK_NAMES.contains(&"starlette-babel"),
        "starlette-babel must be known"
    );
    assert!(
        KNOWN_PACK_NAMES.contains(&"starlette-flash"),
        "starlette-flash must be known"
    );
    assert_eq!(KNOWN_PACK_NAMES.len(), 4, "exactly 4 known packs");
}

#[test]
fn unknown_extra_name_is_error() {
    let result = KnownPack::parse("fastapi");
    assert!(result.is_err(), "fastapi must not be a known pack");
    match result.unwrap_err() {
        PackError::UnknownPack(name) => assert_eq!(name, "fastapi"),
    }
}

#[test]
fn valid_extra_names_parse_ok() {
    assert!(KnownPack::parse("flask").is_ok());
    assert!(KnownPack::parse("starlette").is_ok());
    assert!(KnownPack::parse("starlette-babel").is_ok());
    assert!(KnownPack::parse("starlette-flash").is_ok());
}

// ---------- REQ-EXT-02: pack docs enter registry as Pack(name) --------------

#[test]
fn starlette_url_for_has_pack_source() {
    let mut reg = Registry::load_core();
    reg.load_packs(&["starlette"]);
    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert_eq!(entry.source, Source::Pack("starlette".to_owned()));
}

#[test]
fn flask_url_for_has_flask_pack_source() {
    let mut reg = Registry::load_core();
    reg.load_packs(&["flask"]);
    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert_eq!(entry.source, Source::Pack("flask".to_owned()));
}

// ---------- REQ-EXT-03: disabled pack's symbols are invisible ---------------

#[test]
fn disabled_pack_symbol_not_in_registry() {
    let reg = Registry::load_core(); // no packs loaded
    // url_for is not a core Jinja symbol
    let entry = reg.get(Category::Function, "url_for");
    assert!(
        entry.is_none(),
        "url_for must not exist in core-only registry"
    );
}

#[test]
fn starlette_request_invisible_without_pack() {
    let reg = Registry::load_core();
    assert!(
        reg.get(Category::Variable, "request").is_none(),
        "request must not exist in core-only registry"
    );
}

#[test]
fn starlette_request_visible_with_pack() {
    let mut reg = Registry::load_core();
    reg.load_packs(&["starlette"]);
    assert!(
        reg.get(Category::Variable, "request").is_some(),
        "request must exist after loading starlette pack"
    );
}

// ---------- REQ-EXT-04: four packs ship their full catalogs -----------------

#[test]
fn flask_pack_has_6_docs() {
    let mut reg = Registry::load_core();
    let count = reg.load_packs(&["flask"]);
    assert_eq!(count, 6, "flask pack must contribute 6 docs");
}

#[test]
fn starlette_pack_has_2_docs() {
    let mut reg = Registry::load_core();
    let count = reg.load_packs(&["starlette"]);
    assert_eq!(count, 2, "starlette pack must contribute 2 docs");
}

#[test]
fn starlette_babel_pack_has_10_docs() {
    let mut reg = Registry::load_core();
    let count = reg.load_packs(&["starlette-babel"]);
    assert_eq!(count, 10, "starlette-babel pack must contribute 10 docs");
}

#[test]
fn starlette_flash_pack_has_1_doc() {
    let mut reg = Registry::load_core();
    let count = reg.load_packs(&["starlette-flash"]);
    assert_eq!(count, 1, "starlette-flash pack must contribute 1 doc");
}

#[test]
fn all_packs_total_19_docs() {
    let mut reg = Registry::load_core();
    let count = reg.load_packs(&["flask", "starlette", "starlette-babel", "starlette-flash"]);
    assert_eq!(count, 19, "all packs together must contribute 19 docs");
}

// ---------- REQ-EXT-05: pack vs custom builtins vs hints distinction --------

#[test]
fn pack_source_is_pack_not_custom_or_core() {
    let mut reg = Registry::load_core();
    reg.load_packs(&["flask"]);
    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert!(
        matches!(entry.source, Source::Pack(_)),
        "pack entry must have Source::Pack"
    );
    assert_ne!(entry.source, Source::Core);
    assert_ne!(entry.source, Source::Custom);
    assert_ne!(entry.source, Source::Hint);
}

#[test]
fn hint_overrides_pack_for_same_symbol() {
    let mut reg = Registry::load_core();
    reg.load_packs(&["starlette"]);

    // Insert a Hint entry for url_for — should override the Pack entry
    use jinja_lsp::builtins::registry::DocEntry;
    reg.insert(DocEntry {
        name: "url_for".to_owned(),
        category: Category::Function,
        signature: None,
        since: None,
        params: vec![],
        body: "hint body".to_owned(),
        source: Source::Hint,
        ty: None,
        template: None,
    });

    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert_eq!(entry.source, Source::Hint, "Hint must override Pack");
}

// ---------- jinja-lsp-l6mr: last-loaded pack wins at equal priority ----------

#[test]
fn last_loaded_pack_wins_for_equal_priority() {
    // flask and starlette both define url_for at Pack priority.
    // Loading starlette after flask must give starlette's url_for.
    let mut reg = Registry::load_core();
    reg.load_packs(&["flask", "starlette"]);
    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert!(
        matches!(&entry.source, Source::Pack(name) if name == "starlette"),
        "starlette (loaded second) must win over flask for url_for; got: {:?}",
        entry.source
    );
}

#[test]
fn first_loaded_pack_wins_when_loaded_in_opposite_order() {
    // Load starlette first, then flask — flask should win.
    let mut reg = Registry::load_core();
    reg.load_packs(&["starlette", "flask"]);
    let entry = reg.get(Category::Function, "url_for").unwrap();
    assert!(
        matches!(&entry.source, Source::Pack(name) if name == "flask"),
        "flask (loaded second) must win over starlette for url_for; got: {:?}",
        entry.source
    );
}
