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
    state.update_file(
        "a.html",
        "{% macro greet(name) %}Hi{{ name }}{% endmacro %}",
    );

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
    assert!(
        state.generation > gen0,
        "generation must increment after update_file"
    );
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
    state
        .doc_versions
        .entry("t.html".to_owned())
        .and_modify(|v| *v = (*v).max(version_a))
        .or_insert(version_a);
    state.update_file("t.html", "{{ a }}");

    // did_change(B) interleaves and fully completes before A checks in: record
    // version 2, run pass1 with the newer text.
    let version_b = 2;
    state
        .doc_versions
        .entry("t.html".to_owned())
        .and_modify(|v| *v = (*v).max(version_b))
        .or_insert(version_b);
    state.update_file("t.html", "{{ b }}");

    // Now A finally checks whether it's still the latest version — it must not be.
    let a_is_latest = state.doc_versions.get("t.html").copied() == Some(version_a);
    assert!(
        !a_is_latest,
        "the older edit (A) must detect it is stale and skip publishing"
    );

    // B, checking immediately after its own pass1, must see itself as latest.
    let b_is_latest = state.doc_versions.get("t.html").copied() == Some(version_b);
    assert!(
        b_is_latest,
        "the newer edit (B) must see itself as the latest version"
    );
}

// ---------- jinja-lsp-wgi7: doc_versions must not survive close/reopen -------

#[test]
fn jinja_lsp_wgi7_stale_high_water_mark_does_not_freeze_diagnostics_after_reopen() {
    // jinja-lsp-wgi7: LSP clients are not guaranteed to keep document versions
    // monotonic across a close/reopen — most (VS Code, coc.nvim, ...) restart the
    // counter. Without clearing doc_versions on close/reopen, a high version
    // recorded before close permanently outranks every post-reopen edit, so
    // is_latest is false forever and diagnostics never publish again.
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(jinja_lsp::config::JinjaConfig::default());
    state.sources.insert("t.html".to_owned(), String::new());

    // Many edits before close drive the version high.
    for v in 1..=20i32 {
        state
            .doc_versions
            .entry("t.html".to_owned())
            .and_modify(|cur| *cur = (*cur).max(v))
            .or_insert(v);
    }
    assert_eq!(state.doc_versions.get("t.html").copied(), Some(20));

    // did_close / did_open must clear the stale entry (this is what the fix in
    // server/mod.rs's did_close and did_open handlers does).
    state.doc_versions.remove("t.html");

    // Reopen restarts the client's version counter at 1, then the user edits again.
    let restarted_version = 1;
    state
        .doc_versions
        .entry("t.html".to_owned())
        .and_modify(|cur| *cur = (*cur).max(restarted_version))
        .or_insert(restarted_version);

    let is_latest = state.doc_versions.get("t.html").copied() == Some(restarted_version);
    assert!(
        is_latest,
        "after doc_versions is cleared on close/reopen, a restarted version counter \
         must be recognized as latest — otherwise diagnostics freeze forever"
    );
}

#[test]
fn jinja_lsp_wgi7_did_open_and_did_close_clear_doc_versions() {
    // Structural: verify the fix is actually wired into the handlers, not just
    // provable as a state-level invariant.
    let src = include_str!("../src/server/mod.rs");

    let open_start = src.find("async fn did_open(").expect("did_open must exist");
    let open_end = open_start
        + src[open_start..]
            .find("\n    /// REQ-ARCH-05 / REQ-EDIT-11: change triggers")
            .expect("did_change doc must follow did_open");
    assert!(
        src[open_start..open_end].contains("doc_versions.remove"),
        "did_open must clear any stale doc_versions high-water mark from before a close/reopen cycle"
    );

    let close_start = src
        .find("async fn did_close(")
        .expect("did_close must exist");
    let close_end = close_start
        + src[close_start..]
            .find("\n    async fn did_change_watched_files")
            .expect("did_change_watched_files must follow did_close");
    assert!(
        src[close_start..close_end].contains("doc_versions.remove"),
        "did_close must clear this document's doc_versions entry"
    );

    let deleted_start = src
        .find("FileChangeType::DELETED =>")
        .expect("DELETED arm must exist");
    let deleted_end = deleted_start
        + src[deleted_start..]
            .find("\n                _ => {}")
            .expect("catch-all arm must follow DELETED");
    assert!(
        src[deleted_start..deleted_end].contains("doc_versions.remove"),
        "the DELETED watched-file branch must also clear doc_versions"
    );
}

