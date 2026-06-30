use std::{collections::HashMap, path::Path};

use crate::{
    builtins::registry::Registry,
    config::{ConfigError, ConfigOverlay, ConfigWarning, JinjaConfig},
    parsing::{extract, inline::detect_inline_regions},
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
    /// True when the client advertised UTF-8 position encoding and we negotiated it;
    /// false when falling back to UTF-16 (LSP default).
    pub position_encoding_utf8: bool,
    /// REQ-CFG-10: absolute path of the discovered config file (jinja.toml or pyproject.toml).
    /// None when running zero-config.
    pub config_file_path: Option<String>,
    /// REQ-CFG-10: workspace root path — needed to re-discover config on reload.
    pub workspace_root: Option<String>,
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
            position_encoding_utf8: false,
            config_file_path: None,
            workspace_root: None,
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
            position_encoding_utf8: false,
            config_file_path: None,
            workspace_root: None,
        }
    }

    /// REQ-EDIT-10 / REQ-CFG-07: Apply an InitializationOptions overlay and validate.
    /// Returns validation warnings on success, or an error for invalid config.
    pub fn apply_init_options(
        &mut self,
        overlay: ConfigOverlay,
    ) -> Result<Vec<ConfigWarning>, ConfigError> {
        self.config.apply_overlay(overlay);
        self.registry = Self::build_registry(&self.config);
        self.config.validate()
    }

    /// Replace the active config and rebuild the registry from it.
    /// Called during `initialize` when config is discovered before overlays are applied.
    pub fn reset_config(&mut self, config: JinjaConfig) {
        self.registry = Self::build_registry(&config);
        self.config = config;
    }

    /// REQ-BLTN-07 / REQ-EXT-02 / REQ-HINT-02: Build a registry from core +
    /// extension packs + configured custom_builtins dirs + user hints dirs.
    fn build_registry(config: &JinjaConfig) -> Registry {
        let mut reg = Registry::load_core();
        // REQ-EXT-02: load configured extension packs.
        let extras: Vec<&str> = config.extras.iter().map(|s| s.as_str()).collect();
        reg.load_packs(&extras);
        // REQ-BLTN-07: load docs from custom_builtins dirs.
        for dir_str in &config.custom_builtins {
            reg.load_custom_builtins(Path::new(dir_str));
        }
        // REQ-HINT-02: load user hints from configured hints dirs.
        for dir_str in &config.hints {
            reg.load_hints_from_dir(Path::new(dir_str));
        }
        reg
    }

    /// Pass 1 (REQ-ARCH-03): re-extract one file and atomically replace its
    /// TemplateIndex without touching any other entry.
    ///
    /// REQ-INLN-02/REQ-EXTR-05: if `key` is a host file (non-Jinja extension),
    /// detect embedded Jinja templates and index each one as `key::<offset>`.
    pub fn update_file(&mut self, key: &str, source: &str) {
        let mut idx = extract(source);
        idx.path = key.to_owned();
        self.workspace.templates.insert(key.to_owned(), idx);
        self.sources.insert(key.to_owned(), source.to_owned());

        // Remove stale inline entries from a previous version of this file.
        let inline_prefix = format!("{key}::");
        self.workspace.templates.retain(|k, _| !k.starts_with(&inline_prefix));

        // For host files, detect embedded Jinja templates and index each one.
        if self.is_host_file(key) {
            let patterns: Vec<&str> = self.config.inline_patterns.iter().map(|s| s.as_str()).collect();
            for region in detect_inline_regions(source, &patterns) {
                let inline_key = format!("{key}::{}", region.host_offset);
                self.workspace.index_inline(&inline_key, &region.content);
            }
        }

        self.generation += 1;
    }

    /// True when `key` has a file extension that is NOT in the configured Jinja extensions.
    fn is_host_file(&self, key: &str) -> bool {
        let ext = Path::new(key)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        !ext.is_empty() && !self.config.extensions.iter().any(|e| e == ext)
    }
}
