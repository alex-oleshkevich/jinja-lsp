use std::{collections::HashMap, path::Path};

use crate::{
    parsing::extract,
    workspace::{build_workspace, index::WorkspaceIndex},
};

/// Shared mutable state held behind Arc<RwLock<>> in the LSP backend.
pub struct ServerState {
    pub workspace: WorkspaceIndex,
    /// Raw source text per file key — used by formatting handlers.
    pub sources: HashMap<String, String>,
    /// Incremented by every Pass 1; Pass 2 checks it to discard stale relinks.
    pub generation: u64,
}

impl ServerState {
    /// Build initial state by discovering all templates in `templates_dirs`.
    pub fn from_dirs(templates_dirs: &[&Path], extensions: &[&str]) -> Self {
        Self {
            workspace: build_workspace(templates_dirs, extensions),
            sources: HashMap::new(),
            generation: 0,
        }
    }

    /// Pass 1 (REQ-ARCH-03): re-extract one file and atomically replace its
    /// TemplateIndex without touching any other entry.
    pub fn update_file(&mut self, key: &str, source: &str) {
        let mut idx = extract(source);
        idx.path = key.to_owned();
        self.workspace.templates.insert(key.to_owned(), idx);
        self.sources.insert(key.to_owned(), source.to_owned());
        self.generation += 1;
    }
}
