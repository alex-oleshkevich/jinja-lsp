// REQ-ARCH-01..08: jinja-lsp LSP server — one binary, three front-ends over
// one shared two-pass pipeline.

mod convert;
pub mod state;

pub use convert::*;

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::{Client, LanguageServer, LspService, Server, jsonrpc::Result, lsp_types::*};

use crate::builtins::registry::Registry;
use crate::diagnostic::DiagnosticSeverity as InternalSeverity;
use crate::diagnostics::checks::run_checks;
use crate::diagnostics::{filter_by_config, suppress_by_noqa};
use crate::features::call_hierarchy::{
    CallHierarchyItem as InternalCallHierarchyItem, HierarchyRange, ItemKind, incoming_calls,
    outgoing_calls, prepare_call_hierarchy,
};
use crate::features::code_actions::{
    ActionKind, CodeAction as InternalCodeAction, code_actions, selection_code_actions,
};
use crate::features::code_lens::{
    CodeLensConfig, LensData, LensKind, LensSymbolKind, code_lens as code_lens_feature,
    code_lens_resolve as code_lens_resolve_feature,
};
use crate::features::completions::{CompletionKind, complete, resolve_doc};
use crate::features::definition::{DefinitionLocation, go_to_definition};
use crate::features::document_highlight::{HighlightKind, document_highlight};
use crate::features::folding::{FoldKind, fold_ranges};
use crate::features::hover::hover as hover_feature;
use crate::features::inlay_hints::{
    InlayHintData, InlayHintsConfig, inlay_hint_resolve, inlay_hints,
};
use crate::features::references::{ReferenceLocation, find_references};
use crate::features::rename::{check_rename_preconditions, compute_rename, rename_at_cursor};
use crate::features::semantic_tokens::{
    SemanticToken as InternalSemanticToken, TOKEN_MODIFIERS, TOKEN_TYPES,
    semantic_tokens_full as stok_full, semantic_tokens_range as stok_range,
};
use crate::features::signature_help::signature_help as sig_help_feature;
use crate::features::source_line;
use crate::features::symbols::{
    SymbolKind as InternalSymbolKind, WorkspaceSymbol as InternalWorkspaceSymbol, document_symbols,
    workspace_symbols,
};
use state::ServerState;
use tower_lsp::lsp_types::Diagnostic as LspDiagnostic;

/// REQ-ARCH-02: direct all tracing output to stderr; never stdout (stdout
/// carries JSON-RPC framing).
pub fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .try_init();
}

