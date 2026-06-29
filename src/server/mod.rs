// REQ-ARCH-01..08: jinja-lsp LSP server — one binary, three front-ends over
// one shared two-pass pipeline.

pub mod state;

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::{
    jsonrpc::Result,
    lsp_types::*,
    Client, LanguageServer, LspService, Server,
};

use crate::features::code_actions::{code_actions, selection_code_actions, ActionKind, CodeAction as InternalCodeAction};
use crate::diagnostic::DiagnosticSeverity as InternalSeverity;
use tower_lsp::lsp_types::Diagnostic as LspDiagnostic;
use crate::diagnostics::{filter_by_config, suppress_by_noqa};
use crate::diagnostics::checks::run_checks;
use crate::features::completions::{complete, resolve_doc, CompletionKind};
use crate::features::definition::{go_to_definition, DefinitionLocation};
use crate::features::references::{find_references, ReferenceLocation};
use crate::features::document_highlight::{document_highlight, HighlightKind};
use crate::features::folding::{fold_ranges, FoldKind};
use crate::features::signature_help::signature_help as sig_help_feature;
use crate::features::inlay_hints::{inlay_hints, inlay_hint_resolve, InlayHintData, InlayHintsConfig};
use crate::features::code_lens::{code_lens as code_lens_feature, code_lens_resolve as code_lens_resolve_feature, CodeLensConfig, LensData, LensKind, LensSymbolKind};
use crate::features::call_hierarchy::{prepare_call_hierarchy, incoming_calls, outgoing_calls, CallHierarchyItem as InternalCallHierarchyItem, ItemKind, HierarchyRange};
use crate::features::hover::hover as hover_feature;
use crate::features::formatting::{format_document, format_range};
use crate::features::symbols::{document_symbols, workspace_symbols, SymbolKind as InternalSymbolKind, WorkspaceSymbol as InternalWorkspaceSymbol};
use state::ServerState;

