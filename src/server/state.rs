use std::{collections::HashMap, path::Path};

use crate::{
    config::{ConfigOverlay, JinjaConfig},
    parsing::extract,
    workspace::{build_workspace, index::WorkspaceIndex},
};

/// Shared mutable state held behind Arc<RwLock<>> in the LSP backend.
pub struct ServerState {
    pub workspace: WorkspaceIndex,
    /// Active config (discovered jinja.toml + InitializationOptions overlay).
    pub config: JinjaConfig,
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
            config: JinjaConfig::default(),
            sources: HashMap::new(),
            generation: 0,
        }
    }

    /// Build initial state with an explicit config (for testing / initialize wiring).
    pub fn with_config(config: JinjaConfig) -> Self {
        Self {
            workspace: WorkspaceIndex::default(),
            config,
            sources: HashMap::new(),
            generation: 0,
        }
    }

    /// REQ-EDIT-10: Apply an InitializationOptions (or didChangeConfiguration) overlay.
    pub fn apply_init_options(&mut self, overlay: ConfigOverlay) {
        self.config.apply_overlay(overlay);
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
