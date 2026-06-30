use std::process::Command;

fn jinja_lsp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jinja-lsp"))
}

#[test]
fn help_shows_lsp_check_format_subcommands() {
    let output = jinja_lsp_bin()
        .arg("--help")
        .output()
        .expect("failed to run jinja-lsp --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lsp"), "missing 'lsp' subcommand in --help output");
    assert!(stdout.contains("check"), "missing 'check' subcommand in --help output");
    assert!(stdout.contains("format"), "missing 'format' subcommand in --help output");
}

#[test]
fn lsp_subcommand_recognized() {
    let output = jinja_lsp_bin()
        .arg("lsp")
        .arg("--help")
        .output()
        .expect("failed to run jinja-lsp lsp --help");

    assert!(output.status.success());
}

#[test]
fn check_subcommand_recognized() {
    let output = jinja_lsp_bin()
        .arg("check")
        .arg("--help")
        .output()
        .expect("failed to run jinja-lsp check --help");

    assert!(output.status.success());
}

#[test]
fn format_subcommand_recognized() {
    let output = jinja_lsp_bin()
        .arg("format")
        .arg("--help")
        .output()
        .expect("failed to run jinja-lsp format --help");

    assert!(output.status.success());
}

#[test]
fn version_flag_prints_version_and_exits_0() {
    let output = jinja_lsp_bin()
        .arg("--version")
        .output()
        .expect("failed to run jinja-lsp --version");
    assert!(output.status.success(), "--version must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("jinja-lsp"), "--version must print the program name");
    // Cargo version is in Cargo.toml; verify it's numeric (e.g. "0.1.0")
    assert!(stdout.chars().any(|c| c.is_ascii_digit()), "--version must print a version number");
}