/// REQ-ARCH-02: direct all tracing output to stderr; never stdout (stdout
/// carries JSON-RPC framing).
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
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

    /// Pass 2 (REQ-ARCH-04 / REQ-EXTR-06): relink the workspace — build the
    /// import graph and assemble template chains.  Generation-guarded: if Pass 1
    /// runs while the blocking relink is in progress the stale result is discarded.
    async fn pass2(&self) {
        let (gen_snapshot, workspace_snapshot) = {
            let state = self.state.read().await;
            (state.generation, state.workspace.clone())
        };

        let relinked = tokio::task::spawn_blocking(move || {
            let mut ws = workspace_snapshot;
            ws.relink();
            ws
        })
        .await
        .ok();

        if let Some(relinked) = relinked {
            let mut state = self.state.write().await;
            if state.generation == gen_snapshot {
                state.workspace = relinked;
            }
        }
    }

    fn uri_to_key(uri: &Url) -> String {
        uri.path().to_owned()
    }

    /// Run checks on one file and push findings to the client (REQ-DIAG / F01).
    async fn publish_file_diagnostics(&self, key: &str) {
        let state = self.state.read().await;
        let (Some(source), Some(index)) = (state.sources.get(key), state.workspace.templates.get(key))
        else {
            return;
        };
        let raw = run_checks(source, key, index, &state.registry, &state.workspace);
        let select: Vec<&str> = state.config.lint.select.iter().map(|s| s.as_str()).collect();
        let ignore: Vec<&str> = state.config.lint.ignore.iter().map(|s| s.as_str()).collect();
        let filtered: Vec<crate::diagnostic::Diagnostic> =
            filter_by_config(&raw, &select, &ignore).into_iter().cloned().collect();
        let (kept, w107s) = suppress_by_noqa(&filtered, source);
        let utf8 = state.position_encoding_utf8;
        let mut lsp_diags: Vec<LspDiagnostic> =
            kept.into_iter().chain(w107s).map(|d| to_lsp_diagnostic(source, utf8, &d)).collect();
        lsp_diags.sort_by_key(|d| (d.range.start.line, d.range.start.character));
        let uri = path_to_uri(key);
        drop(state);
        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// REQ-ARCH-08: declare capabilities matching the feature set.
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // REQ-DEF-07: record whether the client supports LocationLink for goto_definition.
        let link_support = params.capabilities.text_document.as_ref()
            .and_then(|td| td.definition.as_ref())
            .and_then(|d| d.link_support)
            .unwrap_or(false);
        // jinja-lsp-7b7s: negotiate UTF-8 position encoding when the client supports it.
        // Our internal offsets are byte-based (tree-sitter, Rust str), which equals
        // UTF-8 code units, so UTF-8 clients need no conversion at all.
        let utf8 = params.capabilities.general.as_ref()
            .and_then(|g| g.position_encodings.as_ref())
            .map(|encs| encs.contains(&PositionEncodingKind::UTF8))
            .unwrap_or(false);
        // REQ-EDIT-10 / REQ-CFG-07: apply InitializationOptions overlay and validate.
        {
            let mut state = self.state.write().await;
            state.definition_link_support = link_support;
            state.position_encoding_utf8 = utf8;
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
                position_encoding: if utf8 { Some(PositionEncodingKind::UTF8) } else { None },
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
                        "{".into(), "%".into(), " ".into(), "|".into(),
                        ".".into(), "(".into(), ",".into(), "\"".into(),
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
                                token_types: vec![],
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(true),
                            ..Default::default()
                        },
                    ),
                ),
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
                    commands: vec!["jinja-lsp.extract-macro".to_owned()],
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
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// REQ-EDIT-02: editor settings changes re-apply the config overlay.
    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        if let Ok(overlay) = serde_json::from_value::<crate::config::ConfigOverlay>(params.settings) {
            match self.state.write().await.apply_init_options(overlay) {
                Ok(warnings) => {
                    for w in &warnings {
                        tracing::warn!("jinja-lsp config warning: {w:?}");
                    }
                }
                Err(e) => tracing::error!("jinja-lsp config error: {e}"),
            }
        }
    }

    /// REQ-ARCH-05 / REQ-EDIT-11: open triggers Pass 1 only for Jinja language IDs.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let lang = params.text_document.language_id.as_str();
        if lang != "jinja" && lang != "jinja-html" {
            return;
        }
        let key = Self::uri_to_key(&params.text_document.uri);
        self.pass1(&key, &params.text_document.text).await;
        self.publish_file_diagnostics(&key).await;
    }

    /// REQ-ARCH-05: change triggers Pass 1 (full-sync, newest content wins).
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            let key = Self::uri_to_key(&params.text_document.uri);
            self.pass1(&key, &change.text).await;
            self.publish_file_diagnostics(&key).await;
        }
    }

    /// REQ-ARCH-05: save triggers Pass 2 (relink).
    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
        self.pass2().await;
    }

    /// REQ-ARCH-05: close keeps the file in the index; it may still be
    /// referenced by other templates.
    async fn did_close(&self, _params: DidCloseTextDocumentParams) {}

    /// REQ-ARCH-06: watched-files dispatch — config and template file changes.
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in &params.changes {
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    // template file: Pass 1 + schedule Pass 2
                    let key = Self::uri_to_key(&change.uri);
                    // read from disk — watched files aren't open buffers
                    if let Ok(source) = std::fs::read_to_string(change.uri.path()) {
                        self.pass1(&key, &source).await;
                        self.pass2().await;
                    }
                }
                FileChangeType::DELETED => {
                    let key = Self::uri_to_key(&change.uri);
                    self.state.write().await.workspace.templates.remove(&key);
                }
                _ => {}
            }
        }
    }

    // REQ-ARCH-07: feature handlers are pure reads — stubs for now; each
    // delegates to features::<module>::<fn>(state, params) when implemented.

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let key = Self::uri_to_key(&params.text_document_position.text_document.uri);
        let pos = params.text_document_position.position;
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, state.position_encoding_utf8);
        let items = complete(source, pos.line, byte_col, index, &state.registry, &state.workspace);
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(
            items.into_iter().map(to_lsp_completion_item).collect(),
        )))
    }

    async fn completion_resolve(&self, mut item: CompletionItem) -> Result<CompletionItem> {
        // REQ-CMP-05: fill documentation lazily from the item's data field.
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
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(result) = hover_feature(source, pos.line, byte_col, index, &state.registry, &state.workspace)
        else {
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
                    character: byte_col_to_lsp_char(source_line(source, result.start_line), result.start_col, utf8),
                },
                end: Position {
                    line: result.end_line,
                    character: byte_col_to_lsp_char(source_line(source, result.end_line), result.end_col, utf8),
                },
            }),
        }))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(help) = sig_help_feature(source, pos.line, byte_col, index, &state.registry, &state.workspace) else {
            return Ok(None);
        };
        let sig_info = SignatureInformation {
            label: help.label,
            documentation: None,
            parameters: Some(help.params.iter().map(|p| ParameterInformation {
                label: ParameterLabel::Simple(p.label.clone()),
                documentation: p.documentation.as_deref().map(|d| Documentation::String(d.to_owned())),
            }).collect()),
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
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let Some(loc) = go_to_definition(source, pos.line, byte_col, &key, index, &state.registry, &state.workspace)
        else {
            return Ok(None);
        };
        let target_source = state.sources.get(&loc.target_path).map(|s| s.as_str()).unwrap_or("");
        if state.definition_link_support {
            Ok(Some(GotoDefinitionResponse::Link(vec![definition_to_link(target_source, &loc, utf8)])))
        } else {
            Ok(Some(GotoDefinitionResponse::Scalar(definition_to_location(target_source, &loc, utf8))))
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let key = Self::uri_to_key(&params.text_document_position.text_document.uri);
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let locs = find_references(source, pos.line, byte_col, &key, include_decl, index, &state.registry, &state.workspace);
        if locs.is_empty() { return Ok(None); }
        let locations: Vec<Location> = locs.iter().map(|r| ref_to_location(r, &state.sources, utf8)).collect();
        Ok(Some(locations))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let syms = document_symbols(source, index);
        if syms.is_empty() {
            return Ok(None);
        }
        let source = source.clone(); // release borrow on state
        drop(state);
        Ok(Some(DocumentSymbolResponse::Nested(
            syms.into_iter().map(|s| to_lsp_document_symbol(&source, utf8, s)).collect(),
        )))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let state = self.state.read().await;
        let utf8 = state.position_encoding_utf8;
        let syms = workspace_symbols(&params.query, &state.workspace);
        if syms.is_empty() { return Ok(None); }
        let result = syms.iter().map(|s| ws_to_lsp_symbol(s, &state.sources, utf8)).collect();
        Ok(Some(result))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let key = Self::uri_to_key(&params.text_document_position_params.text_document.uri);
        let pos = params.text_document_position_params.position;
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let highlights = document_highlight(source, pos.line, byte_col, index, &state.registry);
        if highlights.is_empty() { return Ok(None); }
        let result = highlights.iter().map(|h| {
            let kind = match h.kind {
                HighlightKind::Read => DocumentHighlightKind::READ,
                HighlightKind::Write => DocumentHighlightKind::WRITE,
            };
            DocumentHighlight {
                range: span_to_lsp_range(source, &h.range, utf8),
                kind: Some(kind),
            }
        }).collect();
        Ok(Some(result))
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> Result<Option<Vec<FoldingRange>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let ranges = fold_ranges(source);
        if ranges.is_empty() { return Ok(None); }
        let result = ranges.iter().map(|r| FoldingRange {
            start_line: r.start_line,
            start_character: None,
            end_line: r.end_line,
            end_character: None,
            kind: Some(match r.kind {
                FoldKind::Region => FoldingRangeKind::Region,
                FoldKind::Comment => FoldingRangeKind::Comment,
            }),
            collapsed_text: None,
        }).collect();
        Ok(Some(result))
    }

    async fn inlay_hint(
        &self,
        params: InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let cfg = InlayHintsConfig::default();
        let hints = inlay_hints(source, &key, index, &state.registry, &state.workspace, &cfg);
        if hints.is_empty() { return Ok(None); }
        let result = hints.iter().map(|h| {
            let data = inlay_hint_data_to_json(&h.data);
            InlayHint {
                position: Position {
                    line: h.line,
                    character: byte_col_to_lsp_char(source_line(source, h.line), h.col, utf8),
                },
                label: InlayHintLabel::String(h.label.clone()),
                kind: h.kind.as_ref().map(|_| tower_lsp::lsp_types::InlayHintKind::PARAMETER),
                tooltip: h.tooltip.as_deref().map(|t| InlayHintTooltip::String(t.to_owned())),
                text_edits: None,
                padding_left: Some(true),
                padding_right: None,
                data: Some(data),
            }
        }).collect();
        Ok(Some(result))
    }

    async fn inlay_hint_resolve(&self, mut params: InlayHint) -> Result<InlayHint> {
        let Some(data_val) = &params.data else { return Ok(params) };
        let Some(hint_data) = inlay_hint_data_from_json(data_val) else { return Ok(params) };
        let path = match &hint_data {
            InlayHintData::Parameter { template_path, .. } => template_path.clone(),
            InlayHintData::EndBlock { template_path, .. } => template_path.clone(),
        };
        let state = self.state.read().await;
        let Some(index) = state.workspace.templates.get(&path) else { return Ok(params) };
        // Reconstruct an internal InlayHint with only the data field; resolve fills tooltip.
        let internal = crate::features::inlay_hints::InlayHint {
            line: params.position.line,
            col: 0,
            label: match &params.label { InlayHintLabel::String(s) => s.clone(), _ => String::new() },
            kind: None,
            tooltip: None,
            data: hint_data,
        };
        let resolved = inlay_hint_resolve(internal, index, &state.registry, &state.workspace);
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
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let cfg = CodeLensConfig::default();
        let lenses = code_lens_feature(&key, index, &cfg);
        if lenses.is_empty() { return Ok(None); }
        let result = lenses.into_iter().map(|l| {
            let data = lens_data_to_json(&l.data);
            CodeLens {
                range: Range {
                    start: Position { line: l.line, character: l.col },
                    end: Position { line: l.line, character: l.col },
                },
                command: l.title.map(|title| Command {
                    title,
                    command: String::new(),
                    arguments: None,
                }),
                data: Some(data),
            }
        }).collect();
        Ok(Some(result))
    }

    async fn code_lens_resolve(&self, mut params: CodeLens) -> Result<CodeLens> {
        let Some(data_val) = &params.data else { return Ok(params) };
        let Some(lens_data) = lens_data_from_json(data_val) else { return Ok(params) };
        let path = lens_data.file_path.clone();
        let state = self.state.read().await;
        let internal = crate::features::code_lens::CodeLens {
            line: params.range.start.line,
            col: params.range.start.character,
            title: None,
            data: lens_data,
        };
        let resolved = code_lens_resolve_feature(internal, &state.workspace);
        if let Some(title) = resolved.title {
            if !title.is_empty() {
                params.command = Some(Command { title, command: String::new(), arguments: None });
            }
        }
        drop(path);
        Ok(params)
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };

        // Convert LSP diagnostics to internal ones.
        let diags: Vec<crate::diagnostic::Diagnostic> = params.context.diagnostics
            .iter()
            .filter_map(|d| {
                let code = match &d.code {
                    Some(NumberOrString::String(s)) => s.clone(),
                    _ => return None,
                };
                Some(crate::diagnostic::Diagnostic {
                    code,
                    slug: String::new(),
                    message: d.message.clone(),
                    file: key.clone(),
                    line: d.range.start.line,
                    col: d.range.start.character,
                    severity: crate::diagnostic::DiagnosticSeverity::Warning,
                })
            })
            .collect();

        let mut actions = code_actions(source, &key, &diags, index, &state.workspace, &state.registry);

        // REQ-ACT-07 / REQ-ACT-08: when the client sends a non-empty range (selection),
        // also emit refactor actions for wrap and extract-to-macro.
        let range = &params.range;
        if range.start != range.end {
            let sel = selection_code_actions(
                source,
                &key,
                range.start.line,
                range.end.line,
            );
            actions.extend(sel);
        }

        if actions.is_empty() {
            return Ok(None);
        }

        let lsp_actions: Vec<CodeActionOrCommand> = actions
            .into_iter()
            .map(|a| CodeActionOrCommand::CodeAction(to_lsp_action(a, &key)))
            .collect();

        Ok(Some(lsp_actions))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "jinja-lsp.extract-macro" => {
                // args[0]: {path, start_line, end_line, name}
                let Some(arg) = params.arguments.first() else { return Ok(None) };
                let Some(obj) = arg.as_object() else { return Ok(None) };
                let Some(path) = obj.get("path").and_then(|v| v.as_str()) else { return Ok(None) };
                let Some(start_line) = obj.get("start_line").and_then(|v| v.as_u64()) else { return Ok(None) };
                let Some(end_line) = obj.get("end_line").and_then(|v| v.as_u64()) else { return Ok(None) };
                let Some(name) = obj.get("name").and_then(|v| v.as_str()) else { return Ok(None) };
                let state = self.state.read().await;
                let Some(source) = state.sources.get(path) else { return Ok(None) };
                let Some(workspace_edit) = crate::features::extract_macro::compute_extract_macro(source, path, start_line as u32, end_line as u32, name) else {
                    return Ok(None);
                };
                let lsp_edit = internal_workspace_edit_to_lsp(workspace_edit);
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
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let utf8 = state.position_encoding_utf8;
        let byte_col = lsp_char_to_byte_col(source_line(source, pos.line), pos.character, utf8);
        let items = prepare_call_hierarchy(source, pos.line, byte_col, &key, index, &state.workspace, &state.registry);
        if items.is_empty() { return Ok(None); }
        let result = items.iter().map(|i| internal_item_to_lsp(i, utf8)).collect();
        Ok(Some(result))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let Some(item) = lsp_item_to_internal(&params.item) else { return Ok(None) };
        let state = self.state.read().await;
        let calls = incoming_calls(&item, &state.workspace);
        if calls.is_empty() { return Ok(None); }
        let utf8 = state.position_encoding_utf8;
        let result = calls.iter().map(|c| CallHierarchyIncomingCall {
            from: internal_item_to_lsp(&c.from, utf8),
            from_ranges: c.from_ranges.iter().map(|r| hr_to_range(r)).collect(),
        }).collect();
        Ok(Some(result))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let Some(item) = lsp_item_to_internal(&params.item) else { return Ok(None) };
        let state = self.state.read().await;
        let calls = outgoing_calls(&item, &state.workspace, &state.registry);
        if calls.is_empty() { return Ok(None); }
        let utf8 = state.position_encoding_utf8;
        let result = calls.iter().map(|c| CallHierarchyOutgoingCall {
            to: internal_item_to_lsp(&c.to, utf8),
            from_ranges: c.from_ranges.iter().map(|r| hr_to_range(r)).collect(),
        }).collect();
        Ok(Some(result))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let opts = crate::features::formatting::FormatOptions {
            tab_size: params.options.tab_size,
            insert_spaces: params.options.insert_spaces,
        };
        let edits = format_document(source, opts);
        if edits.is_empty() { return Ok(None); }
        Ok(Some(edits.into_iter().map(to_lsp_edit).collect()))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let range = params.range;
        let opts = crate::features::formatting::FormatOptions {
            tab_size: params.options.tab_size,
            insert_spaces: params.options.insert_spaces,
        };
        let edits = format_range(source, range.start.line, range.end.line, opts);
        if edits.is_empty() { return Ok(None); }
        Ok(Some(edits.into_iter().map(to_lsp_edit).collect()))
    }
}

