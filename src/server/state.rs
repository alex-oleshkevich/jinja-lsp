use std::{collections::HashMap, path::Path};

use crate::{
    builtins::registry::Registry,
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
    /// REQ-BLTN-07: unified doc registry — core + custom_builtins from config.
    pub registry: Registry,
    /// REQ-DEF-07: client declared textDocument/definition linkSupport in InitializeParams.
    pub definition_link_support: bool,
}

impl ServerState {
    /// Build initial state by discovering all templates in `templates_dirs`.
    pub fn from_dirs(templates_dirs: &[&Path], extensions: &[&str]) -> Self {
        let config = JinjaConfig::default();
        let registry = Self::build_registry(&config);
        Self {
            workspace: build_workspace(templates_dirs, extensions),
            config,
            sources: HashMap::new(),
            generation: 0,
            registry,
            definition_link_support: false,
        }
    }

    /// Build initial state with an explicit config (for testing / initialize wiring).
    pub fn with_config(config: JinjaConfig) -> Self {
        let registry = Self::build_registry(&config);
        Self {
            workspace: WorkspaceIndex::default(),
            config,
            sources: HashMap::new(),
            generation: 0,
            registry,
            definition_link_support: false,
        }
    }

    /// REQ-EDIT-10: Apply an InitializationOptions (or didChangeConfiguration) overlay.
    pub fn apply_init_options(&mut self, overlay: ConfigOverlay) {
        self.config.apply_overlay(overlay);
        self.registry = Self::build_registry(&self.config);
    }

    /// REQ-BLTN-07: Build a registry from core + all configured custom_builtins dirs.
    fn build_registry(config: &JinjaConfig) -> Registry {
        let mut reg = Registry::load_core();
        for dir_str in &config.custom_builtins {
            reg.load_custom_builtins(Path::new(dir_str));
        }
        reg
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
