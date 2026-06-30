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
    // Pass the templates/ subdirectory so workspace keys are relative to the
    // template root (e.g. "base.html") and match the paths used in extends/import.
    let dir = fixture_dir(fixture).join("templates");

    let output = Command::new(&bin)
        .args(["check", "--format", "json", dir.to_str().unwrap()])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: Vec<Diagnostic> = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("check output is not valid JSON: {e}\nraw: {stdout}"));

    // Normalize absolute file paths to paths relative to the templates dir so
    // golden files are portable across machines.
    let dir_prefix = dir.to_string_lossy().into_owned() + "/";
    raw.into_iter()
        .map(|mut d| {
            if d.file.starts_with(&dir_prefix) {
                d.file = d.file[dir_prefix.len()..].to_owned();
            }
            d
        })
        .collect()
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

/// Like `check_or_update` but for corpus fixtures where templates live at the
/// fixture root (not a `templates/` subdirectory).
fn check_or_update_corpus(code: &str) {
    let bin = binary_path();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/corpus")
        .join(code);

    let output = Command::new(&bin)
        .args(["check", "--format", "json", dir.to_str().unwrap()])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw_actual: Vec<Diagnostic> = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("check output is not valid JSON for corpus/{code}: {e}\nraw: {stdout}"));

    // Normalize absolute file paths to paths relative to the fixture dir so
    // golden files are portable across machines.
    let dir_prefix = dir.to_string_lossy().into_owned() + "/";
    let normalize = |mut d: Diagnostic| -> Diagnostic {
        if d.file.starts_with(&dir_prefix) {
            d.file = d.file[dir_prefix.len()..].to_owned();
        }
        d
    };
    let actual: Vec<Diagnostic> = raw_actual.into_iter().map(normalize).collect();

    let golden_path = dir.join("expected-diagnostics.json");

    if env::var("UPDATE_FIXTURES").is_ok() {
        let json = serde_json::to_string_pretty(&actual).unwrap();
        fs::write(&golden_path, format!("{json}\n")).unwrap();
        return;
    }

    let expected_raw = fs::read_to_string(&golden_path)
        .unwrap_or_else(|_| panic!("golden file missing: {}", golden_path.display()));
    let expected: Vec<Diagnostic> = serde_json::from_str(&expected_raw)
        .unwrap_or_else(|e| panic!("golden file invalid JSON for corpus/{code}: {e}"));

    assert_eq!(
        actual, expected,
        "corpus/{code} mismatch. Re-run with UPDATE_FIXTURES=1 to accept.\nactual:\n{}\nexpected:\n{}",
        serde_json::to_string_pretty(&actual).unwrap(),
        serde_json::to_string_pretty(&expected).unwrap(),
    );
}

// ---------- REQ-E2E-03 / REQ-E2E-04 ----------------------------------------

#[test]
fn starlette_blog_matches_golden() {
    // REQ-E2E-03: check --format json diffed against golden file.
    // REQ-E2E-04: UPDATE_FIXTURES=1 regenerates the golden file.
    check_or_update("starlette-blog");
}

// ---------- REQ-TEST-02: per-code corpus fixtures ────────────────────────────

#[test] fn corpus_e001() { check_or_update_corpus("e001"); }
#[test] fn corpus_e101() { check_or_update_corpus("e101"); }
#[test] fn corpus_e102() { check_or_update_corpus("e102"); }
#[test] fn corpus_e103() { check_or_update_corpus("e103"); }
#[test] fn corpus_e104() { check_or_update_corpus("e104"); }
#[test] fn corpus_w106() { check_or_update_corpus("w106"); }
#[test] fn corpus_w201() { check_or_update_corpus("w201"); }
#[test] fn corpus_w202() { check_or_update_corpus("w202"); }
#[test] fn corpus_w203() { check_or_update_corpus("w203"); }
#[test] fn corpus_w301() { check_or_update_corpus("w301"); }
#[test] fn corpus_w302() { check_or_update_corpus("w302"); }
#[test] fn corpus_w303() { check_or_update_corpus("w303"); }
#[test] fn corpus_w304() { check_or_update_corpus("w304"); }
#[test] fn corpus_w305() { check_or_update_corpus("w305"); }
#[test] fn corpus_e401() { check_or_update_corpus("e401"); }
#[test] fn corpus_w402() { check_or_update_corpus("w402"); }
#[test] fn corpus_e403() { check_or_update_corpus("e403"); }
#[test] fn corpus_e404() { check_or_update_corpus("e404"); }
#[test] fn corpus_e501() { check_or_update_corpus("e501"); }
#[test] fn corpus_e601() { check_or_update_corpus("e601"); }

// ---------- REQ-TEST-01/02: named fixture corpus (jinja-lsp-r5o7) ───────────

/// Parse-recovery fixtures — triggers E001 on intentionally broken templates.
#[test] fn fixture_syntax_errors() { check_or_update("syntax-errors"); }
/// Template-chain fixture — valid inheritance chain; golden file is empty.
#[test] fn fixture_inheritance() { check_or_update("inheritance"); }
/// Macro-call and template-path fixture — triggers E501, E601.
#[test] fn fixture_call_and_paths() { check_or_update("call-and-paths"); }
/// Config-reload fixture — valid templates with starlette extras; golden file is empty.
#[test] fn fixture_config_reload() { check_or_update("config-reload"); }
