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

use crate::features::code_actions::{code_actions, ActionKind, CodeAction as InternalCodeAction};
use crate::features::completions::{complete, resolve_doc, CompletionKind};
use crate::features::definition::{go_to_definition, DefinitionLocation};
use crate::features::hover::hover as hover_feature;
use crate::features::formatting::{format_document, format_range};
use crate::features::symbols::{document_symbols, SymbolKind as InternalSymbolKind};
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
        // REQ-EDIT-10: apply InitializationOptions overlay on top of discovered config.
        {
            let mut state = self.state.write().await;
            state.definition_link_support = link_support;
            if let Some(opts) = params.initialization_options {
                if let Ok(overlay) = serde_json::from_value::<crate::config::ConfigOverlay>(opts) {
                    state.apply_init_options(overlay);
                }
            }
        }
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "jinja-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
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
                    commands: vec![],
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
            self.state.write().await.apply_init_options(overlay);
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
    }

    /// REQ-ARCH-05: change triggers Pass 1 (full-sync, newest content wins).
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            let key = Self::uri_to_key(&params.text_document.uri);
            self.pass1(&key, &change.text).await;
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
        let items = complete(source, pos.line, pos.character, index, &state.registry, &state.workspace);
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
        let Some(result) = hover_feature(source, pos.line, pos.character, index, &state.registry, &state.workspace)
        else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: result.markdown,
            }),
            range: Some(Range {
                start: Position { line: result.start_line, character: result.start_col },
                end: Position { line: result.end_line, character: result.end_col },
            }),
        }))
    }

    async fn signature_help(
        &self,
        _params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        Ok(None)
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
        let Some(loc) = go_to_definition(source, pos.line, pos.character, &key, index, &state.registry, &state.workspace)
        else {
            return Ok(None);
        };
        if state.definition_link_support {
            Ok(Some(GotoDefinitionResponse::Link(vec![definition_to_link(&loc)])))
        } else {
            Ok(Some(GotoDefinitionResponse::Scalar(definition_to_location(&loc))))
        }
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let Some(index) = state.workspace.templates.get(&key) else { return Ok(None) };
        let syms = document_symbols(source, index);
        if syms.is_empty() {
            return Ok(None);
        }
        Ok(Some(DocumentSymbolResponse::Nested(
            syms.into_iter().map(to_lsp_document_symbol).collect(),
        )))
    }

    async fn document_highlight(
        &self,
        _params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        Ok(None)
    }

    async fn folding_range(
        &self,
        _params: FoldingRangeParams,
    ) -> Result<Option<Vec<FoldingRange>>> {
        Ok(None)
    }

    async fn inlay_hint(
        &self,
        _params: InlayHintParams,
    ) -> Result<Option<Vec<InlayHint>>> {
        Ok(None)
    }

    async fn code_lens(&self, _params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        Ok(None)
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

        let actions = code_actions(source, &key, &diags, index, &state.workspace, &state.registry);

        if actions.is_empty() {
            return Ok(None);
        }

        let lsp_actions: Vec<CodeActionOrCommand> = actions
            .into_iter()
            .map(|a| CodeActionOrCommand::CodeAction(to_lsp_action(a, &key)))
            .collect();

        Ok(Some(lsp_actions))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let key = Self::uri_to_key(&params.text_document.uri);
        let state = self.state.read().await;
        let Some(source) = state.sources.get(&key) else { return Ok(None) };
        let edits = format_document(source);
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
        let edits = format_range(source, range.start.line, range.end.line);
        if edits.is_empty() { return Ok(None); }
        Ok(Some(edits.into_iter().map(to_lsp_edit).collect()))
    }
}

fn path_to_uri(path: &str) -> Url {
    if path.starts_with('/') {
        Url::parse(&format!("file://{path}")).unwrap_or_else(|_| Url::parse("file:///unknown").unwrap())
    } else {
        Url::parse(&format!("file:///{path}")).unwrap_or_else(|_| Url::parse("file:///unknown").unwrap())
    }
}

