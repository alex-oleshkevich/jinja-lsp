// REQ-FMT-E2E: file-based formatter fixture tests.
// Each tests/fixtures/formatter/*.input file is formatted and compared against
// the matching *.expected file.

use std::{fs, path::PathBuf};

use jinja_lsp::format::format;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formatter")
}

#[test]
fn formatter_fixtures_match_expected() {
    let dir = fixture_dir();
    let mut inputs: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("fixtures/formatter directory must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "input").unwrap_or(false))
        .collect();
    inputs.sort();

    assert!(!inputs.is_empty(), "no .input fixtures found in {dir:?}");

    let mut failures: Vec<String> = Vec::new();

    for input_path in &inputs {
        let name = input_path.file_stem().unwrap().to_string_lossy();
        let expected_path = dir.join(format!("{name}.expected"));

        if !expected_path.exists() {
            failures.push(format!("[{name}] missing expected file: {expected_path:?}"));
            continue;
        }

        let source = fs::read_to_string(input_path)
            .unwrap_or_else(|e| panic!("cannot read {input_path:?}: {e}"));
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("cannot read {expected_path:?}: {e}"));

        let actual = format(&source);

        if actual != expected {
            failures.push(format!(
                "[{name}] output mismatch\n  expected: {expected:?}\n  actual:   {actual:?}"
            ));
        }
    }

    if !failures.is_empty() {
        panic!("{} fixture(s) failed:\n{}", failures.len(), failures.join("\n\n"));
    }
}