// ── Position encoding helpers (jinja-lsp-7b7s) ───────────────────────────────

/// Convert an inbound LSP `character` value to a byte column within `line_str`.
///
/// LSP defaults to UTF-16 code units; when UTF-8 was negotiated the character
/// value is already a byte offset, so this is a no-op.
pub fn lsp_char_to_byte_col(line_str: &str, lsp_char: u32, utf8: bool) -> u32 {
    if utf8 {
        return lsp_char;
    }
    // UTF-16 → byte: walk chars, counting UTF-16 code units until we reach lsp_char.
    let mut utf16 = 0u32;
    let mut byte = 0u32;
    for c in line_str.chars() {
        if utf16 >= lsp_char {
            break;
        }
        utf16 += c.len_utf16() as u32;
        byte += c.len_utf8() as u32;
    }
    byte
}

/// Convert an outbound byte column to an LSP `character` value.
///
/// When UTF-8 was negotiated the byte value is used as-is; otherwise it is
/// converted to UTF-16 code units.
pub fn byte_col_to_lsp_char(line_str: &str, byte_col: u32, utf8: bool) -> u32 {
    if utf8 {
        return byte_col;
    }
    let safe = byte_col.min(line_str.len() as u32) as usize;
    line_str[..safe].chars().map(|c| c.len_utf16() as u32).sum()
}

