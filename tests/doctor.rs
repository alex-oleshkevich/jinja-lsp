//! `jinja-lsp doctor` — what the server discovers here, and what it cannot find.
//!
//! The command exists to answer "why is jinja-lsp not seeing my templates", so
//! the cases that matter most are the ones where discovery silently finds
//! nothing. Every test asserts on a real invocation of the built binary.

use std::{fs, path::PathBuf, process::Command};

fn doctor_in(dir: &std::path::Path) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_jinja-lsp"))
        .arg("doctor")
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("run doctor");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("jinja_lsp_doctor").join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

// ─── Template detection ──────────────────────────────────────────────────────

#[test]
fn detects_zero_config_templates_dir() {
    // No config at all: `templates/` is one of the directories discovery looks
    // for, so it must be found and counted without anything being configured.
    let dir = scratch("zero_config");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();
    fs::write(dir.join("templates/b.html"), "{{ y }}").unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        out.contains("none — using zero-config defaults"),
        "must report that no config file was found:\n{out}"
    );
    assert!(
        out.contains("2 templates"),
        "must count both templates:\n{out}"
    );
    assert_eq!(code, 0, "a healthy project exits 0:\n{out}");
}

#[test]
fn detects_configured_templates_dir() {
    let dir = scratch("configured");
    fs::create_dir_all(dir.join("views")).unwrap();
    fs::write(dir.join("views/a.html"), "{{ x }}").unwrap();
    fs::write(dir.join("jinja.toml"), "templates = [\"views\"]\n").unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(out.contains("jinja.toml"), "must name the config:\n{out}");
    assert!(
        out.contains("views") && out.contains("1 template"),
        "must report the configured dir and its count:\n{out}"
    );
    assert_eq!(code, 0);
}

#[test]
fn reports_a_configured_directory_that_does_not_exist() {
    // The whole point of the command. `resolved_template_dirs` drops entries
    // that are not directories, which is right for indexing but would make a
    // typo'd path invisible in the one place built to surface it.
    let dir = scratch("missing_dir");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();
    fs::write(
        dir.join("jinja.toml"),
        "templates = [\"templates\", \"typoed_dir\"]\n",
    )
    .unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        out.contains("typoed_dir") && out.contains("directory not found"),
        "a configured-but-absent directory must be reported:\n{out}"
    );
    assert_eq!(code, 1, "problems exit 1:\n{out}");
}

#[test]
fn reports_a_directory_that_contains_no_matching_files() {
    // Present but empty is a different failure from absent, and the fix is
    // different too: usually the extension list, not the path.
    let dir = scratch("no_matches");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/notes.txt"), "not a template").unwrap();
    fs::write(dir.join("jinja.toml"), "templates = [\"templates\"]\n").unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        out.contains("no matching files"),
        "must distinguish an empty dir from a missing one:\n{out}"
    );
    assert!(
        out.contains("html"),
        "must name the extensions it looked for, since that is usually the fix:\n{out}"
    );
    assert_eq!(code, 1);
}

#[test]
fn expands_the_ellipsis_to_the_auto_discovered_dirs() {
    // `templates = ["custom", "..."]` keeps the defaults; doctor must show what
    // the ellipsis actually expanded to rather than printing "...".
    let dir = scratch("ellipsis");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("custom")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();
    fs::write(dir.join("custom/b.html"), "{{ y }}").unwrap();
    fs::write(
        dir.join("jinja.toml"),
        "templates = [\"custom\", \"...\"]\n",
    )
    .unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        !out.contains("\"...\""),
        "must not echo the literal ellipsis:\n{out}"
    );
    assert!(
        out.contains("custom") && out.contains("templates"),
        "both the explicit dir and the expanded default must appear:\n{out}"
    );
    assert_eq!(code, 0);
}

#[test]
fn custom_extensions_change_what_counts_as_a_template() {
    let dir = scratch("extensions");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.tpl"), "{{ x }}").unwrap();
    fs::write(
        dir.join("jinja.toml"),
        "templates = [\"templates\"]\nextensions = [\"tpl\"]\n",
    )
    .unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        out.contains("1 template"),
        ".tpl must count once configured:\n{out}"
    );
    assert!(
        out.contains("tpl"),
        "must report the extension list:\n{out}"
    );
    assert_eq!(code, 0);
}

// ─── Addons ──────────────────────────────────────────────────────────────────

#[test]
fn reports_sidecar_hint_files() {
    // Sidecars are discovered per template rather than configured, so they are
    // the least visible source of builtins and the easiest to get wrong.
    let dir = scratch("sidecar");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();
    fs::write(
        dir.join("templates/a.html.hints.md"),
        "# Context\n\n## x\n\nA thing.\n",
    )
    .unwrap();

    let (out, _) = doctor_in(&dir);
    assert!(
        out.contains("sidecars") && out.contains("1 *.hints.md file"),
        "a sidecar beside a template must be reported:\n{out}"
    );
}

#[test]
fn reports_an_unknown_extras_pack() {
    let dir = scratch("bad_pack");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();
    fs::write(dir.join("jinja.toml"), "extras = [\"nope\"]\n").unwrap();

    let (out, code) = doctor_in(&dir);
    assert!(
        out.contains("nope"),
        "the unusable pack must be named:\n{out}"
    );
    assert_eq!(code, 1);
}

#[test]
fn stays_quiet_about_what_is_not_configured() {
    // "Don't add noise": a project with no extras, hints, custom builtins or
    // lint filters must not get empty sections announcing their absence.
    let dir = scratch("quiet");
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::write(dir.join("templates/a.html"), "{{ x }}").unwrap();

    let (out, _) = doctor_in(&dir);
    assert!(
        !out.contains("Lint"),
        "no lint filters set, no Lint section:\n{out}"
    );
    assert!(
        !out.contains("sidecars"),
        "no sidecars, no sidecar row:\n{out}"
    );
    assert!(
        out.lines().count() < 16,
        "a clean project's report should stay short; got {} lines:\n{out}",
        out.lines().count()
    );
}

#[test]
fn unreadable_config_exits_two_and_says_why() {
    let dir = scratch("bad_config");
    fs::write(dir.join("jinja.toml"), "templates = [ this is not toml\n").unwrap();

    let (out, code) = doctor_in(&dir);
    assert_eq!(code, 2, "a config that cannot be read exits 2:\n{out}");
    assert!(out.contains("config error"), "must say what failed:\n{out}");
}
