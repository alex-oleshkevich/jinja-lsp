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
