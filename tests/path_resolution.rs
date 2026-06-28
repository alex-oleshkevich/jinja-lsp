use std::path::Path;

use jinja_lsp::parsing::resolve_path;

fn tdir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/templates")
}

#[test]
fn resolves_valid_path_to_absolute() {
    let dir = tdir();
    let result = resolve_path("base.html", &[&dir]);
    assert!(result.is_some(), "expected Some for existing file");
    assert!(result.unwrap().is_absolute());
}

#[test]
fn resolves_nested_path() {
    let dir = tdir();
    let result = resolve_path("blog/post.html", &[&dir]);
    assert!(result.is_some(), "expected Some for nested file: {:?}", dir.join("blog/post.html"));
}

#[test]
fn rejects_parent_dir_traversal() {
    let dir = tdir();
    assert!(resolve_path("../secret.html", &[&dir]).is_none(), ".. at start must be rejected");
    assert!(resolve_path("sub/../../escape.html", &[&dir]).is_none(), ".. in middle must be rejected");
    assert!(resolve_path("a/../b/../../../etc/passwd", &[&dir]).is_none(), "deep traversal rejected");
}

#[test]
fn returns_none_for_nonexistent_file() {
    let dir = tdir();
    assert!(resolve_path("does_not_exist.html", &[&dir]).is_none());
}

#[test]
fn tries_multiple_dirs_in_order() {
    let dir = tdir();
    let other = Path::new("/nonexistent");
    // base.html exists in dir but not in other
    let result = resolve_path("base.html", &[other, &dir]);
    assert!(result.is_some(), "should find file in second dir");
}

#[test]
fn returns_none_when_no_dirs() {
    assert!(resolve_path("base.html", &[]).is_none());
}