fn to_lsp_action(action: InternalCodeAction, _file_uri: &str) -> CodeAction {
    let kind = Some(match action.kind {
        ActionKind::QuickFix => CodeActionKind::QUICKFIX,
        ActionKind::RefactorExtract => CodeActionKind::REFACTOR_EXTRACT,
        ActionKind::RefactorRewrite => CodeActionKind::REFACTOR_REWRITE,
    });

    let edit = action.edit.map(|we| {
        if we.create_files.is_empty() {
            // Simple case: only text changes — use the compact `changes` map.
            let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> = std::collections::HashMap::new();
            for (path, edits) in we.changes {
                changes.insert(path_to_uri(&path), edits.into_iter().map(to_lsp_edit).collect());
            }
            WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None }
        } else {
            // Complex case: file creations — must use document_changes.
            let mut ops: Vec<DocumentChangeOperation> = we.changes
                .into_iter()
                .flat_map(|(path, edits)| {
                    let uri = path_to_uri(&path);
                    edits.into_iter().map(move |e| {
                        DocumentChangeOperation::Edit(TextDocumentEdit {
                            text_document: OptionalVersionedTextDocumentIdentifier { uri: uri.clone(), version: None },
                            edits: vec![OneOf::Left(to_lsp_edit(e))],
                        })
                    })
                })
                .collect();
            for (path, _content) in we.create_files {
                ops.push(DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
                    uri: path_to_uri(&path),
                    options: None,
                    annotation_id: None,
                })));
            }
            WorkspaceEdit { changes: None, document_changes: Some(DocumentChanges::Operations(ops)), change_annotations: None }
        }
    });

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

fn to_lsp_completion_item(item: crate::features::completions::CompletionItem) -> CompletionItem {
    let kind = Some(match item.kind {
        CompletionKind::Filter => CompletionItemKind::FUNCTION,
        CompletionKind::Function => CompletionItemKind::FUNCTION,
        CompletionKind::Test => CompletionItemKind::FUNCTION,
        CompletionKind::Variable => CompletionItemKind::VARIABLE,
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::TemplatePath => CompletionItemKind::FILE,
        CompletionKind::Attribute => CompletionItemKind::FIELD,
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

fn def_range(loc: &DefinitionLocation) -> Range {
    Range {
        start: Position { line: loc.target_start_line, character: loc.target_start_col },
        end: Position { line: loc.target_end_line, character: loc.target_end_col },
    }
}

fn definition_to_location(loc: &DefinitionLocation) -> Location {
    Location { uri: path_to_uri(&loc.target_path), range: def_range(loc) }
}

fn definition_to_link(loc: &DefinitionLocation) -> LocationLink {
    let range = def_range(loc);
    LocationLink {
        origin_selection_range: None,
        target_uri: path_to_uri(&loc.target_path),
        target_range: range,
        target_selection_range: range,
    }
}

fn span_to_range(span: &crate::workspace::symbols::Span) -> Range {
    Range {
        start: Position { line: span.start_line, character: span.start_col },
        end: Position { line: span.end_line, character: span.end_col },
    }
}

fn to_lsp_document_symbol(s: crate::features::symbols::DocumentSymbol) -> DocumentSymbol {
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
        range: span_to_range(&s.range),
        selection_range: span_to_range(&s.selection_range),
        children: if s.children.is_empty() {
            None
        } else {
            Some(s.children.into_iter().map(to_lsp_document_symbol).collect())
        },
    }
}

fn to_lsp_edit(e: crate::features::code_actions::TextEdit) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position { line: e.start_line, character: e.start_col },
            end: Position { line: e.end_line, character: e.end_col },
        },
        new_text: e.new_text,
    }
}

/// REQ-ARCH-02: run the LSP server over stdio with tracing to stderr only.
pub async fn run_lsp_server() {
    init_tracing();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
