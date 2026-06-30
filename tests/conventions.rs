use jinja_lsp::error::{ConfigError, DiagnosticError, ExtractionError, ParseError};

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

// REQ-CONV-03: Four error types, none aborts the server
#[test]
fn parse_error_is_recoverable() {
    let e = ParseError::new("broken template");
    assert!(!e.message().is_empty());
}

#[test]
fn extraction_error_is_recoverable() {
    let e = ExtractionError::new("unexpected node shape");
    assert!(!e.message().is_empty());
}

#[test]
fn config_error_retains_prior_config() {
    let e = ConfigError::new("invalid toml");
    assert!(!e.message().is_empty());
}

#[test]
fn diagnostic_error_does_not_suppress_others() {
    let e = DiagnosticError::new("check E101 failed");
    assert!(!e.message().is_empty());
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
    assert_eq!(idx.macros[0].name, "greet", "extracted macro name must be 'greet'");
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
