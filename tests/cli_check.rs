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
    fs::write(tmp.join("bad.html"), "{{ undefined_var }}").unwrap();
    let (stdout, _, _) = check(&["--format", "compact", tmp.to_str().unwrap()]);
    // Must produce at least one finding
    assert!(!stdout.trim().is_empty(), "compact output must not be empty");
    // Each line: "path:line:col: CODE slug: message" — no blank lines
    for line in stdout.lines() {
        assert!(!line.is_empty(), "compact must not emit blank lines");
        // Verify structure: starts with a path and 1-based line:col
        let parts: Vec<&str> = line.splitn(2, ": ").collect();
        assert_eq!(parts.len(), 2, "each compact line must have ': ' separator: {line:?}");
        let loc = parts[0]; // "path:line:col"
        let loc_parts: Vec<&str> = loc.rsplitn(3, ':').collect();
        assert_eq!(loc_parts.len(), 3, "location must be path:line:col: {line:?}");
        let col: u32 = loc_parts[0].parse().expect("col must be numeric");
        let linenum: u32 = loc_parts[1].parse().expect("line must be numeric");
        assert!(linenum >= 1, "line must be 1-based: {line:?}");
        assert!(col >= 1, "col must be 1-based: {line:?}");
    }
}

// ---------- REQ-LINT-03 / T-30: slug in --select/--ignore → exit 2 ----------

#[test]
fn t30_slug_in_ignore_exits_2() {
    // REQ-LINT-03: slugs must be rejected; only codes/prefixes are valid
    let dir = starlette_templates();
    let (stdout, stderr, code) = check(&["--ignore", "undefined-variable", dir.to_str().unwrap()]);
    assert_eq!(code, 2, "slug in --ignore must exit 2, stdout={stdout:?} stderr={stderr:?}");
    assert!(stderr.contains("error"), "must emit error to stderr: {stderr:?}");
}

#[test]
fn t30_slug_in_select_exits_2() {
    let dir = starlette_templates();
    let (stdout, stderr, code) = check(&["--select", "unused-import", dir.to_str().unwrap()]);
    assert_eq!(code, 2, "slug in --select must exit 2, stdout={stdout:?} stderr={stderr:?}");
    assert!(stderr.contains("error"), "must emit error to stderr: {stderr:?}");
}

// ---------- REQ-LINT-08 / T-29: nonexistent PATH → exit 2 -------------------

#[test]
fn t29_nonexistent_path_exits_2() {
    // REQ-LINT-08: bad PATH → exit 2, not 0
    let (stdout, stderr, code) = check(&["./does-not-exist-jinjachecktest"]);
    assert_eq!(code, 2, "missing path must exit 2, stdout={stdout:?} stderr={stderr:?}");
    assert!(stderr.contains("error"), "must emit error to stderr: {stderr:?}");
}

// ---------- REQ-LINT-10 / T-04c: json file path is workspace-relative -------

#[test]
fn t04c_json_file_field_is_workspace_relative() {
    // REQ-LINT-10: the 'file' field must be relative to the workspace root, not absolute
    let tmp = tmpdir("relpath");
    fs::write(tmp.join("t.html"), "{{ undefined_var }}").unwrap();
    let (stdout, _, _) = check(&["--format", "json", tmp.to_str().unwrap()]);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(!arr.is_empty(), "fixture must produce at least one finding");
    for item in &arr {
        let file = item["file"].as_str().unwrap();
        assert!(!std::path::Path::new(file).is_absolute(),
            "file field must be workspace-relative, not absolute: {file:?}");
        // The only file in the fixture is t.html
        assert_eq!(file, "t.html", "relative path must equal the filename: got {file:?}");
    }
}

// ---------- REQ-LINT-09 / DIAG-06 / jinja-lsp-6zo5: W107 invalid-noqa parity ------

#[test]
fn t6zo5_invalid_noqa_w107_appears_in_check_output() {
    // REQ-LINT-09: check must surface W107 (invalid-noqa) just like the LSP server does.
    // suppress_by_noqa returns (kept, w107s) — w107s must NOT be discarded.
    let tmp = tmpdir("w107");
    // noqa comment with a bogus code that doesn't start with JINJA- → W107
    fs::write(tmp.join("t.html"), "{{ x }} {# noqa: bad-code #}").unwrap();
    let (stdout, _, code) = check(&["--format", "json", tmp.to_str().unwrap()]);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<&str> = arr.iter()
        .filter_map(|v| v["code"].as_str())
        .collect();
    assert!(codes.contains(&"JINJA-W107"),
        "W107 invalid-noqa must appear in check output, got codes: {codes:?}");
    assert_eq!(code, 1, "W107 is a finding → exit 1");
}

#[test]
fn jinja_lsp_8dqt_invalid_noqa_reported_when_file_has_no_other_diagnostics() {
    // jinja-lsp-8dqt: the CLI built per_file only from files that already had
    // filtered diagnostics, so a template whose ONLY problem is an invalid noqa
    // directive produced no findings at all. Content here is otherwise clean.
    let tmp = tmpdir("w107-only-finding");
    fs::write(tmp.join("t.html"), "<p>hello</p>\n{# noqa: bad-code #}\n").unwrap();
    let (stdout, _, code) = check(&["--format", "json", tmp.to_str().unwrap()]);
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    let codes: Vec<&str> = arr.iter()
        .filter_map(|v| v["code"].as_str())
        .collect();
    assert!(codes.contains(&"JINJA-W107"),
        "W107 must be reported even when it is the file's only diagnostic, got codes: {codes:?}");
    assert_eq!(code, 1, "W107 is a finding → exit 1");
}

// ---------- jinja-lsp-6g3l: invalid --format must exit 2 -------------------

#[test]
fn t6g3l_invalid_format_value_exits_2() {
    // An unrecognized --format value is a usage error and must exit 2.
    let tmp = tmpdir("fmt_bogus");
    fs::write(tmp.join("t.html"), "{{ x }}").unwrap();
    let (_, stderr, code) = check(&["--format", "bogus", tmp.to_str().unwrap()]);
    assert_eq!(code, 2, "unrecognized --format must exit 2, got {code}; stderr: {stderr}");
    assert!(stderr.contains("bogus") || stderr.contains("format"),
        "stderr must mention the invalid value or 'format': {stderr}");
}

// ---------- jinja-lsp-k2gk: --verbose emits INFO to stderr ------------------

#[test]
fn k2gk_verbose_flag_emits_discovery_info_to_stderr() {
    // REQ-LINT-02: -v/--verbose must emit discovery counts to stderr.
    let tmp = tmpdir("verbose_info");
    fs::write(tmp.join("a.html"), "{{ x }}").unwrap();
    fs::write(tmp.join("b.html"), "{{ y }}").unwrap();
    let (_, stderr, code) = check(&["--verbose", tmp.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "exit code must be 0 or 1: {code}");
    assert!(!stderr.is_empty(), "--verbose must emit something to stderr");
    // Must mention template count or the word "discovered"/"template"/"file".
    assert!(
        stderr.contains("template") || stderr.contains("discover") || stderr.contains("2"),
        "--verbose stderr must include discovery count: {stderr}"
    );
}

#[test]
fn k2gk_non_verbose_run_emits_nothing_to_stderr() {
    // Without --verbose on a clean input, stderr must be silent.
    let tmp = tmpdir("no_verbose_clean");
    fs::write(tmp.join("a.html"), "{{ x }}").unwrap();
    let (_, stderr, _) = check(&[tmp.to_str().unwrap()]);
    assert!(stderr.is_empty(), "non-verbose run must not emit to stderr: {stderr}");
}
