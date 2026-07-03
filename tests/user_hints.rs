// F04 — User Hints tests: REQ-HINT-01 through REQ-HINT-08.

use jinja_lsp::builtins::registry::{Category, Registry, Source};
use jinja_lsp::diagnostics::DiagCode;
use std::path::Path;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/user-hints")
}

fn hints_dir() -> std::path::PathBuf {
    fixtures_dir().join("hints")
}

fn templates_dir() -> std::path::PathBuf {
    fixtures_dir().join("templates")
}

// ---------- REQ-HINT-03: extended format parses context_variable + attributes --

#[test]
fn context_variable_category_parses() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let src = "---\nname: post\ncategory: context_variable\ntype: Post\n---\nA post.";
    let result = parse_doc_str(src, Source::Hint);
    assert!(result.is_some(), "context_variable hint must parse");
    let (entry, _) = result.unwrap();
    assert_eq!(entry.category, Category::ContextVariable);
    assert_eq!(entry.source, Source::Hint);
}

#[test]
fn hint_type_field_is_stored() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let src = "---\nname: post\ncategory: context_variable\ntype: Post\n---\nA post.";
    let (entry, _) = parse_doc_str(src, Source::Hint).unwrap();
    assert_eq!(entry.ty.as_deref(), Some("Post"));
}

#[test]
fn hint_template_field_is_stored() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let src = "---\nname: user\ncategory: context_variable\ntemplate: detail.html\n---\nA user.";
    let (entry, _) = parse_doc_str(src, Source::Hint).unwrap();
    assert_eq!(entry.template.as_deref(), Some("detail.html"));
}

#[test]
fn hint_attributes_become_attr_docs() {
    use jinja_lsp::builtins::registry::parse_doc_str;
    let src = "---\nname: post\ncategory: context_variable\nattributes:\n  - name: title\n    type: string\n---\nA post.";
    let (_, attrs) = parse_doc_str(src, Source::Hint).unwrap();
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0].attr, "title");
    assert_eq!(attrs[0].parent, "post");
}

// ---------- REQ-HINT-01: sidecar file auto-discovered beside template ----------

#[test]
fn sidecar_file_is_discovered() {
    use jinja_lsp::builtins::hints::find_sidecar;
    let template = templates_dir().join("detail.html");
    let sidecar = find_sidecar(&template);
    assert!(
        sidecar.is_some(),
        "sidecar must be found beside detail.html"
    );
    assert!(
        sidecar.unwrap().ends_with("detail.html.hints.md"),
        "sidecar path must end with detail.html.hints.md"
    );
}

#[test]
fn missing_sidecar_returns_none() {
    use jinja_lsp::builtins::hints::find_sidecar;
    let template = templates_dir().join("base.html"); // no sidecar
    assert!(find_sidecar(&template).is_none());
}

#[test]
fn loading_sidecar_adds_hint_to_registry() {
    use jinja_lsp::builtins::hints::load_sidecar;
    let mut reg = Registry::load_core();
    let template = templates_dir().join("detail.html");
    load_sidecar(&template, &mut reg);
    let entry = reg.get(Category::ContextVariable, "page_title");
    assert!(entry.is_some(), "sidecar hint must be in registry");
    assert_eq!(entry.unwrap().source, Source::Hint);
}

// ---------- REQ-HINT-02: hints dirs are scanned globally ----------------------

#[test]
fn hints_dir_scan_loads_all_md_files() {
    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(&hints_dir());
    assert!(
        reg.get(Category::ContextVariable, "post").is_some(),
        "post from hints dir must be loaded"
    );
}

#[test]
fn global_hint_has_no_template_scope() {
    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(&hints_dir());
    let entry = reg.get(Category::ContextVariable, "post").unwrap();
    assert!(
        entry.template.is_none(),
        "post hint must be global (no template field)"
    );
}

#[test]
fn scoped_hint_has_template_field() {
    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(&hints_dir());
    let entry = reg.get(Category::ContextVariable, "user").unwrap();
    assert_eq!(
        entry.template.as_deref(),
        Some("detail.html"),
        "user hint must be scoped to detail.html"
    );
}

// ---------- REQ-HINT-07: hints merge at highest priority ----------------------

#[test]
fn hint_overrides_builtin_join_filter() {
    let mut reg = Registry::load_core();
    // core join filter already present
    assert!(reg.get(Category::Filter, "join").is_some());
    // load hints dir containing join_override.hints.md
    reg.load_hints_from_dir(&hints_dir());
    let entry = reg.get(Category::Filter, "join").unwrap();
    assert_eq!(
        entry.source,
        Source::Hint,
        "Hint must override Core for join"
    );
}

#[test]
fn hint_source_is_hint_not_core() {
    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(&hints_dir());
    let entry = reg.get(Category::ContextVariable, "post").unwrap();
    assert_eq!(entry.source, Source::Hint);
    assert_ne!(entry.source, Source::Core);
}

// ---------- REQ-HINT-04: hinted context_variable is known to registry --------

#[test]
fn context_variable_is_known_after_hint() {
    let mut reg = Registry::load_core();
    // post is not in core
    assert!(reg.get(Category::ContextVariable, "post").is_none());
    reg.load_hints_from_dir(&hints_dir());
    // now it's known
    assert!(reg.get(Category::ContextVariable, "post").is_some());
}

// ---------- REQ-HINT-05: JINJA-W106 is defined in DiagCode -------------------