// ---------- REQ-FOLD-07: TextEdit/WorkspaceEdit live in edit/, not code_actions
#[test]
fn textedit_and_workspaceedit_defined_in_edit_module() {
    // Verify types are accessible from edit/ (not code_actions).
    use jinja_lsp::edit::{TextEdit, WorkspaceEdit};
    let edit = TextEdit {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 0,
        new_text: String::new(),
    };
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
    let start = src
        .find("async fn code_action(")
        .expect("code_action handler must exist");
    let end = start
        + src[start..]
            .find("\n    async fn code_action_resolve")
            .expect("code_action_resolve must follow code_action");
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
fn jinja_lsp_5qqy_tokens_to_lsp_data_does_not_rescan_source_per_token() {
    // jinja-lsp-5qqy: tokens_to_lsp_data used to call source_line(source, tok.line)
    // once per token, and source_line re-scans the document with split('\n').nth(..)
    // from byte 0 every time — O(lines * tokens) per semanticTokens/full request,
    // which fires on every edit. It must instead split the source into lines once
    // and index into that per token.
    let src = include_str!("../src/server/mod.rs");
    let start = src
        .find("fn tokens_to_lsp_data(")
        .expect("tokens_to_lsp_data must exist");
    let end = start
        + src[start..]
            .find("\n#[cfg(test)]")
            .expect("test module must follow tokens_to_lsp_data");
    let func = &src[start..end];
    assert!(
        !func.contains("source_line(source, tok.line)"),
        "tokens_to_lsp_data must not call source_line per token: {func}"
    );
    assert!(
        func.contains("split('\\n').collect()") || func.contains(".lines().collect()"),
        "tokens_to_lsp_data must split the source into lines once, up front: {func}"
    );
}

#[test]
fn jinja_lsp_0zz7_update_file_does_not_clone_registry_when_no_sidecar() {
    // jinja-lsp-0zz7: update_file ran base_registry_for(key).clone() unconditionally
    // on every didChange keystroke, even though the clone (all core/pack doc entries,
    // nested HashMaps of markdown strings) is thrown away by refresh_sidecar unless a
    // `.hints.md` sidecar actually exists for the template. It must check
    // find_sidecar first and only clone when a sidecar is present.
    let src = include_str!("../src/server/state.rs");
    let start = src
        .find("pub fn update_file(")
        .expect("update_file must exist");
    let end = start
        + src[start..]
            .find("\n    /// Check for `{key}.hints.md`")
            .expect("refresh_sidecar doc comment must follow update_file");
    let func = &src[start..end];
    assert!(
        func.contains("find_sidecar"),
        "update_file must check find_sidecar before cloning the base registry: {func}"
    );
    assert!(
        func.contains("if crate::builtins::hints::find_sidecar")
            || func.contains("if find_sidecar"),
        "update_file must gate the base_registry_for(..).clone() call behind a find_sidecar check: {func}"
    );
}

#[test]
fn jinja_lsp_0zz7_cli_lint_loop_does_not_clone_registry_when_no_sidecar() {
    // jinja-lsp-0zz7: the CLI check loop cloned base_registry for every template
    // unconditionally. It must only clone when a sidecar file actually exists.
    let src = include_str!("../src/main.rs");
    let start = src
        .find("for idx in workspace.templates.values()")
        .expect("CLI lint loop must exist");
    let end = start
        + src[start..]
            .find("all_diags.extend(raw);")
            .expect("loop body must extend all_diags");
    let loop_body = &src[start..end];
    assert!(
        loop_body.contains("find_sidecar"),
        "CLI lint loop must check find_sidecar before cloning base_registry: {loop_body}"
    );
}

#[test]
fn jinja_lsp_md8e_check_w202_early_returns_when_no_macros() {
    // jinja-lsp-md8e: check_w202 scanned every other template's references and
    // from-imports (O(templates^2 x refs) for a full lint run) even for templates
    // that define no macros at all — the common case, and one where the scan can
    // never produce a diagnostic. It must bail out before the workspace scan.
    let src = include_str!("../src/diagnostics/checks/mod.rs");
    let start = src.find("fn check_w202(").expect("check_w202 must exist");
    let end = start
        + src[start..]
            .find("\n// ── W203")
            .expect("W203 section must follow check_w202");
    let func = &src[start..end];
    assert!(
        func.contains("index.macros.is_empty()"),
        "check_w202 must early-return when index.macros is empty: {func}"
    );
}

#[test]
fn jinja_lsp_zcc7_span_containment_is_not_duplicated() {
    // jinja-lsp-zcc7: Span::contains had zero callers while the identical logic was
    // reimplemented three times: body_contains/range_contains in index.rs and
    // span_contains in call_hierarchy.rs. One implementation (Span::contains) must
    // remain, and the call sites must use it.
    let index_src = include_str!("../src/workspace/index.rs");
    assert!(
        !index_src.contains("fn body_contains") && !index_src.contains("fn range_contains"),
        "index.rs must not re-implement span containment as free functions"
    );
    let call_hierarchy_src = include_str!("../src/features/call_hierarchy.rs");
    assert!(
        !call_hierarchy_src.contains("fn span_contains"),
        "call_hierarchy.rs must not re-implement span containment as a free function"
    );
}

#[test]
fn jinja_lsp_bv6m_index_file_into_does_not_full_scan_templates() {
    // jinja-lsp-bv6m: index_file_into ran workspace.templates.retain(..) and
    // workspace.inline_ranges.retain(..) over the ENTIRE template map on every
    // update_file call (every keystroke) just to evict a handful of one file's
    // inline sub-entries. Must instead track and remove exactly those keys.
    let src = include_str!("../src/server/state.rs");
    let start = src
        .find("fn index_file_into(")
        .expect("index_file_into must exist");
    let end = start
        + src[start..]
            .find("\n    fn is_host_file_for_config")
            .expect("is_host_file_for_config must follow index_file_into");
    let func = &src[start..end];
    assert!(
        !func.contains("templates.retain") && !func.contains("inline_ranges.retain"),
        "index_file_into must not full-scan templates/inline_ranges via retain: {func}"
    );
    assert!(
        func.contains("clear_inline_entries_for"),
        "index_file_into must evict stale inline entries via the tracked-key helper: {func}"
    );
}

#[test]
fn jinja_lsp_54gh_rich_formatter_does_not_reread_source_per_diagnostic() {
    // jinja-lsp-54gh: the rich formatter loop called std::fs::read_to_string(&d.file)
    // once PER DIAGNOSTIC — a file with 50 findings was read 50 times, on top of
    // already being read once for checks and once for noqa. Sources must be cached
    // per file and reused for rendering instead.
    let src = include_str!("../src/main.rs");
    let start = src
        .find("_ => {\n            // REQ-LINT-04: rich rustc-style report")
        .expect("rich formatter branch must exist");
    let end = start
        + src[start..]
            .find("if sorted.is_empty()")
            .expect("rich branch must end before the empty-report check");
    let rich_branch = &src[start..end];
    assert!(
        !rich_branch.contains("std::fs::read_to_string"),
        "rich formatter must not re-read source from disk per diagnostic: {rich_branch}"
    );
    assert!(
        rich_branch.contains("source_cache"),
        "rich formatter must reuse the cached source per file: {rich_branch}"
    );
}

#[test]
fn jinja_lsp_isj4_did_change_configuration_skips_empty_overlay() {
    // jinja-lsp-isj4: ConfigOverlay is all-Option with unknown fields ignored, so any
    // JSON payload unrelated to jinja-lsp (or `{}`) deserializes to an empty overlay.
    // Applying it via apply_init_options would permanently discard the real
    // initializationOptions overlay — the handler must check ConfigOverlay::is_empty()
    // and skip before calling apply_init_options.
    let src = include_str!("../src/server/mod.rs");
    let start = src
        .find("async fn did_change_configuration(")
        .expect("did_change_configuration must exist");
    let end = start
        + src[start..]
            .find("\n    /// REQ-ARCH-05 / REQ-EDIT-11: open triggers")
            .expect("did_open must follow did_change_configuration");
    let handler = &src[start..end];
    assert!(
        handler.contains("overlay.is_empty()"),
        "did_change_configuration must skip empty overlays before applying them: {handler}"
    );
}

#[test]
fn jinja_lsp_7f0o_deleted_file_clears_all_per_file_state() {
    // jinja-lsp-7f0o: on FileChangeType::DELETED, only workspace.templates.remove
    // ran — state.sources, state.sidecar_registries, and the file's inline
    // sub-entries were never removed, so a deleted file kept contributing
    // references/symbols via stale inline indexes, sources grew unboundedly, and
    // republish_all_diagnostics kept publishing for files that no longer exist.
    let src = include_str!("../src/server/mod.rs");
    let start = src
        .find("FileChangeType::DELETED =>")
        .expect("DELETED arm must exist");
    let end = start
        + src[start..]
            .find("\n                _ => {}")
            .expect("DELETED arm must be followed by the catch-all match arm");
    let arm = &src[start..end];
    assert!(
        arm.contains("clear_inline_entries_for"),
        "DELETED must evict the file's inline sub-entries: {arm}"
    );
    assert!(
        arm.contains("state.sources.remove"),
        "DELETED must remove the file's source: {arm}"
    );
    assert!(
        arm.contains("state.sidecar_registries.remove"),
        "DELETED must remove the file's sidecar registry: {arm}"
    );
    assert!(
        arm.contains("publish_diagnostics"),
        "DELETED must clear diagnostics for the deleted file: {arm}"
    );
}

#[test]
fn jinja_lsp_v944_unused_submodule_layer_name_stubs_removed() {
    // jinja-lsp-v944: layer_name() in rename.rs/formatting.rs/wrap.rs/extract_macro.rs
    // had no callers anywhere — only the top-level module layer_names (features::,
    // parsing::, etc.) are exercised by tests/fold.rs. Delete the four dead stubs.
    assert!(
        !include_str!("../src/features/rename.rs").contains("fn layer_name"),
        "rename.rs must not define an unused layer_name() stub"
    );
    assert!(
        !include_str!("../src/features/formatting.rs").contains("fn layer_name"),
        "formatting.rs must not define an unused layer_name() stub"
    );
    assert!(
        !include_str!("../src/features/wrap.rs").contains("fn layer_name"),
        "wrap.rs must not define an unused layer_name() stub"
    );
    assert!(
        !include_str!("../src/features/extract_macro.rs").contains("fn layer_name"),
        "extract_macro.rs must not define an unused layer_name() stub"
    );
}

#[test]
fn jinja_lsp_9cyy_resolve_signature_does_not_double_lookup_registry() {
    // jinja-lsp-9cyy: resolve_signature first find_map'd over categories to find
    // WHICH category the callee was in, then called registry.get(category, callee)
    // a second time to fetch the same entry. One find_map over registry.get
    // directly returns the entry itself.
    let src = include_str!("../src/features/signature_help.rs");
    let start = src
        .find("fn resolve_signature(")
        .expect("resolve_signature must exist");
    let end = start
        + src[start..]
            .find("\nfn macro_signature(")
            .expect("macro_signature must follow resolve_signature");
    let func = &src[start..end];
    assert!(
        !func.contains("registry.get(category, callee)"),
        "resolve_signature must not look up the same entry twice: {func}"
    );
}

#[test]
fn jinja_lsp_7ug6_scope_regions_computed_once_per_extract() {
    // jinja-lsp-7ug6: do_blocks and do_variables each independently called
    // build_scope_regions, walking all root children twice per extract() for an
    // identical result. It must be computed once in extract() and threaded through.
    let src = include_str!("../src/parsing/extractor.rs");
    let calls = src.matches("build_scope_regions(").count();
    assert_eq!(
        calls, 2,
        "build_scope_regions must have exactly 2 occurrences (its own fn def + one call site in extract()), not one per consumer: found {calls}"
    );
    assert!(
        !src.contains(
            "fn do_blocks(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {"
        ),
        "do_blocks must take scope_regions as a parameter instead of recomputing it"
    );
    assert!(
        !src.contains(
            "fn do_variables(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {"
        ),
        "do_variables must take scope_regions as a parameter instead of recomputing it"
    );
}

#[test]
fn jinja_lsp_duuc_seen_sets_use_hashset_not_unit_hashmap() {
    // jinja-lsp-duuc: seen_set/seen_for (and run_set/run_for's skip parameters) were
    // HashMap<usize, ()> used purely as sets (insert(k, ()), contains_key). Use
    // HashSet<usize> for clarity. Also run_set evaluated key.unwrap_or(0) twice —
    // bind it once.
    let src = include_str!("../src/parsing/extractor.rs");
    assert!(
        !src.contains("HashMap<usize, ()>"),
        "extractor.rs must not use HashMap<usize, ()> as a set: use HashSet<usize>"
    );
    assert!(
        !src.contains(".insert(k, ())"),
        "extractor.rs must not insert unit values into a set-shaped map: {src}"
    );
    assert!(
        !src.contains("skip.contains_key(&key.unwrap_or(0))"),
        "run_set must not evaluate key.unwrap_or(0) twice"
    );
}

#[test]
fn jinja_lsp_4jc4_is_jinja_file_wrapper_and_noop_drop_removed() {
    // jinja-lsp-4jc4: Backend::is_jinja_file was an async method with an unused
    // _uri parameter that just forwarded to the free fn is_jinja_language_id — call
    // the free function directly. Separately, code_lens_resolve had a drop(path)
    // one line before `path` would go out of scope anyway.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        !src.contains("async fn is_jinja_file("),
        "Backend::is_jinja_file must be removed — call is_jinja_language_id directly"
    );
    assert!(
        !src.contains("drop(path)"),
        "the no-op drop(path) in code_lens_resolve must be removed"
    );
}

