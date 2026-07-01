use std::{
    fs,
    path::Path,
};

use super::index::WorkspaceIndex;
use crate::parsing::{discover_templates, extract};

/// Discover all templates in `templates_dirs` matching `extensions`, extract
/// each one, and return a populated `WorkspaceIndex` keyed by relative path
/// from the owning templates directory (e.g. `"blog/post.html"`).
/// Extends chains are resolved lazily via `WorkspaceIndex::template_chain()`.
pub fn build_workspace(templates_dirs: &[&Path], extensions: &[&str]) -> WorkspaceIndex {
    let paths = discover_templates(templates_dirs, extensions);
    let mut workspace = WorkspaceIndex::default();

    for abs_path in paths {
        if let Some(key) = relative_key(&abs_path, templates_dirs) {
            let source = fs::read_to_string(&abs_path).unwrap_or_default();
            let mut idx = extract(&source);
            idx.path = abs_path.to_string_lossy().to_string();
            workspace.templates.insert(key, idx);
        }
    }

    workspace
}

fn relative_key(abs_path: &Path, templates_dirs: &[&Path]) -> Option<String> {
    for dir in templates_dirs {
        if let Ok(rel) = abs_path.strip_prefix(dir) {
            return Some(rel.to_string_lossy().into_owned());
        }
    }
    None
}

/// Like `build_workspace` but keyed by absolute path, matching how the LSP server
/// identifies files via `uri.path()`.  Used during `initialize` so pre-indexed
/// templates are findable by the same key that `pass1`/`publish_file_diagnostics` use.
pub fn build_workspace_abs(templates_dirs: &[&Path], extensions: &[&str]) -> WorkspaceIndex {
    let paths = discover_templates(templates_dirs, extensions);
    let mut workspace = WorkspaceIndex::default();
    for abs_path in paths {
        let key = abs_path.to_string_lossy().into_owned();
        let source = fs::read_to_string(&abs_path).unwrap_or_default();
        let mut idx = extract(&source);
        idx.path = key.clone();
        workspace.templates.insert(key, idx);
    }
    workspace
}