#[test]
fn w106_exists_in_diag_code() {
    // W106 unknown-attribute must be in the catalog
    assert_eq!(DiagCode::W106.code_str(), "JINJA-W106");
    assert_eq!(DiagCode::W106.slug(), "unknown-attribute");
}

#[test]
fn post_attributes_in_registry_after_hint() {
    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(&hints_dir());
    assert!(
        reg.get_attr("post", "title").is_some(),
        "post.title must be in attr map"
    );
    assert!(
        reg.get_attr("post", "slug").is_some(),
        "post.slug must be in attr map"
    );
    assert!(
        reg.get_attr("post", "body").is_some(),
        "post.body must be in attr map"
    );
    assert!(
        reg.get_attr("post", "author").is_some(),
        "post.author must be in attr map"
    );
}

// ---------- REQ-HINT-01: ServerState.update_file wires sidecar into registry --

#[test]
fn server_state_sidecar_cached_on_update_file() {
    use jinja_lsp::builtins::registry::{Category, Source};
    use jinja_lsp::server::state::ServerState;

    let template_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/user-hints/templates/detail.html");
    let key = template_path.to_str().unwrap();

    let mut state = ServerState::with_config(Default::default());
    state.update_file(key, "{{ page_title }}");

    // The sidecar entry must have been cached.
    assert!(
        state.sidecar_registries.contains_key(key),
        "sidecar_registries must have an entry for the template with a sidecar"
    );

    // registry_for must return the sidecar-overlaid registry.
    let reg = state.registry_for(key);
    let entry = reg.get(Category::ContextVariable, "page_title");
    assert!(
        entry.is_some(),
        "registry_for must include the sidecar hint page_title"
    );
    assert_eq!(entry.unwrap().source, Source::Hint);
}

#[test]
fn server_state_no_sidecar_entry_for_template_without_sidecar() {
    use jinja_lsp::server::state::ServerState;

    let template_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/user-hints/templates/base.html");
    let key = template_path.to_str().unwrap();

    let mut state = ServerState::with_config(Default::default());
    state.update_file(key, "{% block body %}{% endblock %}");

    assert!(
        !state.sidecar_registries.contains_key(key),
        "sidecar_registries must be empty for a template without a sidecar"
    );
}

#[test]
fn server_state_refresh_sidecar_evicts_stale_entry() {
    use jinja_lsp::builtins::registry::Registry;
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    // Manually insert a fake sidecar entry.
    state
        .sidecar_registries
        .insert("no_such.html".to_owned(), Registry::load_core());

    // refresh_sidecar for a path with no sidecar on disk must evict the entry.
    let base = state.base_registry_for("no_such.html").clone();
    state.refresh_sidecar("no_such.html", base);

    assert!(
        !state.sidecar_registries.contains_key("no_such.html"),
        "stale sidecar entry must be evicted when no sidecar file exists"
    );
}

// ---------- REQ-HINT-08 / frvv: sidecar live-reload via refresh_sidecar -------

#[test]
fn sidecar_live_reload_updates_registry() {
    use jinja_lsp::builtins::registry::Category;
    use jinja_lsp::server::state::ServerState;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let template = dir.path().join("view.html");
    let sidecar = dir.path().join("view.html.hints.md");
    fs::write(&template, "{{ ctx_var }}").unwrap();
    // Write initial sidecar — declares ctx_var.
    fs::write(
        &sidecar,
        "---\nname: ctx_var\ncategory: context_variable\n---\nA context var.",
    )
    .unwrap();

    let key = template.to_str().unwrap();
    let mut state = ServerState::with_config(Default::default());
    state.update_file(key, "{{ ctx_var }}");
    assert!(
        state
            .registry_for(key)
            .get(Category::ContextVariable, "ctx_var")
            .is_some(),
        "initial sidecar hint must be in registry"
    );

    // Simulate editing the sidecar: add a second variable.
    fs::write(
        &sidecar,
        "---\nname: new_var\ncategory: context_variable\n---\nAdded by live-reload.",
    )
    .unwrap();
    let base = state.base_registry_for(key).clone();
    state.refresh_sidecar(key, base);

    let reg = state.registry_for(key);
    assert!(
        reg.get(Category::ContextVariable, "new_var").is_some(),
        "live-reload must add new_var after sidecar edit"
    );
}

#[test]
fn server_registers_hints_md_watcher() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("*.hints.md"),
        "server must register a *.hints.md filesystem watcher for live-reload (REQ-HINT-08)"
    );
}

#[test]
fn server_dispatches_sidecar_on_hints_md_change() {
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains(".hints.md") && src.contains("refresh_sidecar"),
        "server did_change_watched_files must call refresh_sidecar when a .hints.md file changes"
    );
}

// ---------- REQ-HINT-08: malformed hint is skipped, siblings load -------------

#[test]
fn malformed_hint_skipped_siblings_load() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("bad.hints.md"),
        "---\nnot valid: yaml: :\n---\nbody",
    )
    .unwrap();
    fs::write(
        dir.path().join("good.hints.md"),
        "---\nname: good_var\ncategory: context_variable\n---\nA good var.",
    )
    .unwrap();

    let mut reg = Registry::load_core();
    reg.load_hints_from_dir(dir.path());
    assert!(
        reg.get(Category::ContextVariable, "good_var").is_some(),
        "good hint must load even if sibling is malformed"
    );
}
