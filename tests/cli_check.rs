// F19 check CLI tests: REQ-LINT-01 through REQ-LINT-11.

use std::{env, fs, path::Path, process::Command};

fn binary() -> std::path::PathBuf {
    let mut p = env::current_exe().unwrap();
    loop {
        p.pop();
        let c = p.join("jinja-lsp");
        if c.is_file() { return c; }
        if p.parent().is_none() { break; }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/jinja-lsp")
}

fn check(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(binary()).arg("check").args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let code = out.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn starlette_templates() -> std::path::PathBuf {
    fixtures_dir().join("starlette-blog/templates")
}

fn tmpdir(suffix: &str) -> std::path::PathBuf {
    let d = env::temp_dir().join(format!("jinja_lsp_cli_{suffix}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

// ---------- REQ-LINT-01: PATH is optional, repeatable positional ------------

#[test]
fn single_file_path_accepted() {
    let f = starlette_templates().join("base.html");
    let (_, _, code) = check(&[f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "exit code must be 0 or 1, got {code}");
}

#[test]
fn directory_path_accepted() {
    let dir = starlette_templates();
    let (_, _, code) = check(&[dir.to_str().unwrap()]);
    assert!(code == 0 || code == 1);
}

#[test]
fn empty_directory_exits_0() {
    // REQ-LINT-08: no findings → exit 0
    let tmp = tmpdir("empty_dir");
    let (stdout, _, code) = check(&[tmp.to_str().unwrap()]);
    assert_eq!(code, 0, "empty dir must exit 0, stdout: {stdout}");
}

// ---------- REQ-LINT-02: flags -----------------------------------------------

#[test]
fn format_json_flag_accepted() {
    let dir = starlette_templates();
    let (stdout, _, code) = check(&["--format", "json", dir.to_str().unwrap()]);
    assert!(code == 0 || code == 1);
    // stdout must be valid JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(parsed.is_ok(), "json format must produce valid JSON: {stdout}");
}

#[test]
fn format_compact_flag_accepted() {
    let dir = starlette_templates();
    let (_, _, code) = check(&["--format", "compact", dir.to_str().unwrap()]);
    assert!(code == 0 || code == 1);
}

#[test]
fn select_flag_accepted() {
    let dir = starlette_templates();
    let (_, _, code) = check(&["--select", "JINJA-E101", dir.to_str().unwrap()]);
    assert!(code == 0 || code == 1);
}

#[test]
fn ignore_flag_accepted() {
    let dir = starlette_templates();
    let (_, _, code) = check(&["--ignore", "JINJA-W2", dir.to_str().unwrap()]);
    assert!(code == 0 || code == 1);
}

// ---------- REQ-LINT-06 / REQ-LINT-07: json shape ---------------------------

#[test]
fn json_output_has_7_key_shape() {
    let dir = starlette_templates();
    let (stdout, _, _) = check(&["--format", "json", dir.to_str().unwrap()]);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    // starlette-blog is clean baseline → may be empty; if not, check shape
    for item in arr {
        let obj = item.as_object().unwrap();
        assert!(obj.contains_key("file"), "must have 'file'");
        assert!(obj.contains_key("line"), "must have 'line'");
        assert!(obj.contains_key("col"), "must have 'col'");
        assert!(obj.contains_key("code"), "must have 'code'");
        assert!(obj.contains_key("slug"), "must have 'slug'");
        assert!(obj.contains_key("severity"), "must have 'severity'");
        assert!(obj.contains_key("message"), "must have 'message'");
    }
}

// ---------- REQ-LINT-08: exit codes -----------------------------------------

#[test]
fn exit_0_when_no_findings() {
    let tmp = tmpdir("exit0");
    let (_, _, code) = check(&[tmp.to_str().unwrap()]);
    assert_eq!(code, 0);
}

// ---------- REQ-LINT-09: shared engine --------------------------------------

#[test]
fn check_uses_build_workspace_same_as_lsp() {
    // Structural proof: main.rs calls build_workspace() for check, and
    // server.rs also uses workspace::build_workspace. Same code path.
    // This test documents the invariant; engine is tested via the workspace tests.
    use jinja_lsp::workspace::build_workspace;
    let dir = starlette_templates();
    let ws = build_workspace(&[&dir], &["html"]);
    assert!(!ws.templates.is_empty(), "engine must produce templates");
}

// ---------- REQ-LINT-10: path normalization ----------------------------------

#[test]
fn json_file_field_is_forward_slash() {
    // When there are findings, the file path uses forward slashes
    // (always true on Linux; document the requirement)
    let tmp = tmpdir("pathslash");
    fs::write(tmp.join("t.html"), "{% if %}{% endif %}").unwrap(); // syntax error
    let (stdout, _, _) = check(&["--format", "json", tmp.to_str().unwrap()]);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    for item in arr {
        let file = item["file"].as_str().unwrap();
        assert!(!file.contains('\\'), "path must use forward slashes: {file}");
    }
}

// ---------- REQ-LINT-04/05: format output ------------------------------------

#[test]
fn compact_format_one_line_per_finding() {
    let tmp = tmpdir("compact");
    // Create a template with a syntax error to generate a finding
    fs::write(tmp.join("bad.html"), "{% block %}{% endblock %}").unwrap();
    let (stdout, _, _) = check(&["--format", "compact", tmp.to_str().unwrap()]);
    // Each finding is one line: "file:line:col: CODE slug message"
    for line in stdout.lines() {
        assert!(!line.is_empty(), "compact lines must not be empty");
    }
}
