// E29 Branch A: REQ-E2E-03, REQ-E2E-04 — golden-file check.
//
// Invokes `jinja-lsp check --format json` against each registered fixture
// and diffs the output against `expected-diagnostics.json`.
// Run with UPDATE_FIXTURES=1 to regenerate golden files.

use std::{
    env, fs,
    path::Path,
    process::Command,
};

use jinja_lsp::diagnostic::Diagnostic;

fn binary_path() -> std::path::PathBuf {
    // Find the built binary (works in both `cargo test` and `cargo nextest`)
    let mut path = env::current_exe().unwrap();
    // current_exe is e2e_branch_a-<hash>; binary is a few dirs up in debug/
    loop {
        path.pop();
        let candidate = path.join("jinja-lsp");
        if candidate.is_file() {
            return candidate;
        }
        if path.parent().is_none() {
            break;
        }
    }
    // Fallback: build path relative to CARGO_MANIFEST_DIR
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/jinja-lsp")
}

fn fixture_dir(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_check(fixture: &str) -> Vec<Diagnostic> {
    let bin = binary_path();
    let dir = fixture_dir(fixture);

    let output = Command::new(&bin)
        .args(["check", "--format", "json", dir.to_str().unwrap()])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("check output is not valid JSON: {e}\nraw: {stdout}"))
}

fn golden_file(fixture: &str) -> std::path::PathBuf {
    fixture_dir(fixture).join("expected-diagnostics.json")
}

fn check_or_update(fixture: &str) {
    let actual = run_check(fixture);
    let golden_path = golden_file(fixture);

    if env::var("UPDATE_FIXTURES").is_ok() {
        let json = serde_json::to_string_pretty(&actual).unwrap();
        fs::write(&golden_path, format!("{json}\n")).unwrap();
        return;
    }

    let expected_raw = fs::read_to_string(&golden_path)
        .unwrap_or_else(|_| panic!("golden file missing: {}", golden_path.display()));
    let expected: Vec<Diagnostic> = serde_json::from_str(&expected_raw)
        .unwrap_or_else(|e| panic!("golden file is invalid JSON: {e}"));

    if actual != expected {
        let actual_json = serde_json::to_string_pretty(&actual).unwrap();
        let expected_json = serde_json::to_string_pretty(&expected).unwrap();
        panic!(
            "Branch A golden mismatch for '{fixture}'\n\
             expected:\n{expected_json}\n\
             actual:\n{actual_json}\n\
             Re-run with UPDATE_FIXTURES=1 to accept the new output."
        );
    }
}

// ---------- REQ-E2E-03 / REQ-E2E-04 ----------------------------------------

#[test]
fn starlette_blog_matches_golden() {
    // REQ-E2E-03: check --format json diffed against golden file.
    // REQ-E2E-04: UPDATE_FIXTURES=1 regenerates the golden file.
    check_or_update("starlette-blog");
}