/// Borrow the Nth line from `source` (empty string when out of bounds).
fn source_line(source: &str, line: u32) -> &str {
    source.split('\n').nth(line as usize).unwrap_or("")
}

fn path_to_uri(path: &str) -> Url {
    if path.starts_with('/') {
        Url::parse(&format!("file://{path}")).unwrap_or_else(|_| Url::parse("file:///unknown").expect("constant fallback URL must parse"))
    } else {
        Url::parse(&format!("file:///{path}")).unwrap_or_else(|_| Url::parse("file:///unknown").expect("constant fallback URL must parse"))
    }
}

fn internal_workspace_edit_to_lsp(we: crate::edit::WorkspaceEdit) -> WorkspaceEdit {
    if we.create_files.is_empty() {
        let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> = std::collections::HashMap::new();
        for (path, edits) in we.changes {
            changes.insert(path_to_uri(&path), edits.into_iter().map(to_lsp_edit).collect());
        }
        WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None }
    } else {
        let mut ops: Vec<DocumentChangeOperation> = Vec::new();
        for (path, content) in we.create_files {
            let uri = path_to_uri(&path);
            ops.push(DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
                uri: uri.clone(),
                options: None,
                annotation_id: None,
            })));
            if !content.is_empty() {
                ops.push(DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
                    edits: vec![OneOf::Left(TextEdit {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 0 },
                        },
                        new_text: content,
                    })],
                }));
            }
        }
        for (path, edits) in we.changes {
            let uri = path_to_uri(&path);
            for e in edits {
                ops.push(DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier { uri: uri.clone(), version: None },
                    edits: vec![OneOf::Left(to_lsp_edit(e))],
                }));
            }
        }
        WorkspaceEdit { changes: None, document_changes: Some(DocumentChanges::Operations(ops)), change_annotations: None }
    }
}

