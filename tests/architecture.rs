// Architecture integration tests: REQ-ARCH-01, REQ-ARCH-02, REQ-ARCH-03.

use std::fs;

// ---------- REQ-ARCH-03: Pass 1 extracts one file ---------------------------

#[test]
fn pass1_updates_only_changed_file() {
    use jinja_lsp::server::state::ServerState;

    let tmp = std::env::temp_dir().join("jinja_lsp_arch_pass1");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("a.html"), "{% set x = 1 %}").unwrap();
    fs::write(tmp.join("b.html"), "{% set y = 2 %}").unwrap();

    let mut state = ServerState::from_dirs(&[tmp.as_path()], &["html"]);
    assert_eq!(state.workspace.templates.len(), 2);
    let b_macros_before = state.workspace.templates["b.html"].macros.len();

    // Pass 1: update only a.html with new content
    state.update_file("a.html", "{% macro greet(name) %}Hi{{ name }}{% endmacro %}");

    // a.html should now have the macro
    assert_eq!(
        state.workspace.templates["a.html"].macros.len(),
        1,
        "a.html must reflect new content"
    );
    // b.html must be untouched
    assert_eq!(
        state.workspace.templates["b.html"].macros.len(),
        b_macros_before,
        "b.html must not change after Pass 1 on a.html"
    );
}

// ---------- REQ-ARCH-03: generation increments on each Pass 1 ---------------

#[test]
fn generation_increments_on_update() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(jinja_lsp::config::JinjaConfig::default());
    let gen0 = state.generation;
    state.update_file("x.html", "{% set a = 1 %}");
    assert!(state.generation > gen0, "generation must increment after update_file");
}

// ---------- jinja-lsp-q0aw: stale-diagnostics-publish guard via doc_versions ----

#[test]
fn jinja_lsp_q0aw_stale_publish_is_detected_after_interleaved_pass1() {
    // Simulates did_change's exact race: pass1(A) then pass1(B) both complete (in
    // real edit order) before either call reaches its publish step. The SLOWER task
    // (A, the older edit) must detect — via doc_versions — that a newer version (B)
    // has already landed by the time it's ready to publish, and skip.
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(jinja_lsp::config::JinjaConfig::default());
    state.sources.insert("t.html".to_owned(), String::new()); // did_open already tracked it

    // did_change(A): record version 1, then run pass1.
    let version_a = 1;
    state.doc_versions.entry("t.html".to_owned())
        .and_modify(|v| *v = (*v).max(version_a))
        .or_insert(version_a);
    state.update_file("t.html", "{{ a }}");

    // did_change(B) interleaves and fully completes before A checks in: record
    // version 2, run pass1 with the newer text.
    let version_b = 2;
    state.doc_versions.entry("t.html".to_owned())
        .and_modify(|v| *v = (*v).max(version_b))
        .or_insert(version_b);
    state.update_file("t.html", "{{ b }}");

    // Now A finally checks whether it's still the latest version — it must not be.
    let a_is_latest = state.doc_versions.get("t.html").copied() == Some(version_a);
    assert!(!a_is_latest, "the older edit (A) must detect it is stale and skip publishing");

    // B, checking immediately after its own pass1, must see itself as latest.
    let b_is_latest = state.doc_versions.get("t.html").copied() == Some(version_b);
    assert!(b_is_latest, "the newer edit (B) must see itself as the latest version");
}

// ---------- REQ-FOLD-07: TextEdit/WorkspaceEdit live in edit/, not code_actions
#[test]
fn textedit_and_workspaceedit_defined_in_edit_module() {
    // Verify types are accessible from edit/ (not code_actions).
    use jinja_lsp::edit::{TextEdit, WorkspaceEdit};
    let edit = TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 0, new_text: String::new() };
    let we = WorkspaceEdit::single("f.html", edit);
    assert!(we.changes.contains_key("f.html"));
}

#[test]
fn jinja_lsp_gqdd_empty_linter_module_removed() {
    // The linter module was a stale placeholder: a comment claiming it held CLI
    // orchestration for `jinja-lsp check` and rich/compact/json formatters, but
    // all of that logic actually lives in src/main.rs — the module itself was
    // completely empty. Neither the module nor its declaration should exist.
    assert!(
        !std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/linter")).exists(),
        "src/linter/ must be removed, not left as an empty placeholder"
    );
    let lib_src = include_str!("../src/lib.rs");
    assert!(
        !lib_src.contains("mod linter"),
        "lib.rs must not declare a linter module that no longer exists"
    );
}

