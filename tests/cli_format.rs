// REQ-FMT-08 / REQ-FMT-09: `jinja-lsp format` CLI integration tests.
// 8nyl: --check must print "would reformat: path" per file and a summary line.

use std::{fs, process::Command};

fn jinja_lsp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jinja-lsp"))
}

fn scratchpad() -> std::path::PathBuf {
    let d = std::env::temp_dir().join("jinja_lsp_fmt_test");
    fs::create_dir_all(&d).ok();
    d
}

// ─── T-01: in-place format rewrites unformatted file ─────────────────────────

#[test]
fn fmt08_t01_inplace_rewrites_file() {
    let dir = scratchpad().join("t01");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let status = jinja_lsp_bin()
        .arg("format")
        .arg(&path)
        .status()
        .expect("run format");

    // REQ-FMT-09: exit 1 when files changed
    assert_eq!(status.code().unwrap(), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), "{{ x }}\n");
}

// ─── T-02: in-place format exits 0 when file already formatted ───────────────

#[test]
fn fmt08_t02_inplace_noop_exits_zero() {
    let dir = scratchpad().join("t02");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{ x }}\n").unwrap();

    let status = jinja_lsp_bin()
        .arg("format")
        .arg(&path)
        .status()
        .expect("run format");

    assert_eq!(status.code().unwrap(), 0);
}

// ─── T-03: --check exits 1 but does not rewrite ──────────────────────────────

#[test]
fn fmt08_t03_check_mode_no_rewrite() {
    let dir = scratchpad().join("t03");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let status = jinja_lsp_bin()
        .arg("format")
        .arg("--check")
        .arg(&path)
        .status()
        .expect("run format --check");

    assert_eq!(status.code().unwrap(), 1);
    // File must NOT have been modified.
    assert_eq!(fs::read_to_string(&path).unwrap(), "{{x}}\n");
}

// ─── T-04: --check exits 0 when already formatted ────────────────────────────

#[test]
fn fmt08_t04_check_mode_already_formatted() {
    let dir = scratchpad().join("t04");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{ x }}\n").unwrap();

    let status = jinja_lsp_bin()
        .arg("format")
        .arg("--check")
        .arg(&path)
        .status()
        .expect("run format --check");

    assert_eq!(status.code().unwrap(), 0);
}

// ─── T-05: --diff prints diff but does not rewrite ───────────────────────────

#[test]
fn fmt08_t05_diff_mode_no_rewrite() {
    let dir = scratchpad().join("t05");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let out = jinja_lsp_bin()
        .arg("format")
        .arg("--diff")
        .arg(&path)
        .output()
        .expect("run format --diff");

    assert_eq!(out.status.code().unwrap(), 1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("{{x}}") || stdout.contains("{{ x }}"),
        "diff output should show change; got: {stdout}");
    // File must NOT have been rewritten.
    assert_eq!(fs::read_to_string(&path).unwrap(), "{{x}}\n");
}

// ─── T-06: directory argument formats all .html/.jinja files ─────────────────

#[test]
fn fmt08_t06_directory_formats_all_templates() {
    let dir = scratchpad().join("t06");
    fs::create_dir_all(&dir).ok();
    fs::write(dir.join("a.html"), "{{a}}\n").unwrap();
    fs::write(dir.join("b.jinja"), "{{b}}\n").unwrap();
    fs::write(dir.join("readme.txt"), "{{c}}\n").unwrap(); // not a template ext

    let status = jinja_lsp_bin()
        .arg("format")
        .arg(&dir)
        .status()
        .expect("run format on directory");

    assert_eq!(status.code().unwrap(), 1);
    assert_eq!(fs::read_to_string(dir.join("a.html")).unwrap(), "{{ a }}\n");
    assert_eq!(fs::read_to_string(dir.join("b.jinja")).unwrap(), "{{ b }}\n");
    // .txt file should NOT have been touched
    assert_eq!(fs::read_to_string(dir.join("readme.txt")).unwrap(), "{{c}}\n");
}

// ─── 8nyl: --check prints per-file message and summary ───────────────────────

#[test]
fn fmt08_check_prints_would_reformat_per_file() {
    let dir = scratchpad().join("8nyl_check");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let out = jinja_lsp_bin()
        .arg("format")
        .arg("--check")
        .arg(&path)
        .output()
        .expect("run format --check");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would reformat:"), "--check must print 'would reformat: path': {stdout}");
    assert!(stdout.contains("1 file would be reformatted"), "--check must print summary: {stdout}");
}

#[test]
fn fmt08_check_summary_counts_unchanged() {
    let dir = scratchpad().join("8nyl_unchanged");
    fs::create_dir_all(&dir).ok();
    fs::write(dir.join("changed.html"), "{{x}}\n").unwrap();
    fs::write(dir.join("ok.html"), "{{ y }}\n").unwrap();

    let out = jinja_lsp_bin()
        .arg("format")
        .arg("--check")
        .arg(&dir)
        .output()
        .expect("run format --check dir");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 unchanged"), "--check summary must count unchanged: {stdout}");
    assert!(stdout.contains("1 file would be reformatted"), "--check must show changed count: {stdout}");
}

#[test]
fn fmt08_diff_prints_summary_line() {
    let dir = scratchpad().join("8nyl_diff");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let out = jinja_lsp_bin()
        .arg("format")
        .arg("--diff")
        .arg(&path)
        .output()
        .expect("run format --diff");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 file would be reformatted."), "--diff must print summary: {stdout}");
}

// ─── T-08: --output - writes formatted content to stdout ─────────────────────

#[test]
fn fmt08_t08_output_stdout() {
    let dir = scratchpad().join("t08");
    fs::create_dir_all(&dir).ok();
    let path = dir.join("tpl.html");
    fs::write(&path, "{{x}}\n").unwrap();

    let out = jinja_lsp_bin()
        .args(["format", "--output", "-"])
        .arg(&path)
        .output()
        .expect("run format --output -");

    assert_eq!(out.status.code().unwrap(), 1, "exit 1 when file would change");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "{{ x }}\n");
    // original file must be untouched
    assert_eq!(fs::read_to_string(&path).unwrap(), "{{x}}\n");
}

// ─── T-09: --output FILE writes to named path, not in-place ──────────────────

#[test]
fn fmt08_t09_output_named_file() {
    let dir = scratchpad().join("t09");
    fs::create_dir_all(&dir).ok();
    let src = dir.join("tpl.html");
    let dst = dir.join("out.html");
    fs::write(&src, "{{x}}\n").unwrap();

    let out = jinja_lsp_bin()
        .args(["format", "--output"])
        .arg(&dst)
        .arg(&src)
        .output()
        .expect("run format --output FILE");

    assert_eq!(out.status.code().unwrap(), 1);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "{{ x }}\n");
    // source must be untouched
    assert_eq!(fs::read_to_string(&src).unwrap(), "{{x}}\n");
}

// ─── T-10: --output FILE with multiple inputs is an error ────────────────────

#[test]
fn fmt08_t10_output_file_multiple_inputs_is_error() {
    let dir = scratchpad().join("t10");
    fs::create_dir_all(&dir).ok();
    let a = dir.join("a.html");
    let b = dir.join("b.html");
    fs::write(&a, "{{x}}\n").unwrap();
    fs::write(&b, "{{y}}\n").unwrap();

    let out = jinja_lsp_bin()
        .args(["format", "--output", "out.html"])
        .arg(&a)
        .arg(&b)
        .output()
        .expect("run format");

    assert_eq!(out.status.code().unwrap(), 2, "exit 2 for --output FILE with multiple inputs");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--output FILE requires a single input file"), "{stderr}");
}