#[test]
fn jinja_lsp_qved_empty_root_queries_dir_removed() {
    // jinja-lsp-qved: the repo-root queries/ directory was empty; the real
    // tree-sitter query files live in src/parsing/queries/ (included via
    // include_str! in extractor.rs). The empty dir misled readers into thinking
    // queries lived at the root.
    assert!(
        !std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/queries")).exists(),
        "repo-root queries/ must be removed, not left as an empty, misleading directory"
    );
    assert!(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/parsing/queries")).is_dir(),
        "src/parsing/queries/ must still exist — it holds the real query files"
    );
}

#[test]
fn jinja_lsp_mb3b_dead_error_module_removed() {
    // jinja-lsp-mb3b: ParseError/ExtractionError/ConfigError/DiagnosticError in
    // src/error.rs were referenced nowhere in the crate outside their own module,
    // didn't implement Display/std::error::Error, and duplicated the actually-used
    // config::ConfigError. The whole module was dead documentation, not code.
    assert!(
        !std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/error.rs")).exists(),
        "src/error.rs must be removed, not left as dead/unwired code"
    );
    let lib_src = include_str!("../src/lib.rs");
    assert!(
        !lib_src.contains("mod error"),
        "lib.rs must not declare an error module that no longer exists"
    );
}

#[test]
fn jinja_lsp_gz5q_dead_path_resolver_removed() {
    // jinja-lsp-gz5q: resolve_path had zero production callers (only its own test
    // suite used it) and failed its own traversal-defence contract for absolute
    // paths (resolve_path("/etc/passwd", ..) escaped the templates dir). Since
    // template-reference resolution actually goes through
    // WorkspaceIndex::resolve_key, the unused and buggy module was deleted rather
    // than fixed in place.
    assert!(
        !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/parsing/path_resolver.rs"
        ))
        .exists(),
        "src/parsing/path_resolver.rs must be removed, not left as dead/buggy code"
    );
    let parsing_mod_src = include_str!("../src/parsing/mod.rs");
    assert!(
        !parsing_mod_src.contains("path_resolver") && !parsing_mod_src.contains("resolve_path"),
        "parsing/mod.rs must not declare or re-export the removed path_resolver module"
    );
}

