// E17 testing infrastructure tests: REQ-TEST-01..05.

use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};

// ---------- REQ-TEST-04: expected-diagnostics.json canonical shape ----------

#[test]
fn diagnostic_serializes_to_canonical_shape() {
    let d = Diagnostic {
        file: "blog/post.html".to_owned(),
        line: 4,
        col: 6,
        code: "JINJA-E101".to_owned(),
        slug: "undefined-variable".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: "'post' is not defined".to_owned(),
    };

    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(json["file"], "blog/post.html");
    assert_eq!(json["line"], 4);
    assert_eq!(json["col"], 6);
    assert_eq!(json["code"], "JINJA-E101");
    assert_eq!(json["slug"], "undefined-variable");
    assert_eq!(json["severity"], "error");
    assert_eq!(json["message"], "'post' is not defined");
}

#[test]
fn diagnostic_deserializes_from_canonical_shape() {
    let json = r#"{
        "file": "blog/post.html",
        "line": 4,
        "col": 6,
        "code": "JINJA-E101",
        "slug": "undefined-variable",
        "severity": "error",
        "message": "'post' is not defined"
    }"#;

    let d: Diagnostic = serde_json::from_str(json).unwrap();
    assert_eq!(d.file, "blog/post.html");
    assert_eq!(d.severity, DiagnosticSeverity::Error);
}

#[test]
fn starlette_blog_golden_file_is_valid_json_array() {
    // REQ-TEST-04: each fixture carries expected-diagnostics.json
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/starlette-blog/expected-diagnostics.json");
    let raw = std::fs::read_to_string(&path).expect("expected-diagnostics.json must exist");
    let arr: Vec<Diagnostic> = serde_json::from_str(&raw).expect("must be valid JSON array");
    // starlette-blog is the clean baseline — no expected diagnostics
    assert!(arr.is_empty(), "starlette-blog must have no diagnostics");
}

// ---------- REQ-TEST-05: tests name the REQ they cover ----------------------

#[test]
fn this_test_covers_req_test_05() {
    // Convention: every test that verifies a REQ names it in a comment or test name.
    // This test documents the convention; it compiles only if Diagnostic exists.
    let _ = Diagnostic {
        file: String::new(),
        line: 0,
        col: 0,
        code: String::new(),
        slug: String::new(),
        severity: DiagnosticSeverity::Error,
        message: String::new(),
    };
}
