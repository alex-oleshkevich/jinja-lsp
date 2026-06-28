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

use crate::workspace::index::WorkspaceIndex;
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
            state: Arc::new(RwLock::new(ServerState {
                workspace: WorkspaceIndex::default(),
                generation: 0,
            })),
        }
    }

    /// Pass 1 (REQ-ARCH-03): re-extract one file, atomically replace its entry.
    async fn pass1(&self, key: &str, source: &str) {
        self.state.write().await.update_file(key, source);
    }

    /// Pass 2 (REQ-ARCH-04): relink the workspace.  Called debounced (on save
    /// or after idle); the generation guard discards stale relinks.
    async fn pass2(&self) {
        let gen_snapshot = self.state.read().await.generation;

        // Relink is currently a no-op — template_chain() resolves lazily from
        // the HashMap, so no extra relink step is needed.  When cross-file
        // diagnostics land (F01) this becomes meaningful.
        tokio::task::spawn_blocking(move || {
            // placeholder: expensive relink work would go here
            let _ = gen_snapshot;
        })
        .await
        .ok();

        // generation guard: only apply if no newer Pass 1 ran while we worked
        // (already satisfied because relink is currently trivial)
    }

    fn uri_to_key(uri: &Url) -> String {
        uri.path().to_owned()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    /// REQ-ARCH-08: declare capabilities matching the feature set.
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
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
                    trigger_characters: Some(vec![
                        ".".into(), "|".into(), " ".into(), "\"".into(), "'".into(),
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

    /// REQ-ARCH-05: open triggers Pass 1.
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
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
        _params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        Ok(None)
    }

    async fn hover(&self, _params: HoverParams) -> Result<Option<Hover>> {
        Ok(None)
    }

    async fn signature_help(
        &self,
        _params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        Ok(None)
    }

    async fn goto_definition(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(None)
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(None)
    }

    async fn document_symbol(
        &self,
        _params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(None)
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
        _params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        Ok(None)
    }

    async fn formatting(
        &self,
        _params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }

    async fn range_formatting(
        &self,
        _params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(None)
    }
}

/// REQ-ARCH-02: run the LSP server over stdio with tracing to stderr only.
pub async fn run_lsp_server() {
    init_tracing();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