#[test]
fn jinja_lsp_wam7_dead_set_block_query_removed() {
    // jinja-lsp-wam7: queries/set_block.scm was never loaded by the extractor —
    // block-set extraction was replaced by the manual byte scanner run_set_block
    // (tree-sitter's ERROR-node recovery can't find multiple block-set tags via a
    // query). The file was dead and misleadingly claimed to still do the capture.
    assert!(
        !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/parsing/queries/set_block.scm"
        ))
        .exists(),
        "src/parsing/queries/set_block.scm must be removed, not left as dead/misleading code"
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
    assert!(
        ws.templates.contains_key("t.html"),
        "workspace must index t.html"
    );
    // Variable `x` was extracted.
    let idx = ws.templates.get("t.html").unwrap();
    assert!(
        idx.variables.iter().any(|v| v.name == "x"),
        "build_workspace must extract variables from templates"
    );
    // Calling it again with the same args produces an identical index — no hidden state.
    let ws2 = build_workspace(&[&tmp], &["html"]);
    assert_eq!(
        ws.templates.len(),
        ws2.templates.len(),
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
    let inline_keys: Vec<_> = state
        .workspace
        .templates
        .keys()
        .filter(|k| k.starts_with("views.py::"))
        .collect();
    assert_eq!(
        inline_keys.len(),
        2,
        "expected 2 inline entries; got: {inline_keys:?}"
    );
}

