use std::path::{Component, Path, PathBuf};

/// Resolve `path` relative to each entry of `templates_dirs`, returning the
/// first match that exists on disk. Returns `None` if:
/// - `path` contains any `..` component (REQ-EXTR-07 traversal defence), or
/// - no templates dir contains a real file at the resolved path.
pub fn resolve_path(path: &str, templates_dirs: &[&Path]) -> Option<PathBuf> {
    if Path::new(path).components().any(|c| c == Component::ParentDir) {
        return None;
    }
    for dir in templates_dirs {
        let candidate = dir.join(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