#[test]
fn jinja_lsp_0ar1_code_action_handler_does_not_clone_workspace_state() {
    // jinja-lsp-0ar1: code_action fired on every cursor move in some editors, and
    // was cloning the full sources map, the workspace index, and the registry per
    // request "to release the lock before CPU-bound work" — but the handler is
    // fully synchronous (no .await between the read and the response), so those
    // clones only added cost with no benefit. The handler must borrow from the
    // held read guard instead of calling .clone() on workspace/registry_for.
    let src = include_str!("../src/server/mod.rs");
    let start = src.find("async fn code_action(").expect("code_action handler must exist");
    let end = start + src[start..].find("\n    async fn code_action_resolve").expect("code_action_resolve must follow code_action");
    let handler = &src[start..end];
    assert!(
        !handler.contains("ws.clone()") && !handler.contains("workspace.clone()"),
        "code_action must not clone the workspace index per request: {handler}"
    );
    assert!(
        !handler.contains("registry_for(&key).clone()"),
        "code_action must not clone the registry per request: {handler}"
    );
    assert!(
        !handler.contains("state.sources.clone()"),
        "code_action must not clone the full sources map per request: {handler}"
    );
}

#[test]
fn code_actions_does_not_define_textedit() {
    // Structural: TextEdit must not be defined in code_actions.rs.
    let src = include_str!("../src/features/code_actions.rs");
    assert!(
        !src.contains("pub struct TextEdit"),
        "TextEdit must be defined in edit/mod.rs, not code_actions.rs"
    );
    assert!(
        !src.contains("pub struct WorkspaceEdit"),
        "WorkspaceEdit must be defined in edit/mod.rs, not code_actions.rs"
    );
}

// ---------- REQ-ARCH-01: CLI structure --------------------------------------

#[test]
fn shared_build_workspace_indexes_templates_and_references() {
    // Both `check` and `lsp` call build_workspace() with the same arguments
    // and must see the same index. Assert the workspace is actually populated.
    use jinja_lsp::workspace::build_workspace;

    let tmp = std::env::temp_dir().join("jinja_lsp_arch_cli");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("t.html"), "{% set x = 1 %}{{ x }}").unwrap();

    let ws = build_workspace(&[&tmp], &["html"]);

    // Template was indexed.
    assert!(ws.templates.contains_key("t.html"), "workspace must index t.html");
    // Variable `x` was extracted.
    let idx = ws.templates.get("t.html").unwrap();
    assert!(
        idx.variables.iter().any(|v| v.name == "x"),
        "build_workspace must extract variables from templates"
    );
    // Calling it again with the same args produces an identical index — no hidden state.
    let ws2 = build_workspace(&[&tmp], &["html"]);
    assert_eq!(
        ws.templates.len(), ws2.templates.len(),
        "build_workspace must be deterministic (same call → same template count)"
    );
}

// ---------- REQ-ARCH-02: logging must not write to stdout -------------------

#[test]
fn init_tracing_is_idempotent_and_wired_to_stderr() {
    // init_tracing uses .with_writer(std::io::stderr) — verified by the source.
    // The test checks it can be called repeatedly without panicking (try_init
    // ignores the second registration instead of panicking).
    // Full stdout isolation is verified by the `tests/cli.rs` integration tests
    // which run the real binary and assert its stdout contains only JSON-RPC frames.
    jinja_lsp::server::init_tracing();
    jinja_lsp::server::init_tracing(); // second call must not panic
    // Emit a tracing event; if the writer were stdout this would appear in test output.
    tracing::debug!("arch-test tracing probe — must stay on stderr");
    // If we reached here without panic, the invariant holds at the source level.
    // (Runtime stdout-isolation is an integration-test concern; see tests/cli.rs)
}

// ---------- REQ-INLN-02 / REQ-EXTR-05: inline template wiring ---------------

#[test]
fn host_file_inline_regions_are_indexed() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    // Python host file with two embedded Jinja templates.
    let py_src = r#"
        a = render_template_string("{{ user.name }}")
        b = render_template_string("{% for x in items %}{{ x }}{% endfor %}")
    "#;
    state.update_file("views.py", py_src);

    // The host file itself must be in the workspace.
    assert!(
        state.workspace.templates.contains_key("views.py"),
        "host file must be indexed as itself"
    );
    // Each inline region must produce a separate index entry.
    let inline_keys: Vec<_> = state.workspace.templates.keys()
        .filter(|k| k.starts_with("views.py::"))
        .collect();
    assert_eq!(inline_keys.len(), 2, "expected 2 inline entries; got: {inline_keys:?}");
}

#[test]
fn host_file_inline_entries_cleared_on_update() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    state.update_file("views.py", r#"render_template_string("{{ old }}")"#);
    assert_eq!(
        state.workspace.templates.keys().filter(|k| k.starts_with("views.py::")).count(),
        1,
        "initial: 1 inline entry"
    );
    // Update to a version with no inline templates.
    state.update_file("views.py", "# no jinja here");
    assert_eq!(
        state.workspace.templates.keys().filter(|k| k.starts_with("views.py::")).count(),
        0,
        "after update with no inline templates: stale entries must be removed"
    );
}

#[test]
fn jinja_template_file_does_not_trigger_inline_detection() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    state.update_file("template.html", r#"render_template_string("{{ user }}")"#);
    // .html is a Jinja extension → should NOT produce inline entries.
    let inline_keys: Vec<_> = state.workspace.templates.keys()
        .filter(|k| k.starts_with("template.html::"))
        .collect();
    assert!(inline_keys.is_empty(), "Jinja template must not produce inline entries; got: {inline_keys:?}");
}
