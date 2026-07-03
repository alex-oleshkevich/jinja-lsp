use std::path::Path;

use jinja_lsp::parsing::discover_templates;
use jinja_lsp::workspace::build_workspace_abs;

#[test]
#[cfg(unix)]
fn e81b_symlink_cycle_does_not_loop() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::TempDir::new().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("page.html"), "content").unwrap();
    // 'sub/loop' → tmp root: a symlink cycle
    symlink(tmp.path(), sub.join("loop")).unwrap();
    // Symlink dirs must not be recursed: exactly one result, no duplicates.
    let found = discover_templates(&[tmp.path()], &["html"]);
    assert_eq!(
        found.len(),
        1,
        "symlink cycle must not produce duplicates: {found:?}"
    );
    assert_eq!(
        found[0].file_name(),
        Some(std::ffi::OsStr::new("page.html"))
    );
}

fn tdir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/templates")
}

fn names(paths: &[std::path::PathBuf]) -> Vec<String> {
    let mut v: Vec<String> = paths
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_owned()))
        .collect();
    v.sort();
    v
}

#[test]
fn discovers_html_files_recursively() {
    let dir = tdir();
    let found = discover_templates(&[&dir], &["html"]);
    let n = names(&found);
    assert!(
        n.contains(&"base.html".to_owned()),
        "base.html missing: {n:?}"
    );
    assert!(
        n.contains(&"post.html".to_owned()),
        "post.html missing: {n:?}"
    );
    assert!(
        n.contains(&"list.html".to_owned()),
        "list.html missing: {n:?}"
    );
}

#[test]
fn filters_by_extension() {
    let dir = tdir();
    let found = discover_templates(&[&dir], &["jinja"]);
    let n = names(&found);
    // macros.jinja should appear
    assert!(
        n.contains(&"macros.jinja".to_owned()),
        "macros.jinja missing: {n:?}"
    );
    // html files must not appear
    assert!(
        !n.contains(&"base.html".to_owned()),
        "base.html should be excluded: {n:?}"
    );
}

#[test]
fn multiple_extensions() {
    let dir = tdir();
    let found = discover_templates(&[&dir], &["html", "jinja"]);
    let n = names(&found);
    assert!(n.contains(&"base.html".to_owned()));
    assert!(n.contains(&"macros.jinja".to_owned()));
    assert!(
        !n.contains(&"styles.css".to_owned()),
        "css must be excluded: {n:?}"
    );
}

#[test]
fn skips_nonexistent_dirs_silently() {
    let nonexistent = Path::new("/definitely/does/not/exist");
    let found = discover_templates(&[nonexistent], &["html"]);
    assert!(found.is_empty());
}

#[test]
fn empty_dirs_returns_empty() {
    let found = discover_templates(&[], &["html"]);
    assert!(found.is_empty());
}

// ─── jinja-lsp-uoyh: build_workspace_abs keys are absolute paths ─────────────

#[test]
fn build_workspace_abs_uses_absolute_path_keys() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("page.html"), "{{ title }}").unwrap();

    let ws = build_workspace_abs(&[tmp.path()], &["html"]);
    let expected_key = tmp.path().join("page.html").to_string_lossy().to_string();

    assert!(
        ws.templates.contains_key(&expected_key),
        "workspace must use absolute path as key; expected {expected_key}, got keys: {:?}",
        ws.templates.keys().collect::<Vec<_>>()
    );
}

#[test]
fn build_workspace_abs_populates_template_index() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("macro.html"),
        "{% macro greet(name) %}hi{% endmacro %}",
    )
    .unwrap();

    let ws = build_workspace_abs(&[tmp.path()], &["html"]);
    let key = tmp.path().join("macro.html").to_string_lossy().to_string();

    let idx = ws.templates.get(&key).expect("template index must exist");
    assert!(
        !idx.macros.is_empty(),
        "macro must be extracted into template index"
    );
    assert_eq!(idx.macros[0].name, "greet");
}

// ─── jwfs: uppercase extensions must be discovered ───────────────────────────

#[test]
fn jwfs_uppercase_extension_is_discovered() {
    use std::fs;
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("index.HTML"), "content").unwrap();
    // extensions list uses lowercase "html"
    let found = discover_templates(&[tmp.path()], &["html"]);
    assert_eq!(
        found.len(),
        1,
        "file with uppercase .HTML extension must be discovered when 'html' is in the list"
    );
}