/// The tower-lsp backend.  The `state` field is the single source of truth;
/// Pass 1 and Pass 2 both go through it.
pub struct Backend {
    pub client: Client,
    pub state: Arc<RwLock<ServerState>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ServerState::with_config(
                crate::config::JinjaConfig::default(),
            ))),
        }
    }

    /// Pass 1 (REQ-ARCH-03): re-extract one file, atomically replace its entry.
    async fn pass1(&self, key: &str, source: &str) {
        self.state.write().await.update_file(key, source);
    }

    /// Convert an inbound URI to a workspace key.
    ///
    /// `Url::path()` is percent-encoded (`/my dir/t.html` -> `/my%20dir/t.html`),
    /// which never matches keys built from real filesystem paths. Use
    /// `to_file_path()` to decode it; fall back to the raw (encoded) path for
    /// non-`file://` URIs, which have no meaningful filesystem path.
    pub fn uri_to_key(uri: &Url) -> String {
        uri.to_file_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| uri.path().to_owned())
    }

    /// Compute the final diagnostics for `key` — filtered, suppressed and sorted.
    ///
    /// The single compute path behind both delivery modes: `publish_file_diagnostics`
    /// pushes the result, and `textDocument/diagnostic` reads the stored copy. Returns
    /// `None` only when the file is not tracked (no source or no index).
    async fn compute_file_diagnostics(&self, key: &str) -> Option<Vec<LspDiagnostic>> {
        let state = self.state.read().await;
        let workspace = state.workspace_for(key);
        let (Some(source), Some(index)) = (state.sources.get(key), workspace.templates.get(key))
        else {
            return None;
        };
        let registry = state.registry_for(key);
        let mut raw = run_checks(source, key, index, registry, workspace);

        // REQ-INLN-03: also check each inline sub-region and translate positions to host coords.
        let inline_keys: Vec<_> = workspace
            .inline_ranges_for(key)
            .map(|(k, r)| (k.to_owned(), r.clone()))
            .collect();
        for (ikey, range) in &inline_keys {
            let inline_source = source
                .get(range.host_offset..range.host_offset + range.content_len)
                .unwrap_or("");
            if let Some(iidx) = workspace.templates.get(ikey.as_str()) {
                let mut inline_diags = run_checks(inline_source, ikey, iidx, registry, workspace);
                for d in &mut inline_diags {
                    let (hl, hc) = range.to_host_position(d.line, d.col);
                    d.line = hl;
                    d.col = hc;
                    d.file = key.to_owned();
                }
                raw.extend(inline_diags);
            }
        }

        let config = state.config_for(key);
        let select: Vec<&str> = config.lint.select.iter().map(|s| s.as_str()).collect();
        let ignore: Vec<&str> = config.lint.ignore.iter().map(|s| s.as_str()).collect();
        let filtered: Vec<crate::diagnostic::Diagnostic> = filter_by_config(&raw, &select, &ignore)
            .into_iter()
            .cloned()
            .collect();
        let (kept, w107s) = suppress_by_noqa(&filtered, source);
        // REQ-DIAG-06/jinja-lsp-ibun: W107 (invalid-noqa) must respect the same
        // select/ignore filters as every other diagnostic code.
        let w107s: Vec<crate::diagnostic::Diagnostic> = filter_by_config(&w107s, &select, &ignore)
            .into_iter()
            .cloned()
            .collect();
        let utf8 = state.position_encoding_utf8;
        let mut lsp_diags: Vec<LspDiagnostic> = kept
            .into_iter()
            .chain(w107s)
            .map(|d| to_lsp_diagnostic(source, utf8, &d))
            .collect();
        lsp_diags.sort_by_key(|d| (d.range.start.line, d.range.start.character));
        Some(lsp_diags)
    }

    /// REQ-DIAG-07: push the diagnostics for `key` and store the same vector for
    /// pull-mode clients. A cleared finding is published as an explicit empty vector.
    async fn publish_file_diagnostics(&self, key: &str) {
        let Some(lsp_diags) = self.compute_file_diagnostics(key).await else {
            return;
        };
        self.state
            .write()
            .await
            .diagnostics
            .insert(key.to_owned(), lsp_diags.clone());
        let uri = path_to_uri(key);
        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }

    /// REQ-ARCH-08: scan every workspace folder in the background, under a
    /// `workDoneProgress` token so the editor can show and cancel it.
    ///
    /// Called from `initialized`, never from `initialize` — the scan reads every
    /// template off disk, and a client that blocks on the `initialize` response
    /// (most do) shows a frozen editor for the duration on a large workspace.
    async fn scan_workspaces(&self) {
        let token = ProgressToken::String("jinja-lsp/initial-scan".to_owned());

        // Same rule as the refresh requests: only ask a client that declared support.
        let report = self.state.read().await.work_done_progress_support
            && self
                .client
                .send_request::<request::WorkDoneProgressCreate>(WorkDoneProgressCreateParams {
                    token: token.clone(),
                })
                .await
                .is_ok();

        if report {
            self.send_progress(
                &token,
                WorkDoneProgress::Begin(WorkDoneProgressBegin {
                    title: "Indexing Jinja templates".to_owned(),
                    cancellable: Some(false),
                    ..Default::default()
                }),
            )
            .await;
        }

        // Collect the scan inputs up front so the state lock is not held across the
        // blocking disk walk.
        let (primary, extras) = {
            let state = self.state.read().await;
            let primary = state.workspace_root.as_ref().map(|root| {
                let root = std::path::PathBuf::from(root);
                (
                    state.config.resolved_template_dirs(&root),
                    state.config.extensions.clone(),
                )
            });
            let extras: Vec<_> = state
                .extra_folders
                .iter()
                .map(|f| {
                    (
                        f.config.resolved_template_dirs(&f.root),
                        f.config.extensions.clone(),
                    )
                })
                .collect();
            (primary, extras)
        };

        let started = std::time::Instant::now();
        if let Some((dirs, exts)) = primary {
            let workspace = Self::build_workspace_blocking(dirs, exts).await;
            tracing::info!(
                "jinja-lsp: indexed {} template(s) in {:?}",
                workspace.templates.len(),
                started.elapsed(),
            );
            self.state.write().await.workspace = workspace;
        }
        for (i, (dirs, exts)) in extras.into_iter().enumerate() {
            let workspace = Self::build_workspace_blocking(dirs, exts).await;
            let mut state = self.state.write().await;
            if let Some(folder) = state.extra_folders.get_mut(i) {
                folder.workspace = workspace;
            }
        }

        if report {
            self.send_progress(
                &token,
                WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: Some(format!("indexed in {:?}", started.elapsed())),
                }),
            )
            .await;
        }

        // Anything opened while the scan was running was indexed against an empty
        // workspace, so its cross-file findings are wrong until republished.
        self.republish_all_diagnostics().await;
    }

    async fn build_workspace_blocking(
        dirs: Vec<std::path::PathBuf>,
        exts: Vec<String>,
    ) -> crate::workspace::index::WorkspaceIndex {
        tokio::task::spawn_blocking(move || {
            let dir_refs: Vec<&std::path::Path> = dirs.iter().map(|p| p.as_path()).collect();
            let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
            crate::workspace::build_workspace_abs(&dir_refs, &ext_refs)
        })
        .await
        .unwrap_or_default()
    }

    async fn send_progress(&self, token: &ProgressToken, value: WorkDoneProgress) {
        self.client
            .send_notification::<notification::Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(value),
            })
            .await;
    }

    /// E01 §5.6 / REQ-ARCH-10: tell the client to re-pull inlay hints and code lenses.
    ///
    /// Both are *derived* UI: their content depends on cross-file facts (a macro's
    /// arity, a block override, a reference count) that Pass 2 can change without the
    /// dependent file being edited. Without these the editor keeps rendering pre-relink
    /// values until it re-pulls for its own reasons. This is the payoff of the two-pass
    /// design — the client refreshes without the server reparsing anything.
    async fn refresh_derived_ui(&self) {
        // Only clients that declared refreshSupport have a handler registered for these;
        // sending regardless makes the client answer MethodNotFound, which is our bug,
        // not theirs. Read both flags under one lock, then release it before awaiting.
        let (inlay, lens) = {
            let state = self.state.read().await;
            (
                state.inlay_hint_refresh_support,
                state.code_lens_refresh_support,
            )
        };
        if inlay {
            if let Err(e) = self.client.inlay_hint_refresh().await {
                tracing::debug!("inlayHint/refresh failed: {e}");
            }
        }
        if lens {
            if let Err(e) = self.client.code_lens_refresh().await {
                tracing::debug!("codeLens/refresh failed: {e}");
            }
        }
    }

    /// Re-publish diagnostics for every file currently tracked in `state.sources`.
    /// Called after a config change so open files reflect new lint rules / registry.
    async fn republish_all_diagnostics(&self) {
        let keys: Vec<String> = self.state.read().await.sources.keys().cloned().collect();
        for key in keys {
            self.publish_file_diagnostics(&key).await;
        }
    }

    /// REQ-CFG-10: re-parse the config file and reload affected state.
    /// REQ-EXTR-08: detects which folder the config belongs to and reloads only that folder.
    #[tracing::instrument(skip(self), name = "config_reload")]
    async fn reload_config_file(&self, file_path: &str) {
        // Check if the config belongs to an extra folder (REQ-EXTR-08).
        let extra_idx = {
            let state = self.state.read().await;
            state
                .extra_folders
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    f.config_file_path.as_deref() == Some(file_path)
                        || state::key_under_root(file_path, f.root.to_str().unwrap_or(""))
                })
                .max_by_key(|(_, f)| f.root.to_str().map(|s| s.len()).unwrap_or(0))
                .map(|(i, _)| i)
        };

        if let Some(ei) = extra_idx {
            let root = {
                let state = self.state.read().await;
                state.extra_folders[ei].root.to_string_lossy().into_owned()
            };
            let root_path = std::path::Path::new(&root);
            let (new_config, new_config_path) =
                match crate::config::JinjaConfig::discover_with_path(root_path) {
                    Ok(pair) => pair,
                    Err(e) => {
                        let msg = format!(
                            "jinja-lsp: extra folder config reload error (previous retained): {e}"
                        );
                        tracing::warn!("{msg}");
                        self.client.show_message(MessageType::WARNING, msg).await;
                        return;
                    }
                };
            let dirs = new_config.resolved_template_dirs(root_path);
            let exts = new_config.extensions.clone();
            let workspace_changed;
            {
                let mut state = self.state.write().await;
                state.extra_folders[ei].config_file_path =
                    new_config_path.map(|p| p.to_string_lossy().into_owned());
                let (registry_changed, ws_changed) = crate::server::state::config_delta(
                    &state.extra_folders[ei].config,
                    &new_config,
                );
                workspace_changed = ws_changed;
                if registry_changed {
                    state.extra_folders[ei].registry =
                        Registry::from_config(&new_config, root_path);
                }
                state.extra_folders[ei].config = new_config;
            }
            if workspace_changed {
                let new_workspace = tokio::task::spawn_blocking(move || {
                    let dir_refs: Vec<&std::path::Path> =
                        dirs.iter().map(|p| p.as_path()).collect();
                    let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
                    crate::workspace::build_workspace_abs(&dir_refs, &ext_refs)
                })
                .await
                .unwrap_or_default();
                self.state.write().await.extra_folders[ei].workspace = new_workspace;
            }
            let msg = format!("jinja-lsp: extra folder config reloaded from {file_path}");
            tracing::info!("{msg}");
            self.client.log_message(MessageType::INFO, msg).await;
            self.republish_all_diagnostics().await;
            // REQ-ARCH-10: a config reload relinks, so derived UI is stale too.
            self.refresh_derived_ui().await;
            return;
        }

        // Primary folder config reload.
        let root = {
            let state = self.state.read().await;
            state.workspace_root.clone()
        };
        let Some(root) = root else { return };
        let root_path = std::path::Path::new(&root);
        let (new_config, new_config_path) =
            match crate::config::JinjaConfig::discover_with_path(root_path) {
                Ok(pair) => pair,
                Err(e) => {
                    // REQ-CFG-10 / E15 §12.2: invalid config retains previous; user is notified.
                    let msg =
                        format!("jinja-lsp: config reload error (previous config retained): {e}");
                    tracing::warn!("{msg}");
                    self.client.show_message(MessageType::WARNING, msg).await;
                    return;
                }
            };
        let workspace_changed;
        let dirs: Vec<std::path::PathBuf>;
        let exts: Vec<String>;
        {
            let mut state = self.state.write().await;
            state.config_file_path = new_config_path.map(|p| p.to_string_lossy().into_owned());
            dirs = new_config.resolved_template_dirs(root_path);
            exts = new_config.extensions.clone();
            let (_, ws_changed) = state.reload_config_selective(new_config);
            workspace_changed = ws_changed;
        }
        if workspace_changed {
            let new_workspace = tokio::task::spawn_blocking(move || {
                let dir_refs: Vec<&std::path::Path> = dirs.iter().map(|p| p.as_path()).collect();
                let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
                crate::workspace::build_workspace_abs(&dir_refs, &ext_refs)
            })
            .await
            .unwrap_or_default();
            self.state.write().await.workspace = new_workspace;
        }
        let msg = format!("jinja-lsp: config reloaded from {file_path}");
        tracing::info!("{msg}");
        self.client.log_message(MessageType::INFO, msg).await;
        self.republish_all_diagnostics().await;
        // REQ-ARCH-10: a config reload relinks, so derived UI is stale too.
        self.refresh_derived_ui().await;
    }
}