fn to_lsp_action(action: InternalCodeAction, _file_uri: &str) -> CodeAction {
    let kind = Some(match action.kind {
        ActionKind::QuickFix => CodeActionKind::QUICKFIX,
        ActionKind::RefactorExtract => CodeActionKind::REFACTOR_EXTRACT,
        ActionKind::RefactorRewrite => CodeActionKind::REFACTOR_REWRITE,
    });

    let edit = action.edit.map(internal_workspace_edit_to_lsp);

    CodeAction {
        title: action.title,
        kind,
        diagnostics: None,
        edit,
        command: None,
        is_preferred: Some(action.is_preferred),
        disabled: None,
        data: None,
    }
}

fn to_lsp_diagnostic(source: &str, utf8: bool, d: &crate::diagnostic::Diagnostic) -> LspDiagnostic {
    let severity = Some(match d.severity {
        InternalSeverity::Error => DiagnosticSeverity::ERROR,
        InternalSeverity::Warning => DiagnosticSeverity::WARNING,
        InternalSeverity::Info => DiagnosticSeverity::INFORMATION,
        InternalSeverity::Hint => DiagnosticSeverity::HINT,
    });
    let col = byte_col_to_lsp_char(source_line(source, d.line), d.col, utf8);
    LspDiagnostic {
        range: Range {
            start: Position { line: d.line, character: col },
            end: Position { line: d.line, character: col + 1 },
        },
        severity,
        code: Some(NumberOrString::String(d.code.clone())),
        source: Some("jinja-lsp".to_owned()),
        message: d.message.clone(),
        ..Default::default()
    }
}

