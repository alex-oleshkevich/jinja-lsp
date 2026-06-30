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
        // file_type() does NOT follow symlinks — prevents symlink-cycle recursion.
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk(&entry.path(), extensions, out);
        } else if ft.is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if extensions.iter().any(|e| *e == ext_lower) {
                    out.push(path);
                }
            }
        }
        // symlinks (ft.is_symlink()) are intentionally skipped
    }
}