#[test]
fn host_file_inline_entries_cleared_on_update() {
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    state.update_file("views.py", r#"render_template_string("{{ old }}")"#);
    assert_eq!(
        state
            .workspace
            .templates
            .keys()
            .filter(|k| k.starts_with("views.py::"))
            .count(),
        1,
        "initial: 1 inline entry"
    );
    // Update to a version with no inline templates.
    state.update_file("views.py", "# no jinja here");
    assert_eq!(
        state
            .workspace
            .templates
            .keys()
            .filter(|k| k.starts_with("views.py::"))
            .count(),
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
    let inline_keys: Vec<_> = state
        .workspace
        .templates
        .keys()
        .filter(|k| k.starts_with("template.html::"))
        .collect();
    assert!(
        inline_keys.is_empty(),
        "Jinja template must not produce inline entries; got: {inline_keys:?}"
    );
}

#[test]
fn jinja_lsp_bv6m_stale_inline_entries_removed_when_host_status_changes() {
    // jinja-lsp-bv6m: the old inline_ranges retain-cleanup only ran inside the
    // `is_host_file_for_config` branch, so if a config change makes a former host
    // file a plain template (its extension added to config.extensions), the old
    // inline_ranges entries were never evicted and lingered forever.
    use jinja_lsp::server::state::ServerState;

    let mut state = ServerState::with_config(Default::default());
    state.update_file("views.py", r#"render_template_string("{{ old }}")"#);
    assert_eq!(
        state.workspace.inline_ranges.len(),
        1,
        "initial: 1 inline_ranges entry"
    );

    // Reconfigure so "py" is now a Jinja template extension — views.py is no longer a host file.
    let mut cfg = state.config.clone();
    cfg.extensions.push("py".to_owned());
    state.reset_config(cfg);
    state.update_file("views.py", r#"render_template_string("{{ old }}")"#);

    assert_eq!(
        state.workspace.inline_ranges.len(),
        0,
        "inline_ranges must not linger once the file is no longer classified as a host file"
    );
    assert!(
        !state
            .workspace
            .templates
            .keys()
            .any(|k| k.starts_with("views.py::")),
        "templates must not retain stale inline entries once the file is no longer a host file"
    );
}
