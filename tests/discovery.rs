use std::path::Path;

use jinja_lsp::parsing::discover_templates;

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
    assert_eq!(found.len(), 1, "symlink cycle must not produce duplicates: {found:?}");
    assert_eq!(found[0].file_name(), Some(std::ffi::OsStr::new("page.html")));
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
    assert!(n.contains(&"base.html".to_owned()), "base.html missing: {n:?}");
    assert!(n.contains(&"post.html".to_owned()), "post.html missing: {n:?}");
    assert!(n.contains(&"list.html".to_owned()), "list.html missing: {n:?}");
}

#[test]
fn filters_by_extension() {
    let dir = tdir();
    let found = discover_templates(&[&dir], &["jinja"]);
    let n = names(&found);
    // macros.jinja should appear
    assert!(n.contains(&"macros.jinja".to_owned()), "macros.jinja missing: {n:?}");
    // html files must not appear
    assert!(!n.contains(&"base.html".to_owned()), "base.html should be excluded: {n:?}");
}

#[test]
fn multiple_extensions() {
    let dir = tdir();
    let found = discover_templates(&[&dir], &["html", "jinja"]);
    let n = names(&found);
    assert!(n.contains(&"base.html".to_owned()));
    assert!(n.contains(&"macros.jinja".to_owned()));
    assert!(!n.contains(&"styles.css".to_owned()), "css must be excluded: {n:?}");
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
