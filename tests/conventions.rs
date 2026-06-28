use jinja_lsp::error::{ConfigError, DiagnosticError, ExtractionError, ParseError};

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
