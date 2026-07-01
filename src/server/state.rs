use std::{collections::HashMap, path::Path};

use crate::{
    builtins::{hints::load_sidecar, registry::Registry},
    config::{ConfigError, ConfigOverlay, ConfigWarning, JinjaConfig},
    parsing::{extract, inline::detect_inline_regions},
    workspace::{build_workspace, index::WorkspaceIndex, inline::InlineRange},
};

/// Per-folder state for additional workspace folders (folder 1..N).
/// Folder 0 (the primary folder) is stored directly in `ServerState`.
pub struct FolderState {
    /// Absolute path of the workspace folder root.
    pub root: std::path::PathBuf,
    pub workspace: WorkspaceIndex,
    pub config: JinjaConfig,
    pub registry: Registry,
    /// Absolute path of the config file discovered for this folder.
    pub config_file_path: Option<String>,
    /// Incremented by Pass 1; Pass 2 guards against stale relinks.
    pub generation: u64,
}

/// Shared mutable state held behind Arc<RwLock<>> in the LSP backend.
pub struct ServerState {
    pub workspace: WorkspaceIndex,
    /// Active config (discovered jinja.toml + InitializationOptions overlay).
    pub config: JinjaConfig,
    /// Raw source text per file key — used by formatting handlers.
    pub sources: HashMap<String, String>,
    /// Incremented by every Pass 1 on the primary folder; Pass 2 checks it to discard stale relinks.
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
    /// REQ-EXTR-08: additional workspace folders (folder 1..N).
    /// Each gets its own isolated WorkspaceIndex and config.
    pub extra_folders: Vec<FolderState>,
    /// Persisted overlay from initializationOptions / didChangeConfiguration.
    /// Re-applied on top of every file-based config reload so the editor's
    /// settings always take precedence over jinja.toml (E15 §5.7).
    pub init_overlay: Option<ConfigOverlay>,
    /// REQ-HINT-01: per-template registries that layer a sidecar `.hints.md` on top of
    /// the folder registry. Populated (and invalidated) by `update_file`.
    pub sidecar_registries: HashMap<String, Registry>,
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
            extra_folders: Vec::new(),
            init_overlay: None,
            sidecar_registries: HashMap::new(),
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
            extra_folders: Vec::new(),
            init_overlay: None,
            sidecar_registries: HashMap::new(),
        }
    }

    /// REQ-EDIT-10 / REQ-CFG-07: Apply an InitializationOptions overlay and validate.
    /// Persists the overlay so it can be re-applied after config file reloads.
    /// Returns validation warnings on success, or an error for invalid config.
    pub fn apply_init_options(
        &mut self,
        overlay: ConfigOverlay,
    ) -> Result<Vec<ConfigWarning>, ConfigError> {
        self.init_overlay = Some(overlay.clone());
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

    /// Replace the base config from a file reload, then re-apply the persisted overlay.
    /// Use this instead of `reset_config` when reloading jinja.toml at runtime so that
    /// the editor's initializationOptions always win (E15 §5.7).
    pub fn reload_base_config(&mut self, base: JinjaConfig) {
        self.config = base;
        if let Some(overlay) = self.init_overlay.clone() {
            self.config.apply_overlay(overlay);
        }
        self.registry = Self::build_registry(&self.config);
    }

    /// REQ-BLTN-07 / REQ-EXT-02 / REQ-HINT-02: Build a registry from core +
    /// extension packs + configured custom_builtins dirs + user hints dirs.
    pub fn build_registry(config: &JinjaConfig) -> Registry {
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

    /// REQ-EXTR-08: Return the WorkspaceIndex for the folder that owns `key`.
    /// Uses longest-prefix match on folder roots. Falls back to the primary workspace.
    pub fn workspace_for<'a>(&'a self, key: &str) -> &'a WorkspaceIndex {
        self.extra_folders.iter()
            .filter(|f| key_under_root(key, f.root.to_str().unwrap_or("")))
            .max_by_key(|f| f.root.to_str().map(|s| s.len()).unwrap_or(0))
            .map(|f| &f.workspace)
            .unwrap_or(&self.workspace)
    }

    /// REQ-EXTR-08: Mutable borrow of the extra FolderState that owns `key`, if any.
    /// Returns `None` if the file belongs to the primary folder.
    fn extra_folder_for_mut(&mut self, key: &str) -> Option<&mut FolderState> {
        let idx = self.extra_folders.iter().enumerate()
            .filter(|(_, f)| key_under_root(key, f.root.to_str().unwrap_or("")))
            .max_by_key(|(_, f)| f.root.to_str().map(|s| s.len()).unwrap_or(0))
            .map(|(i, _)| i)?;
        Some(&mut self.extra_folders[idx])
    }

    /// REQ-EXTR-08: Return the Registry for the folder that owns `key`.
    /// REQ-HINT-01: if a sidecar `.hints.md` was loaded for this template,
    /// returns that overlay registry instead of the bare folder registry.
    pub fn registry_for<'a>(&'a self, key: &str) -> &'a Registry {
        if let Some(sidecar) = self.sidecar_registries.get(key) {
            return sidecar;
        }
        self.base_registry_for(key)
    }

    /// Folder/global registry without sidecar overlay — used to build sidecars.
    pub fn base_registry_for<'a>(&'a self, key: &str) -> &'a Registry {
        self.extra_folders.iter()
            .filter(|f| key_under_root(key, f.root.to_str().unwrap_or("")))
            .max_by_key(|f| f.root.to_str().map(|s| s.len()).unwrap_or(0))
            .map(|f| &f.registry)
            .unwrap_or(&self.registry)
    }

    /// REQ-EXTR-08: Return the JinjaConfig for the folder that owns `key`.
    pub fn config_for<'a>(&'a self, key: &str) -> &'a JinjaConfig {
        self.extra_folders.iter()
            .filter(|f| key_under_root(key, f.root.to_str().unwrap_or("")))
            .max_by_key(|f| f.root.to_str().map(|s| s.len()).unwrap_or(0))
            .map(|f| &f.config)
            .unwrap_or(&self.config)
    }

    /// Pass 1 (REQ-ARCH-03): re-extract one file and atomically replace its
    /// TemplateIndex without touching any other entry.
    ///
    /// REQ-INLN-02/REQ-EXTR-05: if `key` is a host file (non-Jinja extension),
    /// detect embedded Jinja templates and index each one as `key::<offset>`.
    ///
    /// REQ-EXTR-08: routes the update to the correct folder's WorkspaceIndex.
    pub fn update_file(&mut self, key: &str, source: &str) {
        self.sources.insert(key.to_owned(), source.to_owned());

        if let Some(folder) = self.extra_folder_for_mut(key) {
            Self::index_file_into(key, source, &mut folder.workspace, &folder.config);
            folder.generation += 1;
        } else {
            let config = self.config.clone();
            Self::index_file_into(key, source, &mut self.workspace, &config);
            self.generation += 1;
        }

        // REQ-HINT-01: rebuild the per-template sidecar registry when the template changes.
        // Use base_registry_for (not registry_for) so the stale sidecar is never its own seed.
        let base = self.base_registry_for(key).clone();
        self.refresh_sidecar(key, base);
    }

    /// Check for `{key}.hints.md` and (re)build the sidecar registry entry.
    /// Clears the cached entry when no sidecar exists so stale hints are evicted.
    pub fn refresh_sidecar(&mut self, key: &str, base_registry: Registry) {
        let path = Path::new(key);
        let sidecar_exists = crate::builtins::hints::find_sidecar(path).is_some();
        if sidecar_exists {
            let mut reg = base_registry;
            load_sidecar(path, &mut reg);
            self.sidecar_registries.insert(key.to_owned(), reg);
        } else {
            self.sidecar_registries.remove(key);
        }
    }

    /// Index `source` at `key` into the given workspace, handling inline regions.
    fn index_file_into(key: &str, source: &str, workspace: &mut WorkspaceIndex, config: &JinjaConfig) {
        let mut idx = extract(source);
        idx.path = key.to_owned();
        workspace.templates.insert(key.to_owned(), idx);

        // Remove stale inline entries from a previous version of this file.
        let inline_prefix = format!("{key}::");
        workspace.templates.retain(|k, _| !k.starts_with(&inline_prefix));

        // For host files, detect embedded Jinja templates and index each one.
        if Self::is_host_file_for_config(key, config) {
            let patterns: Vec<&str> = config.inline_patterns.iter().map(|s| s.as_str()).collect();
            // Clear stale inline range metadata alongside stale template entries.
            workspace.inline_ranges.retain(|k, _| !k.starts_with(&inline_prefix));
            for region in detect_inline_regions(source, &patterns) {
                let inline_key = format!("{key}::{}", region.host_offset);
                workspace.index_inline(&inline_key, &region.content);
                // REQ-INLN-03: store host-coordinate metadata for position translation.
                workspace.register_inline_range(&inline_key, InlineRange {
                    host_path: key.to_owned(),
                    host_offset: region.host_offset,
                    host_line: region.host_line,
                    host_col: region.host_col,
                    content_len: region.content.len(),
                });
            }
        }
    }

    fn is_host_file_for_config(key: &str, config: &JinjaConfig) -> bool {
        let ext = Path::new(key)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        !ext.is_empty() && !config.extensions.iter().any(|e| e == ext)
    }
}

/// REQ-EXTR-08 / mauu: path-boundary-safe starts_with check for folder roots.
///
/// `starts_with` alone lets `/a/proj` match `/a/project/x.html`.
/// This function requires that the prefix is followed by `'/'` or equals `key` exactly,
/// so folder roots with overlapping name prefixes are routed correctly.
pub fn key_under_root(key: &str, root: &str) -> bool {
    if root.is_empty() {
        return false;
    }
    if !key.starts_with(root) {
        return false;
    }
    // Either key IS root, or the next byte is a path separator.
    key.len() == root.len() || key.as_bytes().get(root.len()) == Some(&b'/')
}