fn to_lsp_completion_item(item: crate::features::completions::CompletionItem) -> CompletionItem {
    let kind = Some(match item.kind {
        CompletionKind::Filter => CompletionItemKind::FUNCTION,
        CompletionKind::Function => CompletionItemKind::FUNCTION,
        CompletionKind::Test => CompletionItemKind::FUNCTION,
        CompletionKind::Variable => CompletionItemKind::VARIABLE,
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::TemplatePath => CompletionItemKind::FILE,
        CompletionKind::Attribute => CompletionItemKind::FIELD,
        CompletionKind::KeywordArg => CompletionItemKind::PROPERTY,
    });
    CompletionItem {
        label: item.label,
        kind,
        detail: item.detail,
        documentation: item.documentation.map(|d| Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d,
        })),
        data: item.data.map(serde_json::Value::String),
        ..Default::default()
    }
}

fn def_range(target_source: &str, loc: &DefinitionLocation, utf8: bool) -> Range {
    Range {
        start: Position {
            line: loc.target_start_line,
            character: byte_col_to_lsp_char(source_line(target_source, loc.target_start_line), loc.target_start_col, utf8),
        },
        end: Position {
            line: loc.target_end_line,
            character: byte_col_to_lsp_char(source_line(target_source, loc.target_end_line), loc.target_end_col, utf8),
        },
    }
}

fn definition_to_location(target_source: &str, loc: &DefinitionLocation, utf8: bool) -> Location {
    Location { uri: path_to_uri(&loc.target_path), range: def_range(target_source, loc, utf8) }
}

fn definition_to_link(target_source: &str, loc: &DefinitionLocation, utf8: bool) -> LocationLink {
    let range = def_range(target_source, loc, utf8);
    LocationLink {
        origin_selection_range: None,
        target_uri: path_to_uri(&loc.target_path),
        target_range: range,
        target_selection_range: range,
    }
}

fn ref_to_location(r: &ReferenceLocation, sources: &std::collections::HashMap<String, String>, utf8: bool) -> Location {
    let empty = String::new();
    let src = sources.get(&r.path).unwrap_or(&empty);
    Location {
        uri: path_to_uri(&r.path),
        range: Range {
            start: Position {
                line: r.start_line,
                character: byte_col_to_lsp_char(source_line(src, r.start_line), r.start_col, utf8),
            },
            end: Position {
                line: r.end_line,
                character: byte_col_to_lsp_char(source_line(src, r.end_line), r.end_col, utf8),
            },
        },
    }
}

