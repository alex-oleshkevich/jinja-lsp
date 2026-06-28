use std::{
    fs,
    path::{Path, PathBuf},
};

/// Scan each directory in `templates_dirs` recursively for files whose extension
/// matches one of `extensions`. Non-existent directories are silently skipped.
pub fn discover_templates(templates_dirs: &[&Path], extensions: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in templates_dirs {
        walk(dir, extensions, &mut found);
    }
    found
}

fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, extensions, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if extensions.contains(&ext) {
                out.push(path);
            }
        }
    }
}