/// REQ-EDIT-11: the canonical languageIds the server accepts, one per language
/// declared in editors/zed/extension.toml's [language_servers.jinja2-lsp.language_ids]
/// (which must stay in sync with this list). Any other ID (html, jinja2, j2, …) is
/// not a Jinja file from the server's perspective.
pub fn is_jinja_language_id(lang: &str) -> bool {
    matches!(
        lang,
        "jinja"
            | "jinja-html"
            | "jinja-yaml"
            | "jinja-json"
            | "jinja-markdown"
            | "jinja-css"
            | "jinja-python"
            | "jinja-javascript"
            | "jinja-typescript"
            | "jinja-shell"
            | "jinja-sql"
            | "jinja-toml"
            | "jinja-xml"
            | "jinja-go"
            | "jinja-rust"
            | "jinja-lua"
            | "jinja-ini"
            | "jinja-dotenv"
            | "jinja-dockerfile"
            | "jinja-makefile"
            | "jinja-systemd"
            | "jinja-hcl"
            | "jinja-graphql"
            | "jinja-plaintext"
    )
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// REQ-ARCH-08: declare capabilities matching the feature set.
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // REQ-DEF-07: record whether the client supports LocationLink for goto_definition.
        let link_support = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|td| td.definition.as_ref())
            .and_then(|d| d.link_support)
            .unwrap_or(false);
        // jinja-lsp-7b7s: negotiate UTF-8 position encoding when the client supports it.
        // Our internal offsets are byte-based (tree-sitter, Rust str), which equals
        // UTF-8 code units, so UTF-8 clients need no conversion at all.
        let utf8 = params
            .capabilities
            .general
            .as_ref()
            .and_then(|g| g.position_encodings.as_ref())
            .map(|encs| encs.contains(&PositionEncodingKind::UTF8))
            .unwrap_or(false);

        // jinja-lsp-uoyh: Resolve workspace root from root_uri or first workspace folder.
        // Additional workspace folders (REQ-EXTR-08) get their own FolderState below.
        let all_folder_paths: Vec<std::path::PathBuf> = {
            let mut paths: Vec<std::path::PathBuf> = Vec::new();
            if let Some(root) = params.root_uri.as_ref().and_then(|u| u.to_file_path().ok()) {
                paths.push(root);
            }
            if let Some(folders) = &params.workspace_folders {
                for f in folders {
                    if let Ok(p) = f.uri.to_file_path() {
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
            }
            paths
        };
        let root_path: Option<&std::path::PathBuf> = all_folder_paths.first();

        // REQ-CFG-01 / REQ-CFG-10: discover config from project root; record path for live reload.
        let (discovered_config, config_file_path) = root_path
            .and_then(|root| crate::config::JinjaConfig::discover_with_path(root).ok())
            .unwrap_or_default();

        tracing::info!(
            "jinja-lsp: initializing, root={:?}, config={:?}",
            root_path,
            config_file_path,
        );

        // REQ-ARCH-08: `initialize` must return immediately. The workspace scan reads
        // every template off disk, so it runs in the background from `initialized`
        // under a workDoneProgress token — see `scan_workspaces`. Start empty here.
        let initial_workspace = crate::workspace::index::WorkspaceIndex::default();

        // REQ-EXTR-08: build an isolated FolderState for each additional workspace folder.
        let mut extra_folders: Vec<state::FolderState> = Vec::new();
        for extra_root in all_folder_paths.iter().skip(1) {
            let (extra_cfg, extra_cfg_path) =
                crate::config::JinjaConfig::discover_with_path(extra_root)
                    .ok()
                    .unwrap_or_default();
            let extra_registry = Registry::from_config(&extra_cfg, extra_root);
            let extra_dirs = extra_cfg.resolved_template_dirs(extra_root);
            let extra_exts: Vec<String> = extra_cfg.extensions.clone();
            // REQ-ARCH-08: scanned in the background too (see `scan_workspaces`).
            let _ = (&extra_dirs, &extra_exts);
            extra_folders.push(state::FolderState {
                root: extra_root.clone(),
                workspace: crate::workspace::index::WorkspaceIndex::default(),
                config: extra_cfg,
                registry: extra_registry,
                config_file_path: extra_cfg_path.map(|p| p.to_string_lossy().into_owned()),
                generation: 0,
            });
        }

        // REQ-EDIT-10 / REQ-CFG-07: apply InitializationOptions overlay on top of discovered config.
        {
            let mut state = self.state.write().await;
            state.definition_link_support = link_support;
            state.inlay_hint_refresh_support = params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|w| w.inlay_hint.as_ref())
                .and_then(|i| i.refresh_support)
                .unwrap_or(false);
            state.code_lens_refresh_support = params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|w| w.code_lens.as_ref())
                .and_then(|c| c.refresh_support)
                .unwrap_or(false);
            state.work_done_progress_support = params
                .capabilities
                .window
                .as_ref()
                .and_then(|w| w.work_done_progress)
                .unwrap_or(false);
            state.position_encoding_utf8 = utf8;
            state.workspace = initial_workspace;
            state.config_file_path = config_file_path.map(|p| p.to_string_lossy().into_owned());
            state.workspace_root = root_path.map(|p| p.to_string_lossy().into_owned());
            state.extra_folders = extra_folders;
            state.reset_config(discovered_config);
            if let Some(opts) = params.initialization_options {
                if let Ok(overlay) = serde_json::from_value::<crate::config::ConfigOverlay>(opts) {
                    match state.apply_init_options(overlay) {
                        Ok(warnings) => {
                            for w in &warnings {
                                tracing::warn!("jinja-lsp config warning: {w:?}");
                            }
                        }
                        Err(e) => {
                            tracing::error!("jinja-lsp config error: {e}");
                        }
                    }
                }
            }
        }
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "jinja-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                position_encoding: if utf8 {
                    Some(PositionEncodingKind::UTF8)
                } else {
                    None
                },
                // REQ-ARCH-05: full-text sync (didOpen, didChange, didSave, didClose)
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(true),
                    // REQ-CMP-01: trigger chars that match TRIGGER_CHARS in features::completions.
                    trigger_characters: Some(vec![
                        "{".into(),
                        "%".into(),
                        " ".into(),
                        "|".into(),
                        ".".into(),
                        "(".into(),
                        ",".into(),
                        "\"".into(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".into(), ",".into()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES
                                    .iter()
                                    .map(|s| SemanticTokenType::new(s))
                                    .collect(),
                                token_modifiers: TOKEN_MODIFIERS
                                    .iter()
                                    .map(|s| SemanticTokenModifier::new(s))
                                    .collect(),
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(true),
                            ..Default::default()
                        },
                    ),
                ),
                // E01 §5.6 / REQ-ARCH-09: push AND pull. Zed uses pull mode, so a
                // server that only pushes gives it nothing.
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("jinja-lsp".to_owned()),
                        // Diagnostics are file-local; an edit in one template never
                        // changes another's findings, so there is nothing to re-pull.
                        inter_file_dependencies: false,
                        workspace_diagnostics: false,
                        ..Default::default()
                    },
                )),
                inlay_hint_provider: Some(OneOf::Left(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(true),
                }),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        resolve_provider: Some(true),
                        ..Default::default()
                    },
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "jinja-lsp.extract-macro".to_owned(),
                        "jinja-lsp.wrap-block".to_owned(),
                        "jinja-lsp.rename".to_owned(),
                    ],
                    ..Default::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                // REQ-ARCH-06: watched-files registration (config + templates)
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "jinja-lsp initialized")
            .await;
        // REQ-ARCH-08: the initial scan runs here, not in `initialize`.
        self.scan_workspaces().await;
        // REQ-CFG-10 / REQ-ARCH-06: register config file watchers so the client
        // notifies us when jinja.toml or pyproject.toml changes on disk.
        // REQ-HINT-08: also watch *.hints.md so live-editing a sidecar rebuilds the registry.
        let watchers = vec![
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/jinja.toml".to_owned()),
                kind: Some(WatchKind::Change | WatchKind::Create | WatchKind::Delete),
            },
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/pyproject.toml".to_owned()),
                kind: Some(WatchKind::Change | WatchKind::Create | WatchKind::Delete),
            },
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/*.hints.md".to_owned()),
                kind: Some(WatchKind::Change | WatchKind::Create | WatchKind::Delete),
            },
        ];
        let registration = Registration {
            id: "jinja-lsp-config-watcher".to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
            register_options: serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                watchers,
            })
            .ok(),
        };
        let _ = self.client.register_capability(vec![registration]).await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// REQ-EDIT-02 / REQ-CFG-11: editor settings changes re-apply the config overlay
    /// and republish diagnostics so open files immediately reflect the new lint config.
    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        if let Ok(overlay) = serde_json::from_value::<crate::config::ConfigOverlay>(params.settings)
        {
            // jinja-lsp-isj4: ConfigOverlay is all-Option with unknown fields ignored,
            // so any JSON payload unrelated to jinja-lsp (or `{}`) deserializes to an
            // empty overlay. Applying it would permanently discard the real
            // initializationOptions overlay that's supposed to be re-applied after
            // every jinja.toml reload (E15 §5.7) — skip instead of treating an empty
            // payload as "clear every setting".
            if overlay.is_empty() {
                return;
            }
            match self.state.write().await.apply_init_options(overlay) {
                Ok(warnings) => {
                    for w in &warnings {
                        tracing::warn!("jinja-lsp config warning: {w:?}");
                    }
                }
                Err(e) => tracing::error!("jinja-lsp config error: {e}"),
            }
            // REQ-CFG-11: republish diagnostics so that lint-rule changes are immediately visible.
            self.republish_all_diagnostics().await;
            // REQ-ARCH-10: a config reload relinks, so derived UI is stale too.
            self.refresh_derived_ui().await;
        }
    }

    /// REQ-ARCH-05 / REQ-EDIT-11: open triggers Pass 1 only for "jinja"/"jinja-html" files.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if !is_jinja_language_id(&params.text_document.language_id) {
            return;
        }
        let key = Self::uri_to_key(&params.text_document.uri);
        tracing::debug!("jinja-lsp: did_open {key}");
        // jinja-lsp-wgi7: a reopened document's version counter is not guaranteed to
        // continue from where it left off before the previous close — most clients
        // restart it (often back to 1). Clear any stale high-water mark left by
        // jinja-lsp-q0aw's monotonic-max guard, or every did_change after reopen
        // would see a lower version than the stale mark and skip its publish forever.
        self.state.write().await.doc_versions.remove(&key);
        self.pass1(&key, &params.text_document.text).await;
        self.publish_file_diagnostics(&key).await;
    }

    /// REQ-ARCH-05 / REQ-EDIT-11: change triggers Pass 1 (full-sync, newest content wins).
    /// Only for documents did_open already accepted (tracked in `state.sources`) — a
    /// document did_open rejected for languageId must not get indexed on its first edit.
    ///
    /// jinja-lsp-q0aw: tower-lsp dispatches notifications concurrently, so two rapid
    /// edits can interleave as pass1(A), pass1(B), publish(B), publish(A) — leaving
    /// stale diagnostics for the newest text. Record this edit's document version and
    /// skip the publish if a newer version has already been recorded by the time
    /// pass1 finishes (that newer call's own publish will show correct diagnostics).
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            let key = Self::uri_to_key(&params.text_document.uri);
            let version = params.text_document.version;
            let tracked = {
                let mut state = self.state.write().await;
                if !state.sources.contains_key(&key) {
                    false
                } else {
                    state
                        .doc_versions
                        .entry(key.clone())
                        .and_modify(|v| *v = (*v).max(version))
                        .or_insert(version);
                    true
                }
            };
            if !tracked {
                return;
            }
            self.pass1(&key, &change.text).await;
            let is_latest =
                self.state.read().await.doc_versions.get(&key).copied() == Some(version);
            if !is_latest {
                return;
            }
            self.publish_file_diagnostics(&key).await;
            self.refresh_derived_ui().await;
        }
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {}

    /// REQ-ARCH-05: close keeps the file in the index; it may still be
    /// referenced by other templates.
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // jinja-lsp-wgi7: this document's LSP editing session has ended, so its
        // version-tracking high-water mark is no longer meaningful — a future
        // did_open starts a fresh version stream. Clearing it here (not just on
        // reopen) also prevents doc_versions growing unboundedly across many
        // open/close cycles over a long-running server session.
        let key = Self::uri_to_key(&params.text_document.uri);
        tracing::debug!("jinja-lsp: did_close {key}");
        self.state.write().await.doc_versions.remove(&key);
    }

    /// REQ-ARCH-06: watched-files dispatch — config and template file changes.
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in &params.changes {
            let key = Self::uri_to_key(&change.uri);
            // REQ-CFG-10: detect config file changes (jinja.toml or pyproject.toml).
            let is_config_file = {
                let state = self.state.read().await;
                state.config_file_path.as_deref() == Some(&key)
                    || key.ends_with("jinja.toml")
                    || key.ends_with("pyproject.toml")
            };
            if is_config_file {
                self.reload_config_file(&key).await;
                continue;
            }
            // REQ-HINT-08: sidecar hint file changed — rebuild the per-template registry overlay
            // and republish diagnostics so the editor sees the updated hint-driven warnings.
            if key.ends_with(".hints.md") {
                if let Some(template_key) = key.strip_suffix(".hints.md") {
                    let base = {
                        let state = self.state.read().await;
                        state.base_registry_for(template_key).clone()
                    };
                    self.state.write().await.refresh_sidecar(template_key, base);
                    self.publish_file_diagnostics(template_key).await;
                }
                continue;
            }
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    // Use tokio::fs to avoid blocking the async executor on disk I/O.
                    if let Ok(source) = tokio::fs::read_to_string(&key).await {
                        self.pass1(&key, &source).await;
                    }
                }
                FileChangeType::DELETED => {
                    {
                        let mut state = self.state.write().await;
                        // REQ-EXTR-08: remove from the correct folder's workspace.
                        let extra_idx = state
                            .extra_folders
                            .iter()
                            .enumerate()
                            .filter(|(_, f)| {
                                crate::server::state::key_under_root(
                                    &key,
                                    f.root.to_str().unwrap_or(""),
                                )
                            })
                            .max_by_key(|(_, f)| f.root.to_str().map(|s| s.len()).unwrap_or(0))
                            .map(|(i, _)| i);
                        // jinja-lsp-7f0o: a deleted file must lose ALL its per-file state, not
                        // just the template entry — otherwise sources grows unboundedly, stale
                        // sidecar overlays/inline sub-entries keep contributing references, and
                        // the editor keeps showing diagnostics for a file that no longer exists.
                        let workspace = if let Some(ei) = extra_idx {
                            &mut state.extra_folders[ei].workspace
                        } else {
                            &mut state.workspace
                        };
                        workspace.templates.remove(&key);
                        workspace.clear_inline_entries_for(&key);
                        state.sources.remove(&key);
                        state.sidecar_registries.remove(&key);
                        // jinja-lsp-wgi7: also drop the version high-water mark so a
                        // future recreate+reopen of this path doesn't inherit a stale one.
                        state.doc_versions.remove(&key);
                    }
                    let uri = path_to_uri(&key);
                    self.client.publish_diagnostics(uri, vec![], None).await;
                }
                _ => {}
            }
        }
    }

    // REQ-ARCH-07: feature handlers are pure reads — stubs for now; each
    // delegates to features::<module>::<fn>(state, params) when implemented.

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let key = Self::uri_to_key(&params.text_document_position.text_document.uri);
        let pos = params.text_document_position.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let byte_col = lsp_char_to_byte_col(
            source_line(source, pos.line),
            pos.character,
            state.position_encoding_utf8,
        );
        let (items, is_incomplete) = complete(
            source,
            pos.line,
            byte_col,
            index,
            state.registry_for(&key),
            workspace,
        );
        if items.is_empty() {
            return Ok(None);
        }
        let lsp_items: Vec<CompletionItem> =
            items.into_iter().map(to_lsp_completion_item).collect();
        if is_incomplete {
            Ok(Some(CompletionResponse::List(CompletionList {
                is_incomplete: true,
                items: lsp_items,
            })))
        } else {
            Ok(Some(CompletionResponse::Array(lsp_items)))
        }
    }

    async fn completion_resolve(&self, mut item: CompletionItem) -> Result<CompletionItem> {
        // REQ-CMP-05: fill documentation lazily from the item's data field.
        // completion_resolve has no document URI, so falls back to the primary registry.
        if let Some(data) = item.data.as_ref().and_then(|d| d.as_str()) {
            let state = self.state.read().await;
            if let Some(doc) = resolve_doc(data, &state.registry) {
                item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc,
                }));
            }
        }
        Ok(item)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(result) = hover_feature(
            source,
            pos.line,
            byte_col,
            index,
            state.registry_for(&key),
            workspace,
        ) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: result.markdown,
            }),
            range: Some(Range {
                start: Position {
                    line: result.start_line,
                    character: byte_col_to_lsp_char(
                        source_line(source, result.start_line),
                        result.start_col,
                        utf8,
                    ),
                },
                end: Position {
                    line: result.end_line,
                    character: byte_col_to_lsp_char(
                        source_line(source, result.end_line),
                        result.end_col,
                        utf8,
                    ),
                },
            }),
        }))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(help) = sig_help_feature(
            source,
            pos.line,
            byte_col,
            index,
            state.registry_for(&key),
            workspace,
        ) else {
            return Ok(None);
        };
        let sig_info = SignatureInformation {
            label: help.label,
            documentation: None,
            parameters: Some(
                help.params
                    .iter()
                    .map(|p| ParameterInformation {
                        label: ParameterLabel::Simple(p.label.clone()),
                        documentation: p
                            .documentation
                            .as_deref()
                            .map(|d| Documentation::String(d.to_owned())),
                    })
                    .collect(),
            ),
            active_parameter: help.active_parameter.map(|i| i as u32),
        };
        Ok(Some(SignatureHelp {
            signatures: vec![sig_info],
            active_signature: Some(0),
            active_parameter: help.active_parameter.map(|i| i as u32),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(loc) = go_to_definition(
            source,
            pos.line,
            byte_col,
            &key,
            index,
            state.registry_for(&key),
            workspace,
        ) else {
            return Ok(None);
        };
        let target_source = state
            .sources
            .get(&loc.target_path)
            .map(|s| s.as_str())
            .unwrap_or("");
        if state.definition_link_support {
            let origin = lsp_range_at_cursor(source, pos.line, pos.character, utf8);
            Ok(Some(GotoDefinitionResponse::Link(vec![
                definition_to_link(target_source, &loc, utf8, Some(origin)),
            ])))
        } else {
            Ok(Some(GotoDefinitionResponse::Scalar(
                definition_to_location(target_source, &loc, utf8),
            )))
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let key = Self::uri_to_key(&params.text_document_position.text_document.uri);
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let locs = find_references(
            source,
            pos.line,
            byte_col,
            &key,
            include_decl,
            index,
            state.registry_for(&key),
            workspace,
        );
        if locs.is_empty() {
            return Ok(None);
        }
        let locations: Vec<Location> = locs
            .iter()
            .map(|r| ref_to_location(r, &state.sources, utf8))
            .collect();
        Ok(Some(locations))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = state.workspace_for(&key).templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let syms = document_symbols(source, index);
        if syms.is_empty() {
            return Ok(None);
        }
        let source = source.clone(); // release borrow on state
        drop(state);
        Ok(Some(DocumentSymbolResponse::Nested(
            syms.into_iter()
                .map(|s| to_lsp_document_symbol(&source, utf8, s))
                .collect(),
        )))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let state = self.state.read().await;
        let utf8 = state.position_encoding_utf8;
        // workspace_symbols searches the primary folder; multi-folder symbol search is a future enhancement.
        let syms = workspace_symbols(&params.query, &state.workspace);
        if syms.is_empty() {
            return Ok(None);
        }
        let result = syms
            .iter()
            .map(|s| ws_to_lsp_symbol(s, &state.sources, utf8))
            .collect();
        Ok(Some(result))
    }

    /// REQ-DIAG-07 / E01 §5.6: pull-mode diagnostics.
    ///
    /// A read of the store that `publish_file_diagnostics` filled, never a recompute —
    /// so pull-mode and push-mode clients cannot disagree, and noqa-suppressed findings
    /// are absent from both. A file the server has not indexed reports an empty set
    /// rather than an error, which is what the protocol expects for an untracked doc.
    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let stored = self.state.read().await.diagnostics.get(&key).cloned();

        // Not yet published (a pull can arrive before the first Pass 1 completes):
        // compute once so the client is not told "no findings" prematurely.
        let items = match stored {
            Some(d) => d,
            None => self
                .compute_file_diagnostics(&key)
                .await
                .unwrap_or_default(),
        };

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            }),
        ))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let highlights =
            document_highlight(source, pos.line, byte_col, index, state.registry_for(&key));
        if highlights.is_empty() {
            return Ok(None);
        }
        let result = highlights
            .iter()
            .map(|h| {
                let kind = match h.kind {
                    HighlightKind::Read => DocumentHighlightKind::READ,
                    HighlightKind::Write => DocumentHighlightKind::WRITE,
                };
                DocumentHighlight {
                    range: span_to_lsp_range(source, &h.range, utf8),
                    kind: Some(kind),
                }
            })
            .collect();
        Ok(Some(result))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let ranges = fold_ranges(source);
        if ranges.is_empty() {
            return Ok(None);
        }
        let result = ranges
            .iter()
            .map(|r| FoldingRange {
                start_line: r.start_line,
                start_character: None,
                end_line: r.end_line,
                end_character: None,
                kind: Some(match r.kind {
                    FoldKind::Region => FoldingRangeKind::Region,
                    FoldKind::Comment => FoldingRangeKind::Comment,
                }),
                collapsed_text: None,
            })
            .collect();
        Ok(Some(result))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let cfg = InlayHintsConfig::default();
        let hints = inlay_hints(
            source,
            &key,
            index,
            state.registry_for(&key),
            workspace,
            &cfg,
        );
        if hints.is_empty() {
            return Ok(None);
        }
        let result = hints
            .iter()
            .map(|h| {
                let data = inlay_hint_data_to_json(&h.data);
                InlayHint {
                    position: Position {
                        line: h.line,
                        character: byte_col_to_lsp_char(source_line(source, h.line), h.col, utf8),
                    },
                    label: InlayHintLabel::String(h.label.clone()),
                    kind: h
                        .kind
                        .as_ref()
                        .map(|_| tower_lsp::lsp_types::InlayHintKind::PARAMETER),
                    tooltip: h
                        .tooltip
                        .as_deref()
                        .map(|t| InlayHintTooltip::String(t.to_owned())),
                    text_edits: None,
                    padding_left: Some(true),
                    padding_right: None,
                    data: Some(data),
                }
            })
            .collect();
        Ok(Some(result))
    }

    async fn inlay_hint_resolve(&self, mut params: InlayHint) -> Result<InlayHint> {
        let Some(data_val) = &params.data else {
            return Ok(params);
        };
        let Some(hint_data) = inlay_hint_data_from_json(data_val) else {
            return Ok(params);
        };
        let path = match &hint_data {
            InlayHintData::Parameter { template_path, .. } => template_path.clone(),
            InlayHintData::EndBlock { template_path, .. } => template_path.clone(),
        };
        let state = self.state.read().await;
        let workspace = state.workspace_for(&path);
        let Some(index) = workspace.templates.get(&path) else {
            return Ok(params);
        };
        // Reconstruct an internal InlayHint with only the data field; resolve fills tooltip.
        let internal = crate::features::inlay_hints::InlayHint {
            line: params.position.line,
            col: 0,
            label: match &params.label {
                InlayHintLabel::String(s) => s.clone(),
                _ => String::new(),
            },
            kind: None,
            tooltip: None,
            data: hint_data,
        };
        let resolved = inlay_hint_resolve(internal, index, state.registry_for(&path), workspace);
        if let Some(tooltip) = resolved.tooltip {
            params.tooltip = Some(InlayHintTooltip::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: tooltip,
            }));
        }
        Ok(params)
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = state.workspace_for(&key).templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let cfg = CodeLensConfig::default();
        let lenses = code_lens_feature(&key, index, &cfg);
        if lenses.is_empty() {
            return Ok(None);
        }
        let result = lenses
            .into_iter()
            .map(|l| {
                let data = lens_data_to_json(&l.data);
                let character = byte_col_to_lsp_char(source_line(source, l.line), l.col, utf8);
                CodeLens {
                    range: Range {
                        start: Position {
                            line: l.line,
                            character,
                        },
                        end: Position {
                            line: l.line,
                            character,
                        },
                    },
                    // l.title is always None here -- code_lens_feature() only anchors
                    // lenses (REQ-LENS-04); code_lens_resolve() fills in the title and
                    // the real navigation command.
                    command: None,
                    data: Some(data),
                }
            })
            .collect();
        Ok(Some(result))
    }

    async fn code_lens_resolve(&self, mut params: CodeLens) -> Result<CodeLens> {
        let Some(data_val) = &params.data else {
            return Ok(params);
        };
        let Some(lens_data) = lens_data_from_json(data_val) else {
            return Ok(params);
        };
        let path = lens_data.file_path.clone();
        let state = self.state.read().await;
        let workspace = state.workspace_for(&path);
        let internal = crate::features::code_lens::CodeLens {
            line: params.range.start.line,
            col: params.range.start.character,
            title: None,
            data: lens_data,
        };
        let resolved = code_lens_resolve_feature(internal, workspace);
        if let Some(title) = resolved.title {
            if !title.is_empty() {
                // jinja-lsp-qpc6: "editor.action.showReferences" is the de facto LSP
                // convention (rust-analyzer, gopls, ...) for a code lens that jumps to
                // one-or-many locations. Zed handles it entirely client-side (see
                // crates/editor/src/code_lens.rs's try_show_references), opening its
                // native multi-location peek UI -- no execute_command round-trip.
                // arguments = [uri, position, locations[]]; Zed only reads index 2,
                // but the first two are included for parity with other clients.
                let targets =
                    crate::features::code_lens::code_lens_targets(&resolved.data, workspace);
                let (command, arguments) = if targets.is_empty() {
                    (String::new(), None)
                } else {
                    let locations: Vec<Location> = targets
                        .iter()
                        .map(|t| {
                            lens_target_to_location(t, &state.sources, state.position_encoding_utf8)
                        })
                        .collect();
                    (
                        "editor.action.showReferences".to_owned(),
                        Some(vec![
                            serde_json::to_value(path_to_uri(&path)).unwrap_or_default(),
                            serde_json::to_value(params.range.start).unwrap_or_default(),
                            serde_json::to_value(locations).unwrap_or_default(),
                        ]),
                    )
                };
                params.command = Some(Command {
                    title,
                    command,
                    arguments,
                });
            }
        }
        Ok(params)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let key = Self::uri_to_key(&params.text_document.uri);

        // Synchronous, read-only work — hold the read guard throughout instead of
        // cloning sources/workspace/registry per request (jinja-lsp-0ar1).
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let registry = state.registry_for(&key);

        // Convert LSP diagnostics to internal ones.
        let diags: Vec<crate::diagnostic::Diagnostic> = params
            .context
            .diagnostics
            .iter()
            .filter_map(|d| from_lsp_diagnostic(d, &key, source, utf8))
            .collect();

        let mut actions = code_actions(source, &key, &diags, index, workspace, registry);

        // REQ-ACT-07 / REQ-ACT-08: when the client sends a non-empty range (selection),
        // also emit refactor actions for wrap and extract-to-macro.
        let range = &params.range;
        if range.start != range.end {
            let sel = selection_code_actions(source, &key, range.start.line, range.end.line);
            actions.extend(sel);
        }

        if actions.is_empty() {
            return Ok(None);
        }

        let lsp_actions: Vec<CodeActionOrCommand> = actions
            .into_iter()
            .map(|a| CodeActionOrCommand::CodeAction(to_lsp_action(a, &key, &state.sources, utf8)))
            .collect();

        Ok(Some(lsp_actions))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "jinja-lsp.extract-macro" => {
                // args[0]: {path, start_line, end_line, name}
                let Some(arg) = params.arguments.first() else {
                    return Ok(None);
                };
                let Some(obj) = arg.as_object() else {
                    return Ok(None);
                };
                let Some(path) = obj.get("path").and_then(|v| v.as_str()) else {
                    return Ok(None);
                };
                let Some(start_line) = obj.get("start_line").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let Some(end_line) = obj.get("end_line").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let Some(name) = obj.get("name").and_then(|v| v.as_str()) else {
                    return Ok(None);
                };
                let state = self.state.read().await;
                let Some(source) = state.sources.get(path) else {
                    return Ok(None);
                };
                let Some(workspace_edit) = crate::features::extract_macro::compute_extract_macro(
                    source,
                    path,
                    start_line as u32,
                    end_line as u32,
                    name,
                ) else {
                    return Ok(None);
                };
                let utf8 = state.position_encoding_utf8;
                let lsp_edit = internal_workspace_edit_to_lsp(workspace_edit, &state.sources, utf8);
                // Drop the read guard before the client round-trip: apply_edit triggers a
                // didChange that needs state.write(), and tokio's write-preferring RwLock
                // would otherwise stall behind this still-held read guard (jinja-lsp-1sjt).
                drop(state);
                let _ = self.client.apply_edit(lsp_edit).await;
                Ok(None)
            }
            // REQ-ACT-07: args[0]: {path, start_line, end_line, name}
            "jinja-lsp.wrap-block" => {
                let Some(arg) = params.arguments.first() else {
                    return Ok(None);
                };
                let Some(obj) = arg.as_object() else {
                    return Ok(None);
                };
                let Some(path) = obj.get("path").and_then(|v| v.as_str()) else {
                    return Ok(None);
                };
                let Some(start_line) = obj.get("start_line").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let Some(end_line) = obj.get("end_line").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("new_block");
                let state = self.state.read().await;
                let Some(source) = state.sources.get(path) else {
                    return Ok(None);
                };
                let Some(workspace_edit) = crate::features::wrap::wrap_selection(
                    source,
                    path,
                    start_line as u32,
                    end_line as u32,
                    crate::features::wrap::WrapKind::Block(name.to_owned()),
                ) else {
                    return Ok(None);
                };
                let utf8 = state.position_encoding_utf8;
                let lsp_edit = internal_workspace_edit_to_lsp(workspace_edit, &state.sources, utf8);
                // Drop the read guard before the client round-trip (jinja-lsp-1sjt).
                drop(state);
                let _ = self.client.apply_edit(lsp_edit).await;
                Ok(None)
            }
            // REQ-ACT-11: args[0]: {path, line, col, new_name}
            "jinja-lsp.rename" => {
                let Some(arg) = params.arguments.first() else {
                    return Ok(None);
                };
                let Some(obj) = arg.as_object() else {
                    return Ok(None);
                };
                let Some(path) = obj.get("path").and_then(|v| v.as_str()) else {
                    return Ok(None);
                };
                let Some(line) = obj.get("line").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let Some(col) = obj.get("col").and_then(|v| v.as_u64()) else {
                    return Ok(None);
                };
                let Some(new_name) = obj.get("new_name").and_then(|v| v.as_str()) else {
                    return Ok(None);
                };
                let state = self.state.read().await;
                let workspace = state.workspace_for(path);
                let Some(source) = state.sources.get(path) else {
                    return Ok(None);
                };
                let Some(index) = workspace.templates.get(path) else {
                    return Ok(None);
                };
                let Some((target, old_name)) =
                    rename_at_cursor(source, path, line as u32, col as u32, index, workspace)
                else {
                    return Ok(None);
                };
                // REQ-ACT-11: validate identifier and check for scope collision before applying.
                if let Some(reason) = check_rename_preconditions(new_name, &target, index) {
                    self.client.show_message(MessageType::WARNING, reason).await;
                    return Ok(None);
                }
                let Some(workspace_edit) = compute_rename(
                    &state.sources,
                    path,
                    &old_name,
                    new_name,
                    target,
                    index,
                    workspace,
                ) else {
                    return Ok(None);
                };
                let utf8 = state.position_encoding_utf8;
                let lsp_edit = internal_workspace_edit_to_lsp(workspace_edit, &state.sources, utf8);
                // Drop the read guard before the client round-trip (jinja-lsp-1sjt).
                drop(state);
                let _ = self.client.apply_edit(lsp_edit).await;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    async fn code_action_resolve(&self, params: CodeAction) -> Result<CodeAction> {
        // Our code actions ship with full edits on creation, so no lazy resolve is needed.
        Ok(params)
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let items = prepare_call_hierarchy(
            source,
            pos.line,
            byte_col,
            &key,
            index,
            workspace,
            state.registry_for(&key),
        );
        if items.is_empty() {
            return Ok(None);
        }
        let result = items
            .iter()
            .map(|i| internal_item_to_lsp(i, &state.sources, utf8))
            .collect();
        Ok(Some(result))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let Some(item) = lsp_item_to_internal(&params.item) else {
            return Ok(None);
        };
        let state = self.state.read().await;
        // incoming/outgoing_calls work on the primary workspace; multi-folder call hierarchy is a future enhancement.
        let calls = incoming_calls(&item, &state.workspace);
        if calls.is_empty() {
            return Ok(None);
        }
        let utf8 = state.position_encoding_utf8;
        let result = calls
            .iter()
            .map(|c| {
                let src = state
                    .sources
                    .get(&c.from.uri)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                CallHierarchyIncomingCall {
                    from: internal_item_to_lsp(&c.from, &state.sources, utf8),
                    from_ranges: c
                        .from_ranges
                        .iter()
                        .map(|r| hr_to_range(r, src, utf8))
                        .collect(),
                }
            })
            .collect();
        Ok(Some(result))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let Some(item) = lsp_item_to_internal(&params.item) else {
            return Ok(None);
        };
        let state = self.state.read().await;
        let calls = outgoing_calls(&item, &state.workspace, &state.registry); // primary workspace
        if calls.is_empty() {
            return Ok(None);
        }
        let utf8 = state.position_encoding_utf8;
        // from_ranges are call sites within item.uri (the caller template).
        let caller_src = state
            .sources
            .get(&item.uri)
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_owned();
        let result = calls
            .iter()
            .map(|c| CallHierarchyOutgoingCall {
                to: internal_item_to_lsp(&c.to, &state.sources, utf8),
                from_ranges: c
                    .from_ranges
                    .iter()
                    .map(|r| hr_to_range(r, &caller_src, utf8))
                    .collect(),
            })
            .collect();
        Ok(Some(result))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let tokens = stok_full(source, index, state.registry_for(&key), workspace);
        if tokens.is_empty() {
            return Ok(None);
        }
        let data = tokens_to_lsp_data(&tokens, source, state.position_encoding_utf8);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let workspace = state.workspace_for(&key);
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let Some(index) = workspace.templates.get(&key) else {
            return Ok(None);
        };
        let tokens = stok_range(
            source,
            params.range.start.line,
            params.range.end.line,
            index,
            state.registry_for(&key),
            workspace,
        );
        if tokens.is_empty() {
            return Ok(None);
        }
        let data = tokens_to_lsp_data(&tokens, source, state.position_encoding_utf8);
        Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let opts = crate::features::formatting::FormatOptions {
            tab_size: params.options.tab_size,
            insert_spaces: params.options.insert_spaces,
        };
        // REQ-FMT-07/jinja-lsp-qupa: honor jinja.toml [format] options (space_around_pipe,
        // preferred_quote, …), overriding only tab_size/insert_spaces from the LSP request.
        let config = opts.merge_into(&state.config_for(&key).format);
        let utf8 = state.position_encoding_utf8;
        let edits = crate::features::formatting::format_document_with_config(source, &config);
        if edits.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            edits
                .into_iter()
                .map(|e| to_lsp_edit(e, source, utf8))
                .collect(),
        ))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else {
            return Ok(None);
        };
        let range = params.range;
        let opts = crate::features::formatting::FormatOptions {
            tab_size: params.options.tab_size,
            insert_spaces: params.options.insert_spaces,
        };
        let config = opts.merge_into(&state.config_for(&key).format);
        let utf8 = state.position_encoding_utf8;
        let edits = crate::features::formatting::format_range_with_config(
            source,
            range.start.line,
            range.end.line,
            &config,
        );
        if edits.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            edits
                .into_iter()
                .map(|e| to_lsp_edit(e, source, utf8))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use crate::features::semantic_tokens::SemanticToken as IST;

    #[test]
    fn is_jinja_language_id_accepts_canonical_ids() {
        for id in [
            "jinja",
            "jinja-html",
            "jinja-yaml",
            "jinja-json",
            "jinja-markdown",
            "jinja-css",
            "jinja-python",
            "jinja-javascript",
            "jinja-typescript",
            "jinja-shell",
            "jinja-sql",
            "jinja-toml",
            "jinja-xml",
            "jinja-go",
            "jinja-rust",
            "jinja-lua",
            "jinja-ini",
            "jinja-dotenv",
            "jinja-dockerfile",
            "jinja-makefile",
            "jinja-systemd",
            "jinja-hcl",
            "jinja-graphql",
            "jinja-plaintext",
        ] {
            assert!(is_jinja_language_id(id), "expected {id:?} to be accepted");
        }
    }

    /// jinja-lsp-fxse: Zed's default languageId for a language with no explicit
    /// `[language_servers.<id>.language_ids]` override in extension.toml is just
    /// `name.to_lowercase()` (confirmed against Zed's language_name.rs). For "Jinja2
    /// (HTML)" that default is "jinja2 (html)", not "jinja-html" — so extension.toml
    /// MUST declare an explicit language_ids mapping to "jinja-html", or every real
    /// didOpen is silently rejected here and live editing never gets diagnostics.
    #[test]
    fn is_jinja_language_id_rejects_zed_unmapped_default() {
        assert!(!is_jinja_language_id("jinja2 (html)"));
    }

    #[test]
    fn delta_encoding_two_tokens_same_line() {
        // Two tokens on line 0: x at col 3 (len 1), y at col 7 (len 1).
        let tokens = vec![
            IST {
                line: 0,
                start_char: 3,
                length: 1,
                token_type: 1,
                token_modifiers: 0,
            },
            IST {
                line: 0,
                start_char: 7,
                length: 1,
                token_type: 1,
                token_modifiers: 0,
            },
        ];
        let data = tokens_to_lsp_data(&tokens, "{{ x }} {{ y }}", true);
        assert_eq!(data[0].delta_line, 0);
        assert_eq!(data[0].delta_start, 3, "first token at col 3");
        assert_eq!(data[0].length, 1);
        assert_eq!(data[1].delta_line, 0);
        assert_eq!(
            data[1].delta_start, 4,
            "second token: delta 7-3=4 from first"
        );
    }

    #[test]
    fn delta_encoding_two_tokens_diff_lines() {
        let tokens = vec![
            IST {
                line: 0,
                start_char: 3,
                length: 1,
                token_type: 1,
                token_modifiers: 0,
            },
            IST {
                line: 1,
                start_char: 5,
                length: 2,
                token_type: 3,
                token_modifiers: 4,
            },
        ];
        let src = "{{ x }}\n{{ ab | f }}";
        let data = tokens_to_lsp_data(&tokens, src, true);
        assert_eq!(data[0].delta_line, 0);
        assert_eq!(data[0].delta_start, 3);
        assert_eq!(data[1].delta_line, 1, "line jumped by 1");
        assert_eq!(data[1].delta_start, 5, "absolute col on new line");
        assert_eq!(data[1].token_type, 3);
        assert_eq!(data[1].token_modifiers_bitset, 4);
    }

    #[test]
    fn utf16_length_for_ascii_equals_byte_length() {
        let tokens = vec![IST {
            line: 0,
            start_char: 3,
            length: 5,
            token_type: 0,
            token_modifiers: 0,
        }];
        let src = "{{ hello }}";
        let data_utf8 = tokens_to_lsp_data(&tokens, src, true);
        let data_utf16 = tokens_to_lsp_data(&tokens, src, false);
        assert_eq!(data_utf8[0].length, 5, "UTF-8 mode: length=5 (bytes)");
        assert_eq!(
            data_utf16[0].length, 5,
            "UTF-16 mode: ASCII length same as byte length"
        );
    }

    // REQ-ACT-10: to_lsp_action must propagate diagnostics from internal action.
    #[test]
    fn oeph_to_lsp_action_propagates_diagnostics() {
        use crate::diagnostic::{Diagnostic as InternalDiag, DiagnosticSeverity};
        use crate::features::code_actions::{ActionKind, CodeAction as InternalAction};
        use std::collections::HashMap;

        let src = "{{ content }}".to_owned();
        let mut sources = HashMap::new();
        sources.insert("t.html".to_owned(), src);

        let diag = InternalDiag {
            file: "t.html".to_owned(),
            line: 0,
            col: 3,
            code: "JINJA-W203".to_owned(),
            slug: "unused-import".to_owned(),
            severity: DiagnosticSeverity::Warning,
            message: "unused import".to_owned(),
        };

        let action = InternalAction {
            title: "Remove import".to_owned(),
            kind: ActionKind::QuickFix,
            diagnostics: vec![diag],
            is_preferred: true,
            edit: None,
            command: None,
        };

        let lsp = to_lsp_action(action, "t.html", &sources, true);
        let linked = lsp
            .diagnostics
            .expect("diagnostics must be Some for a QuickFix");
        assert_eq!(linked.len(), 1, "must carry one linked diagnostic");
        assert_eq!(
            linked[0].code,
            Some(tower_lsp::lsp_types::NumberOrString::String(
                "JINJA-W203".to_owned()
            )),
            "linked diagnostic code must match"
        );
    }

    // REQ-ACT-10: refactor actions with no diagnostics must have diagnostics=None.
    #[test]
    fn oeph_to_lsp_action_refactor_has_no_diagnostics() {
        use crate::features::code_actions::{ActionKind, CodeAction as InternalAction};
        use std::collections::HashMap;

        let action = InternalAction {
            title: "Wrap in if".to_owned(),
            kind: ActionKind::RefactorRewrite,
            diagnostics: vec![],
            is_preferred: false,
            edit: None,
            command: None,
        };

        let lsp = to_lsp_action(action, "t.html", &HashMap::new(), true);
        assert!(
            lsp.diagnostics.is_none(),
            "refactor actions must not carry diagnostics"
        );
    }

    // jinja-lsp-c8b7: from_lsp_diagnostic must convert the UTF-16 `character` column
    // to a byte column so code-action handlers building TextEdits from diag.col land
    // at the right offset on lines with non-ASCII text before the symbol.
    #[test]
    fn c8b7_from_lsp_diagnostic_converts_utf16_col_to_byte_col() {
        // "café " is 5 chars / 6 bytes (é is 2 bytes) — "foo" starts at UTF-16 char 5
        // but byte 6.
        let source = "café foo";
        let lsp_diag = LspDiagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 5,
                },
                end: Position {
                    line: 0,
                    character: 8,
                },
            },
            code: Some(NumberOrString::String("JINJA-E103".to_owned())),
            message: "undefined function 'foo'".to_owned(),
            ..Default::default()
        };
        let diag =
            from_lsp_diagnostic(&lsp_diag, "t.html", source, false).expect("code must be Some");
        assert_eq!(
            diag.col, 6,
            "byte col for 'foo' after non-ASCII 'café ' must be 6, not the raw UTF-16 char 5"
        );
    }
}
