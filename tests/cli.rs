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