fn span_to_lsp_range(source: &str, span: &crate::workspace::symbols::Span, utf8: bool) -> Range {
    Range {
        start: Position {
            line: span.start_line,
            character: byte_col_to_lsp_char(source_line(source, span.start_line), span.start_col, utf8),
        },
        end: Position {
            line: span.end_line,
            character: byte_col_to_lsp_char(source_line(source, span.end_line), span.end_col, utf8),
        },
    }
}

fn to_lsp_document_symbol(source: &str, utf8: bool, s: crate::features::symbols::DocumentSymbol) -> DocumentSymbol {
    let kind = match s.kind {
        InternalSymbolKind::Class => SymbolKind::CLASS,
        InternalSymbolKind::Function => SymbolKind::FUNCTION,
        InternalSymbolKind::Variable => SymbolKind::VARIABLE,
        InternalSymbolKind::Namespace => SymbolKind::NAMESPACE,
        InternalSymbolKind::Module => SymbolKind::MODULE,
    };
    #[allow(deprecated)]
    DocumentSymbol {
        name: s.name,
        detail: s.detail,
        kind,
        tags: None,
        deprecated: None,
        range: span_to_lsp_range(source, &s.range, utf8),
        selection_range: span_to_lsp_range(source, &s.selection_range, utf8),
        children: if s.children.is_empty() {
            None
        } else {
            Some(s.children.into_iter().map(|c| to_lsp_document_symbol(source, utf8, c)).collect())
        },
    }
}

fn to_lsp_edit(e: crate::edit::TextEdit) -> TextEdit {
    // Note: col values here come from the code-actions feature which works in
    // byte offsets. Full UTF-16 conversion for edit ranges is tracked separately;
    // for now these values are passed through unchanged (correct for UTF-8 mode
    // and ASCII-safe edits in UTF-16 mode).
    TextEdit {
        range: Range {
            start: Position { line: e.start_line, character: e.start_col },
            end: Position { line: e.end_line, character: e.end_col },
        },
        new_text: e.new_text,
    }
}

fn ws_to_lsp_symbol(sym: &InternalWorkspaceSymbol, sources: &std::collections::HashMap<String, String>, utf8: bool) -> SymbolInformation {
    let kind = match sym.kind {
        InternalSymbolKind::Class => SymbolKind::CLASS,
        InternalSymbolKind::Function => SymbolKind::FUNCTION,
        InternalSymbolKind::Variable => SymbolKind::VARIABLE,
        InternalSymbolKind::Namespace => SymbolKind::NAMESPACE,
        InternalSymbolKind::Module => SymbolKind::MODULE,
    };
    let empty = String::new();
    let src = sources.get(&sym.container_name).unwrap_or(&empty);
    #[allow(deprecated)]
    SymbolInformation {
        name: sym.name.clone(),
        kind,
        tags: None,
        deprecated: None,
        location: Location {
            uri: path_to_uri(&sym.container_name),
            range: span_to_lsp_range(src, &sym.location, utf8),
        },
        container_name: Some(sym.container_name.clone()),
    }
}

fn internal_item_to_lsp(item: &InternalCallHierarchyItem, utf8: bool) -> CallHierarchyItem {
    let kind = match item.kind {
        ItemKind::Function => SymbolKind::FUNCTION,
        ItemKind::Module => SymbolKind::MODULE,
    };
    let data = serde_json::json!({
        "name": item.name,
        "kind": match item.kind { ItemKind::Function => "function", ItemKind::Module => "module" },
        "detail": item.detail,
        "uri": item.uri,
        "range": { "sl": item.range.start_line, "sc": item.range.start_col, "el": item.range.end_line, "ec": item.range.end_col },
        "sr": { "sl": item.selection_range.start_line, "sc": item.selection_range.start_col, "el": item.selection_range.end_line, "ec": item.selection_range.end_col },
    });
    let _ = utf8;
    CallHierarchyItem {
        name: item.name.clone(),
        kind,
        tags: None,
        detail: Some(item.detail.clone()),
        uri: path_to_uri(&item.uri),
        range: Range {
            start: Position { line: item.range.start_line, character: item.range.start_col },
            end: Position { line: item.range.end_line, character: item.range.end_col },
        },
        selection_range: Range {
            start: Position { line: item.selection_range.start_line, character: item.selection_range.start_col },
            end: Position { line: item.selection_range.end_line, character: item.selection_range.end_col },
        },
        data: Some(data),
    }
}

