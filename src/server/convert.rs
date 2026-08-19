//! Conversions between the crate's internal types and `lsp_types`, plus the
//! position-encoding helpers everything else here is built on.
//!
//! Nothing in this module reads state or performs I/O: each function maps one
//! value to another, which is what keeps the handlers in `mod.rs` short enough
//! to read as protocol wiring.

use super::*;

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
    let mut safe = (byte_col as usize).min(line_str.len());
    while safe > 0 && !line_str.is_char_boundary(safe) {
        safe -= 1;
    }
    line_str[..safe].chars().map(|c| c.len_utf16() as u32).sum()
}

/// Convert a workspace key back to a URI for the client.
///
/// Prefers `Url::from_file_path`, which percent-encodes spaces/`#`/`?`/non-ASCII
/// correctly. Falls back to the hand-rolled form for keys that aren't real
/// absolute filesystem paths (e.g. inline-template keys like `view.py::47`).
pub fn path_to_uri(path: &str) -> Url {
    Url::from_file_path(path).unwrap_or_else(|_| {
        if path.starts_with('/') {
            Url::parse(&format!("file://{path}")).unwrap_or_else(|_| {
                Url::parse("file:///unknown").expect("constant fallback URL must parse")
            })
        } else {
            Url::parse(&format!("file:///{path}")).unwrap_or_else(|_| {
                Url::parse("file:///unknown").expect("constant fallback URL must parse")
            })
        }
    })
}

pub(crate) fn internal_workspace_edit_to_lsp(
    we: crate::edit::WorkspaceEdit,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> WorkspaceEdit {
    let empty = String::new();
    if we.create_files.is_empty() {
        let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
            std::collections::HashMap::new();
        for (path, edits) in we.changes {
            let src = sources.get(&path).unwrap_or(&empty);
            let lsp = edits
                .into_iter()
                .map(|e| to_lsp_edit(e, src, utf8))
                .collect();
            changes.insert(path_to_uri(&path), lsp);
        }
        WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }
    } else {
        let mut ops: Vec<DocumentChangeOperation> = Vec::new();
        for (path, content) in we.create_files {
            let uri = path_to_uri(&path);
            ops.push(DocumentChangeOperation::Op(ResourceOp::Create(
                CreateFile {
                    uri: uri.clone(),
                    options: None,
                    annotation_id: None,
                },
            )));
            if !content.is_empty() {
                ops.push(DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
                    edits: vec![OneOf::Left(TextEdit {
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        new_text: content,
                    })],
                }));
            }
        }
        for (path, edits) in we.changes {
            let uri = path_to_uri(&path);
            let src = sources.get(&path).unwrap_or(&empty);
            for e in edits {
                ops.push(DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: None,
                    },
                    edits: vec![OneOf::Left(to_lsp_edit(e, src, utf8))],
                }));
            }
        }
        WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Operations(ops)),
            change_annotations: None,
        }
    }
}

