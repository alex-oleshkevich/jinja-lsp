// REQ-CONV-02: no bare .unwrap() in user-data extraction paths
#[test]
fn no_bare_unwrap_in_call_hierarchy() {
    let src = include_str!("../src/features/call_hierarchy.rs");
    assert!(
        !src.contains(".unwrap()"),
        "call_hierarchy.rs must not have bare .unwrap() — use graceful fallback or .expect(reason)"
    );
}

#[test]
fn no_bare_unwrap_in_completions() {
    let src = include_str!("../src/features/completions.rs");
    assert!(
        !src.contains(".unwrap()"),
        "completions.rs must not have bare .unwrap() — use .expect(reason) for invariant-protected sites"
    );
}

#[test]
fn no_bare_unwrap_in_symbols() {
    let src = include_str!("../src/features/symbols.rs");
    assert!(
        !src.contains(".unwrap()"),
        "symbols.rs must not have bare .unwrap() — use .expect(reason) for invariant-protected sites"
    );
}

// REQ-CONV-01: partial extraction — symbols before a syntax error are still emitted

#[test]
fn conv01_partial_extraction_continues_past_syntax_error() {
    use jinja_lsp::parsing::extract;
    // The macro is defined before the unclosed tag. It must still be extracted.
    let src = "{% macro greet(name) %}Hello {{ name }}{% endmacro %}{% if unclosed ";
    let idx = extract(src);
    assert!(
        !idx.macros.is_empty(),
        "REQ-CONV-01: macros defined before syntax error must still be extracted; got none"
    );
    assert_eq!(
        idx.macros[0].name, "greet",
        "extracted macro name must be 'greet'"
    );
}

#[test]
fn conv01_set_variable_before_error_is_extracted() {
    use jinja_lsp::parsing::extract;
    let src = "{% set x = 1 %}{{ x }} {% if unclosed ";
    let idx = extract(src);
    assert!(
        !idx.variables.is_empty(),
        "REQ-CONV-01: variables before syntax error must be extracted"
    );
}

// REQ-CONV-02: no panic on malformed / adversarial input

#[test]
fn conv02_no_panic_on_empty_source() {
    use jinja_lsp::parsing::extract;
    let _idx = extract(""); // must not panic
}

#[test]
fn conv02_no_panic_on_unclosed_delimiter() {
    use jinja_lsp::parsing::extract;
    let _idx = extract("{{ unclosed"); // must not panic
}

#[test]
fn conv02_no_panic_on_deeply_nested_braces() {
    use jinja_lsp::parsing::extract;
    let src = "{% for i in x %}{% for j in y %}{% for k in z %}{{ i }}{% endfor %}{% endfor %}{% endfor %}";
    let _idx = extract(src); // must not panic
}

#[test]
fn conv02_no_panic_on_binary_like_input() {
    use jinja_lsp::parsing::extract;
    // High byte-value ASCII that isn't valid UTF-8 would panic; here we test
    // legitimate-but-bizarre strings.
    let _idx = extract("{{ \u{FEFF}\u{200B} }}"); // BOM + zero-width space
}

// REQ-CONV-04: tracing goes to stderr, not stdout

#[test]
fn conv04_tracing_writes_to_stderr_not_stdout() {
    // The server's init_tracing() wires tracing to stderr.
    // Verify the source code contains `.with_writer(std::io::stderr)` as a
    // static guarantee that tracing never corrupts the JSON-RPC stdout stream.
    let src = include_str!("../src/server/mod.rs");
    assert!(
        src.contains("std::io::stderr"),
        "REQ-CONV-04: init_tracing must route to stderr, not stdout"
    );
    assert!(
        !src.contains("std::io::stdout"),
        "REQ-CONV-04: stdout must never be used as a tracing target"
    );
}

#[test]
fn conv04_init_tracing_does_not_panic() {
    // init_tracing uses try_init so double-registration in tests is safe.
    jinja_lsp::server::init_tracing();
}