fn lsp_item_to_internal(item: &CallHierarchyItem) -> Option<InternalCallHierarchyItem> {
    let obj = item.data.as_ref()?.as_object()?;
    let kind = match obj.get("kind")?.as_str()? {
        "function" => ItemKind::Function,
        "module" => ItemKind::Module,
        _ => return None,
    };
    let range_obj = obj.get("range")?.as_object()?;
    let sr_obj = obj.get("sr")?.as_object()?;
    Some(InternalCallHierarchyItem {
        name: obj.get("name")?.as_str()?.to_owned(),
        kind,
        detail: obj.get("detail")?.as_str()?.to_owned(),
        uri: obj.get("uri")?.as_str()?.to_owned(),
        range: HierarchyRange {
            start_line: range_obj.get("sl")?.as_u64()? as u32,
            start_col: range_obj.get("sc")?.as_u64()? as u32,
            end_line: range_obj.get("el")?.as_u64()? as u32,
            end_col: range_obj.get("ec")?.as_u64()? as u32,
        },
        selection_range: HierarchyRange {
            start_line: sr_obj.get("sl")?.as_u64()? as u32,
            start_col: sr_obj.get("sc")?.as_u64()? as u32,
            end_line: sr_obj.get("el")?.as_u64()? as u32,
            end_col: sr_obj.get("ec")?.as_u64()? as u32,
        },
    })
}

fn hr_to_range(r: &HierarchyRange) -> Range {
    Range {
        start: Position { line: r.start_line, character: r.start_col },
        end: Position { line: r.end_line, character: r.end_col },
    }
}

fn lens_data_to_json(data: &LensData) -> serde_json::Value {
    let sym = match data.symbol_kind { LensSymbolKind::Macro => "macro", LensSymbolKind::Block => "block" };
    let kind = match data.lens_kind {
        LensKind::ReferenceCount => "ref_count",
        LensKind::InheritanceOverrides => "overrides",
        LensKind::InheritanceExtended => "extended",
    };
    serde_json::json!({
        "file_path": data.file_path,
        "symbol_kind": sym,
        "symbol_name": data.symbol_name,
        "decl_line": data.decl_line,
        "decl_col": data.decl_col,
        "lens_kind": kind,
    })
}

fn lens_data_from_json(val: &serde_json::Value) -> Option<LensData> {
    let obj = val.as_object()?;
    let symbol_kind = match obj.get("symbol_kind")?.as_str()? {
        "macro" => LensSymbolKind::Macro,
        "block" => LensSymbolKind::Block,
        _ => return None,
    };
    let lens_kind = match obj.get("lens_kind")?.as_str()? {
        "ref_count" => LensKind::ReferenceCount,
        "overrides" => LensKind::InheritanceOverrides,
        "extended" => LensKind::InheritanceExtended,
        _ => return None,
    };
    Some(LensData {
        file_path: obj.get("file_path")?.as_str()?.to_owned(),
        symbol_kind,
        symbol_name: obj.get("symbol_name")?.as_str()?.to_owned(),
        decl_line: obj.get("decl_line")?.as_u64()? as u32,
        decl_col: obj.get("decl_col")?.as_u64()? as u32,
        lens_kind,
    })
}

fn inlay_hint_data_to_json(data: &InlayHintData) -> serde_json::Value {
    match data {
        InlayHintData::Parameter { template_path, symbol_name, param_index } => serde_json::json!({
            "type": "parameter",
            "template_path": template_path,
            "symbol_name": symbol_name,
            "param_index": param_index,
        }),
        InlayHintData::EndBlock { template_path, block_name } => serde_json::json!({
            "type": "endblock",
            "template_path": template_path,
            "block_name": block_name,
        }),
    }
}

fn inlay_hint_data_from_json(val: &serde_json::Value) -> Option<InlayHintData> {
    let obj = val.as_object()?;
    match obj.get("type")?.as_str()? {
        "parameter" => Some(InlayHintData::Parameter {
            template_path: obj.get("template_path")?.as_str()?.to_owned(),
            symbol_name: obj.get("symbol_name")?.as_str()?.to_owned(),
            param_index: obj.get("param_index")?.as_u64()? as u32,
        }),
        "endblock" => Some(InlayHintData::EndBlock {
            template_path: obj.get("template_path")?.as_str()?.to_owned(),
            block_name: obj.get("block_name")?.as_str()?.to_owned(),
        }),
        _ => None,
    }
}

/// REQ-ARCH-02: run the LSP server over stdio with tracing to stderr only.
pub async fn run_lsp_server() {
    init_tracing();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