pub(crate) fn to_lsp_action(
    action: InternalCodeAction,
    _file_uri: &str,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> CodeAction {
    let kind = Some(match action.kind {
        ActionKind::QuickFix => CodeActionKind::QUICKFIX,
        ActionKind::RefactorExtract => CodeActionKind::REFACTOR_EXTRACT,
        ActionKind::RefactorRewrite => CodeActionKind::REFACTOR_REWRITE,
    });

    let edit = action
        .edit
        .map(|we| internal_workspace_edit_to_lsp(we, sources, utf8));

    let diagnostics = if action.diagnostics.is_empty() {
        None
    } else {
        let lsp_diags: Vec<LspDiagnostic> = action
            .diagnostics
            .iter()
            .map(|d| {
                let source = sources.get(&d.file).map(|s| s.as_str()).unwrap_or("");
                to_lsp_diagnostic(source, utf8, d)
            })
            .collect();
        Some(lsp_diags)
    };

    let command = action.command.map(|(cmd_id, args)| Command {
        title: cmd_id.clone(),
        command: cmd_id,
        arguments: Some(vec![args]),
    });

    CodeAction {
        title: action.title,
        kind,
        diagnostics,
        edit,
        command,
        is_preferred: Some(action.is_preferred),
        disabled: None,
        data: None,
    }
}

/// Convert an LSP diagnostic (UTF-16 `character` column) back into an internal
/// diagnostic (byte column), so code-action handlers that build TextEdits from
/// `diag.col` land at the right byte offset on lines with non-ASCII text.
pub(crate) fn from_lsp_diagnostic(
    d: &LspDiagnostic,
    key: &str,
    source: &str,
    utf8: bool,
) -> Option<crate::diagnostic::Diagnostic> {
    let code = match &d.code {
        Some(NumberOrString::String(s)) => s.clone(),
        _ => return None,
    };
    let byte_col = lsp_char_to_byte_col(
        source_line(source, d.range.start.line),
        d.range.start.character,
        utf8,
    );
    Some(crate::diagnostic::Diagnostic {
        code,
        slug: String::new(),
        message: d.message.clone(),
        file: key.to_owned(),
        line: d.range.start.line,
        col: byte_col,
        severity: crate::diagnostic::DiagnosticSeverity::Warning,
    })
}

pub(crate) fn to_lsp_diagnostic(
    source: &str,
    utf8: bool,
    d: &crate::diagnostic::Diagnostic,
) -> LspDiagnostic {
    let severity = Some(match d.severity {
        InternalSeverity::Error => DiagnosticSeverity::ERROR,
        InternalSeverity::Warning => DiagnosticSeverity::WARNING,
        InternalSeverity::Info => DiagnosticSeverity::INFORMATION,
        InternalSeverity::Hint => DiagnosticSeverity::HINT,
    });
    let col = byte_col_to_lsp_char(source_line(source, d.line), d.col, utf8);
    LspDiagnostic {
        range: Range {
            start: Position {
                line: d.line,
                character: col,
            },
            end: Position {
                line: d.line,
                character: col + 1,
            },
        },
        severity,
        code: Some(NumberOrString::String(d.code.clone())),
        source: Some("jinja-lsp".to_owned()),
        message: d.message.clone(),
        ..Default::default()
    }
}

pub(crate) fn to_lsp_completion_item(
    item: crate::features::completions::CompletionItem,
) -> CompletionItem {
    let kind = Some(match item.kind {
        CompletionKind::Filter => CompletionItemKind::FUNCTION,
        CompletionKind::Function => CompletionItemKind::FUNCTION,
        CompletionKind::Test => CompletionItemKind::FUNCTION,
        CompletionKind::Variable => CompletionItemKind::VARIABLE,
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::File | CompletionKind::TemplatePath => CompletionItemKind::FILE,
        CompletionKind::Folder => CompletionItemKind::FOLDER,
        CompletionKind::Attribute => CompletionItemKind::FIELD,
        CompletionKind::KeywordArg => CompletionItemKind::PROPERTY,
    });
    CompletionItem {
        label: item.label,
        kind,
        detail: item.detail,
        documentation: item.documentation.map(|d| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: d,
            })
        }),
        data: item.data.map(serde_json::Value::String),
        ..Default::default()
    }
}

pub(crate) fn def_range(target_source: &str, loc: &DefinitionLocation, utf8: bool) -> Range {
    Range {
        start: Position {
            line: loc.target_start_line,
            character: byte_col_to_lsp_char(
                source_line(target_source, loc.target_start_line),
                loc.target_start_col,
                utf8,
            ),
        },
        end: Position {
            line: loc.target_end_line,
            character: byte_col_to_lsp_char(
                source_line(target_source, loc.target_end_line),
                loc.target_end_col,
                utf8,
            ),
        },
    }
}

pub(crate) fn definition_to_location(
    target_source: &str,
    loc: &DefinitionLocation,
    utf8: bool,
) -> Location {
    Location {
        uri: path_to_uri(&loc.target_path),
        range: def_range(target_source, loc, utf8),
    }
}