// jinja-lsp-1sjt: execute_command must drop the state read guard before the
// client.apply_edit round-trip. apply_edit triggers a client-side didChange
// that needs state.write(), and tokio's write-preferring RwLock would stall
// behind a still-held read guard — a stall or deadlock depending on ordering.
#[test]
fn execute_command_drops_state_guard_before_apply_edit() {
    let src = include_str!("../src/server/mod.rs");
    let pattern = "drop(state);\n                let _ = self.client.apply_edit(lsp_edit).await;";
    let occurrences = src.matches(pattern).count();
    assert_eq!(
        occurrences, 3,
        "expected all 3 execute_command branches (extract-macro, wrap-block, rename) \
         to drop the state guard immediately before client.apply_edit; found {occurrences}"
    );
}

// REQ-REF-02 / F10 §5: workspace-wide lookups must be order-stable.

#[test]
fn no_unsorted_workspace_templates_walk_in_features() {
    // `WorkspaceIndex::templates` is a HashMap, so walking it directly and taking the
    // first match yields a different answer per process (Rust re-seeds its hasher on
    // every run). Two feature modules did exactly that for workspace-wide macro lookup,
    // making the call-hierarchy target and the E103 import quick-fix non-reproducible.
    // Both now go through WorkspaceIndex::find_macro_with_path, which sorts by key.
    //
    // Aggregations that are order-insensitive (collecting into a HashSet, summing) are
    // fine — this guard covers `features/` only, where results are user-visible picks.
    for (name, src) in [
        (
            "call_hierarchy.rs",
            include_str!("../src/features/call_hierarchy.rs"),
        ),
        (
            "code_actions.rs",
            include_str!("../src/features/code_actions.rs"),
        ),
    ] {
        assert!(
            !src.contains(".templates.iter()"),
            "{name} must not walk workspace.templates directly — use a sorted \
             WorkspaceIndex lookup (find_macro_with_path) so the pick is stable \
             across runs"
        );
    }
}

// ─── REQ-FOLD-04: one module per check (jinja-lsp-8yz) ──────────────────────

#[test]
fn each_check_lives_in_its_own_module() {
    // checks/ was a single 1112-line mod.rs holding every check as a private fn.
    // mod.rs is now the dispatcher only: it declares the modules and calls them.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/diagnostics/checks");
    let modules: Vec<String> = std::fs::read_dir(&dir)
        .expect("checks dir must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rs") && n != "mod.rs")
        .collect();
    assert!(
        modules.len() >= 18,
        "REQ-FOLD-04: expected one module per check, found {}: {modules:?}",
        modules.len()
    );

    // Every module name must map to at least one real diagnostic code, so a stray
    // helper file cannot masquerade as a check module.
    use jinja_lsp::diagnostics::DiagCode;
    let known: Vec<String> = DiagCode::ALL
        .iter()
        .map(|c| {
            c.code_str()
                .trim_start_matches("JINJA-")
                .to_ascii_lowercase()
        })
        .collect();
    for m in &modules {
        let stem = m.trim_end_matches(".rs");
        assert!(
            stem.split('_').all(|part| known.iter().any(|k| k == part)),
            "REQ-FOLD-04: {m} does not name a diagnostic code (or a combination of them)"
        );
    }
}

#[test]
fn checks_mod_is_a_dispatcher_not_an_implementation() {
    let src = include_str!("../src/diagnostics/checks/mod.rs");
    assert!(
        src.lines().count() < 200,
        "REQ-FOLD-04: checks/mod.rs must stay a dispatcher; it is {} lines — a check \
         implementation has crept back in",
        src.lines().count()
    );
    // The only `fn check_*` allowed here would be an implementation; the dispatcher
    // calls them but must not define them.
    assert!(
        !src.contains("\nfn check_"),
        "REQ-FOLD-04: checks/mod.rs must not define a check; move it to its own module"
    );
}