pub(crate) fn definition_to_link(
    target_source: &str,
    loc: &DefinitionLocation,
    utf8: bool,
    origin: Option<Range>,
) -> LocationLink {
    let range = def_range(target_source, loc, utf8);
    LocationLink {
        origin_selection_range: origin,
        target_uri: path_to_uri(&loc.target_path),
        target_range: range,
        target_selection_range: range,
    }
}

pub(crate) fn lsp_range_at_cursor(source: &str, line: u32, character: u32, utf8: bool) -> Range {
    let line_text = source_line(source, line);
    let col = lsp_char_to_byte_col(line_text, character, utf8);
    let end_col = byte_col_to_lsp_char(line_text, col + 1, utf8);
    Range {
        start: Position { line, character },
        end: Position {
            line,
            character: end_col,
        },
    }
}

pub(crate) fn ref_to_location(
    r: &ReferenceLocation,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> Location {
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

/// jinja-lsp-qpc6: a lens target is a single declaration point, not a span, so
/// its LSP Location is zero-width -- matching how code_lens()'s own `range` is
/// built for the lens's anchor position.
pub(crate) fn lens_target_to_location(
    t: &crate::features::code_lens::LensTarget,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> Location {
    let empty = String::new();
    let src = sources.get(&t.path).unwrap_or(&empty);
    let character = byte_col_to_lsp_char(source_line(src, t.line), t.col, utf8);
    Location {
        uri: path_to_uri(&t.path),
        range: Range {
            start: Position {
                line: t.line,
                character,
            },
            end: Position {
                line: t.line,
                character,
            },
        },
    }
}

pub(crate) fn span_to_lsp_range(
    source: &str,
    span: &crate::workspace::symbols::Span,
    utf8: bool,
) -> Range {
    Range {
        start: Position {
            line: span.start_line,
            character: byte_col_to_lsp_char(
                source_line(source, span.start_line),
                span.start_col,
                utf8,
            ),
        },
        end: Position {
            line: span.end_line,
            character: byte_col_to_lsp_char(source_line(source, span.end_line), span.end_col, utf8),
        },
    }
}

pub(crate) fn to_lsp_document_symbol(
    source: &str,
    utf8: bool,
    s: crate::features::symbols::DocumentSymbol,
) -> DocumentSymbol {
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
            Some(
                s.children
                    .into_iter()
                    .map(|c| to_lsp_document_symbol(source, utf8, c))
                    .collect(),
            )
        },
    }
}

pub(crate) fn to_lsp_edit(e: crate::edit::TextEdit, source: &str, utf8: bool) -> TextEdit {
    let start_char = byte_col_to_lsp_char(source_line(source, e.start_line), e.start_col, utf8);
    let end_char = byte_col_to_lsp_char(source_line(source, e.end_line), e.end_col, utf8);
    TextEdit {
        range: Range {
            start: Position {
                line: e.start_line,
                character: start_char,
            },
            end: Position {
                line: e.end_line,
                character: end_char,
            },
        },
        new_text: e.new_text,
    }
}

pub(crate) fn ws_to_lsp_symbol(
    sym: &InternalWorkspaceSymbol,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> SymbolInformation {
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

pub(crate) fn internal_item_to_lsp(
    item: &InternalCallHierarchyItem,
    sources: &std::collections::HashMap<String, String>,
    utf8: bool,
) -> CallHierarchyItem {
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
    let empty = String::new();
    let src = sources.get(&item.uri).unwrap_or(&empty);
    CallHierarchyItem {
        name: item.name.clone(),
        kind,
        tags: None,
        detail: Some(item.detail.clone()),
        uri: path_to_uri(&item.uri),
        range: Range {
            start: Position {
                line: item.range.start_line,
                character: byte_col_to_lsp_char(
                    source_line(src, item.range.start_line),
                    item.range.start_col,
                    utf8,
                ),
            },
            end: Position {
                line: item.range.end_line,
                character: byte_col_to_lsp_char(
                    source_line(src, item.range.end_line),
                    item.range.end_col,
                    utf8,
                ),
            },
        },
        selection_range: Range {
            start: Position {
                line: item.selection_range.start_line,
                character: byte_col_to_lsp_char(
                    source_line(src, item.selection_range.start_line),
                    item.selection_range.start_col,
                    utf8,
                ),
            },
            end: Position {
                line: item.selection_range.end_line,
                character: byte_col_to_lsp_char(
                    source_line(src, item.selection_range.end_line),
                    item.selection_range.end_col,
                    utf8,
                ),
            },
        },
        data: Some(data),
    }
}

pub(crate) fn lsp_item_to_internal(item: &CallHierarchyItem) -> Option<InternalCallHierarchyItem> {
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

pub(crate) fn hr_to_range(r: &HierarchyRange, source: &str, utf8: bool) -> Range {
    Range {
        start: Position {
            line: r.start_line,
            character: byte_col_to_lsp_char(source_line(source, r.start_line), r.start_col, utf8),
        },
        end: Position {
            line: r.end_line,
            character: byte_col_to_lsp_char(source_line(source, r.end_line), r.end_col, utf8),
        },
    }
}

pub(crate) fn lens_data_to_json(data: &LensData) -> serde_json::Value {
    let sym = match data.symbol_kind {
        LensSymbolKind::Macro => "macro",
        LensSymbolKind::Block => "block",
    };
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

pub(crate) fn lens_data_from_json(val: &serde_json::Value) -> Option<LensData> {
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

pub(crate) fn inlay_hint_data_to_json(data: &InlayHintData) -> serde_json::Value {
    match data {
        InlayHintData::Parameter {
            template_path,
            symbol_name,
            param_index,
        } => serde_json::json!({
            "type": "parameter",
            "template_path": template_path,
            "symbol_name": symbol_name,
            "param_index": param_index,
        }),
        InlayHintData::EndBlock {
            template_path,
            block_name,
        } => serde_json::json!({
            "type": "endblock",
            "template_path": template_path,
            "block_name": block_name,
        }),
    }
}

pub(crate) fn inlay_hint_data_from_json(val: &serde_json::Value) -> Option<InlayHintData> {
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

/// Convert internal byte-based SemanticTokens to the LSP wire format (delta-encoded).
///
/// The LSP protocol requires delta-encoded positions in the negotiated encoding (UTF-16 by
/// default, UTF-8 when negotiated). `tokens` must already be sorted by (line, start_char).
pub(crate) fn tokens_to_lsp_data(
    tokens: &[InternalSemanticToken],
    source: &str,
    utf8: bool,
) -> Vec<SemanticToken> {
    // jinja-lsp-5qqy: split the document into lines once instead of calling
    // source_line (which re-scans from byte 0) per token — O(lines + tokens)
    // instead of O(lines * tokens).
    let lines: Vec<&str> = source.split('\n').collect();
    let mut data = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_wire_char = 0u32;
    for tok in tokens {
        let line_str = lines.get(tok.line as usize).copied().unwrap_or("");
        let (wire_char, wire_length) = if utf8 {
            (tok.start_char, tok.length)
        } else {
            let wc = byte_col_to_lsp_char(line_str, tok.start_char, false);
            let byte_start = tok.start_char as usize;
            let byte_end = (tok.start_char + tok.length) as usize;
            let name_text = line_str
                .get(byte_start..byte_end.min(line_str.len()))
                .unwrap_or("");
            let wl: u32 = name_text.chars().map(|c| c.len_utf16() as u32).sum();
            (wc, wl)
        };
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            wire_char - prev_wire_char
        } else {
            wire_char
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: wire_length,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.token_modifiers,
        });
        prev_line = tok.line;
        prev_wire_char = wire_char;
    }
    data
}

/// REQ-ARCH-02: run the LSP server over stdio with tracing to stderr only.
pub async fn run_lsp_server() {
    init_tracing();
    tracing::info!(
        "{} v{} (built {}) starting",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_TIMESTAMP"),
    );
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
