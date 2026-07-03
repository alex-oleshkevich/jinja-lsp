// REQ-INLAY-01..07: inlay hints — macro/filter parameter labels and endblock echoes.
//
// Two shipped hint categories:
//   - parameterNames: label positional args at macro/filter call sites
//   - endblockNames:  echo the block name after a name-less {% endblock %}
//
// Tooltips are deferred to inlayHint/resolve (REQ-INLAY-05): the initial response
// carries only label+kind+position and an opaque logical-key `data` payload.

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::ReferenceKind,
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InlayHintKind {
    Parameter = 2,
}

/// Opaque logical-key payload for lazy resolve (REQ-INLAY-05).
///
/// Never a raw byte offset — the index is replaced atomically on every edit
/// (E07 REQ-DATA-08), so byte offsets would be stale immediately.
#[derive(Debug, Clone, PartialEq)]
pub enum InlayHintData {
    Parameter {
        template_path: String,
        /// Macro or filter name.
        symbol_name: String,
        /// Declared-parameter index into the source's `parameters`/`params` list.
        param_index: u32,
    },
    EndBlock {
        template_path: String,
        block_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct InlayHint {
    /// 0-based line number.
    pub line: u32,
    /// 0-based UTF-8 column.
    pub col: u32,
    /// The visible label (e.g. `"name:"` or `"content"`).
    pub label: String,
    /// `Some(Parameter)` for param-name hints; `None` for endblock echoes (REQ-INLAY-02).
    pub kind: Option<InlayHintKind>,
    /// Populated only after `inlay_hint_resolve` (REQ-INLAY-05).
    pub tooltip: Option<String>,
    /// Logical key for resolve — stable across edits.
    pub data: InlayHintData,
}

#[derive(Debug, Clone)]
pub struct InlayHintsConfig {
    /// Emit `param:` labels at macro and filter call sites (REQ-INLAY-04).
    pub parameter_names: bool,
    /// Echo the block name after a bare `{% endblock %}` (REQ-INLAY-04).
    pub endblock_names: bool,
}

impl Default for InlayHintsConfig {
    fn default() -> Self {
        Self { parameter_names: true, endblock_names: true }
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Collect all inlay hints for `source`.
///
/// The returned hints carry `tooltip: None`; populate it with [`inlay_hint_resolve`].
pub fn inlay_hints(
    source: &str,
    template_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    config: &InlayHintsConfig,
) -> Vec<InlayHint> {
    let mut out = Vec::new();

    if config.parameter_names {
        collect_param_hints(source, template_path, index, registry, workspace, &mut out);
    }
    if config.endblock_names {
        collect_endblock_echoes(source, template_path, &mut out);
    }

    out.sort_by_key(|h| (h.line, h.col));
    out
}

/// Attach a tooltip to `hint` by re-looking-up its logical key (REQ-INLAY-05).
///
/// On a miss (the symbol moved, was renamed, or deleted) the hint is returned unchanged.
pub fn inlay_hint_resolve(
    mut hint: InlayHint,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> InlayHint {
    match &hint.data {
        InlayHintData::Parameter { template_path: _, symbol_name, param_index } => {
            let tooltip =
                resolve_param_tooltip(symbol_name, *param_index as usize, index, registry, workspace);
            if let Some(tt) = tooltip {
                hint.tooltip = Some(tt);
            }
            // On miss: return unchanged — never throw (REQ-INLAY-05)
        }
        InlayHintData::EndBlock { template_path: _, block_name } => {
            let bn = block_name.clone();
            if index.blocks.iter().any(|b| b.name == bn) {
                hint.tooltip = Some(format!("block `{}`", bn));
            }
        }
    }
    hint
}

// ── Parameter-name hints ──────────────────────────────────────────────────────

fn collect_param_hints(
    source: &str,
    template_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    out: &mut Vec<InlayHint>,
) {
    for r in &index.references {
        match r.kind {
            ReferenceKind::Function => {
                // Macro call — check macro first.
                if let Some(params) = resolve_macro_params(&r.name, index, workspace) {
                    emit_call_hints(source, template_path, &r.name, r.span.end_byte, &params, out);
                } else if let Some(entry) = registry.get(Category::Filter, &r.name) {
                    // Filter-with-args (`x | f(args)`) is captured as Function, not Filter,
                    // because the grammar's function_call node matches before the bare-identifier
                    // filter pattern. Fall back to filter params.
                    if !entry.params.is_empty() {
                        let names: Vec<String> =
                            entry.params.iter().map(|p| p.name.clone()).collect();
                        emit_call_hints(source, template_path, &r.name, r.span.end_byte, &names, out);
                    }
                }
            }
            ReferenceKind::Filter => {
                // Bare filter (`x | f`) — currently the grammar always produces a bare
                // identifier here (no args). find_args_start returns None if no `(` follows.
                if let Some(entry) = registry.get(Category::Filter, &r.name) {
                    if !entry.params.is_empty() {
                        let names: Vec<String> =
                            entry.params.iter().map(|p| p.name.clone()).collect();
                        emit_call_hints(source, template_path, &r.name, r.span.end_byte, &names, out);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Return the declared parameter names for a resolvable macro, or `None` if the
/// macro can't be found (in which case we emit no hints — P4, never guess).
fn resolve_macro_params(
    name: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<Vec<String>> {
    // 1. Locally defined macro.
    if let Some(mac) = index.macros.iter().find(|m| m.name == name) {
        return Some(mac.parameters.iter().map(|p| p.name.clone()).collect());
    }

    // 2. From-imported macro ({% from "..." import name %}).
    for fi in &index.from_imports {
        for imp in &fi.names {
            // The call-site name is the alias (if any) or the original name.
            let call_name = imp.alias.as_deref().unwrap_or(&imp.name);
            if call_name == name {
                if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
                    if let Some(mac) = src_idx.macros.iter().find(|m| m.name == imp.name) {
                        return Some(mac.parameters.iter().map(|p| p.name.clone()).collect());
                    }
                }
            }
        }
    }

    None
}

/// Emit parameter-name inlay hints for the call site whose callee name ends at
/// `name_end_byte` in `source`.
fn emit_call_hints(
    source: &str,
    template_path: &str,
    symbol_name: &str,
    name_end_byte: usize,
    params: &[String],
    out: &mut Vec<InlayHint>,
) {
    let Some(args_start) = find_args_start(source, name_end_byte) else { return };
    let args = parse_args(source, args_start);

    for (param_idx, param_name) in params.iter().enumerate() {
        let Some(arg) = args.get(param_idx) else { break };
        if arg.is_keyword { break } // Stop at the first keyword argument (REQ-INLAY-01).
        if arg_matches_param(&arg.text, param_name) { continue } // Suppression (REQ-INLAY-06).

        let (line, col) = super::byte_to_line_col(source, arg.start_byte);
        out.push(InlayHint {
            line,
            col,
            label: format!("{}:", param_name),
            kind: Some(InlayHintKind::Parameter),
            tooltip: None,
            data: InlayHintData::Parameter {
                template_path: template_path.to_string(),
                symbol_name: symbol_name.to_string(),
                param_index: param_idx as u32,
            },
        });
    }
}

/// Returns the byte offset of the first character INSIDE the `(…)` argument list,
/// or `None` if there is no `(` immediately after any optional whitespace.
fn find_args_start(source: &str, name_end_byte: usize) -> Option<usize> {
    let after = source.get(name_end_byte..)?;
    let first_non_ws = after.find(|c: char| !c.is_whitespace())?;
    if after.as_bytes().get(first_non_ws) == Some(&b'(') {
        Some(name_end_byte + first_non_ws + 1)
    } else {
        None
    }
}

/// An argument parsed from a call's argument list.
struct Arg {
    /// Byte offset of the first non-whitespace character.
    start_byte: usize,
    /// Trimmed text of the argument.
    text: String,
    /// True when the argument contains a top-level `=` that is not `==`/`!=`/`<=`/`>=`.
    is_keyword: bool,
}

/// Parse the argument list starting INSIDE the opening `(` at `args_start`.
fn parse_args(source: &str, args_start: usize) -> Vec<Arg> {
    let bytes = source.as_bytes();
    let mut args = Vec::new();
    let mut arg_raw_start = args_start;
    // depth counts nested `(` and `[` so we only split on commas/close-paren at depth 0.
    let mut depth = 0usize;
    let mut in_str = false;
    let mut str_char = b'"';
    // escaped tracks whether the previous byte was an unescaped backslash.
    // We toggle it so \\ resets escaped, making the next char unescaped.
    let mut escaped = false;
    let mut i = args_start;

    while i < bytes.len() {
        let b = bytes[i];

        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == str_char {
                in_str = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_str = true;
            str_char = b;
        } else if b == b'(' || b == b'[' {
            depth += 1;
        } else if b == b')' {
            if depth == 0 {
                // Matched the call's opening `(` — flush the last argument and stop.
                push_arg(source, arg_raw_start, i, &mut args);
                break;
            }
            depth -= 1;
        } else if b == b']' {
            depth = depth.saturating_sub(1);
        } else if b == b',' && depth == 0 {
            push_arg(source, arg_raw_start, i, &mut args);
            arg_raw_start = i + 1;
        }

        i += 1;
    }

    args
}

fn push_arg(source: &str, raw_start: usize, raw_end: usize, args: &mut Vec<Arg>) {
    let slice = &source[raw_start..raw_end];
    let text = slice.trim().to_string();
    if text.is_empty() {
        return;
    }
    let leading_ws = slice.len() - slice.trim_start().len();
    let start_byte = raw_start + leading_ws;
    let is_keyword = has_top_level_assign(source, raw_start, raw_end);
    args.push(Arg { start_byte, text, is_keyword });
}

/// Returns `true` if `source[start..end]` contains a top-level `=` that is not
/// part of `==`, `!=`, `<=`, or `>=`.
fn has_top_level_assign(source: &str, start: usize, end: usize) -> bool {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut str_char = b'"';

    let mut i = start;
    while i < end {
        let b = bytes[i];
        if in_str {
            if b == str_char && (i == start || bytes[i - 1] != b'\\') {
                in_str = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_str = true;
            str_char = b;
        } else if b == b'(' || b == b'[' {
            depth += 1;
        } else if b == b')' || b == b']' {
            depth = depth.saturating_sub(1);
        } else if b == b'=' && depth == 0 {
            let prev = if i > start { bytes[i - 1] } else { 0 };
            let next = if i + 1 < end { bytes[i + 1] } else { 0 };
            // Skip ==, !=, <=, >=
            if prev != b'!' && prev != b'<' && prev != b'>' && prev != b'=' && next != b'=' {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Suppression check (REQ-INLAY-06).
///
/// Returns `true` (suppress the hint) when:
/// - `arg_text` is a bare identifier equal to `param_name`, or
/// - it is a dotted accessor whose final segment equals `param_name`, or
/// - it is a subscript with a string-literal key equal to `param_name`.
fn arg_matches_param(arg_text: &str, param_name: &str) -> bool {
    // 1. Bare identifier.
    if arg_text == param_name {
        return true;
    }

    // 2. Dotted attribute access: `x.y.z` — final segment after the last `.`.
    // String literals never cause false positives here: `"hello.name"` has final
    // segment `name"` (with the closing quote), which won't equal `name`.
    if let Some(dot_pos) = arg_text.rfind('.') {
        let final_seg = &arg_text[dot_pos + 1..];
        if final_seg == param_name {
            return true;
        }
    }

    // 3. Subscript with a string-literal key: `x["name"]` or `x['name']`.
    let dq = format!("[\"{}\"]", param_name);
    let sq = format!("['{}']", param_name);
    if arg_text.ends_with(&dq) || arg_text.ends_with(&sq) {
        return true;
    }

    false
}

// ── Endblock echo hints ───────────────────────────────────────────────────────

/// Scan `source` for bare `{% endblock %}` tags (without an explicit block name)
/// and emit kind-less hints that echo the enclosing block's name (REQ-INLAY-02).
fn collect_endblock_echoes(source: &str, template_path: &str, out: &mut Vec<InlayHint>) {
    let bytes = source.as_bytes();
    let mut block_stack: Vec<String> = Vec::new();
    let mut i = 0;
    // jinja-lsp-7h6z: while inside {% raw %}…{% endraw %}, literal tag text (e.g. a
    // stray {% endblock %} written as example markup) must not affect the real
    // block stack. Mirrors folding.rs's in_raw handling for the same scan.
    let mut in_raw = false;

    while i + 1 < bytes.len() {
        if bytes[i] != b'{' || bytes[i + 1] != b'%' {
            i += 1;
            continue;
        }

        // Found `{%`; scan forward to `%}`.
        let tag_start = i;
        let Some(close_pos) = find_tag_close(bytes, i + 2) else {
            i += 2;
            continue;
        };

        let inner = &source[tag_start + 2..close_pos];
        let trimmed = inner.trim().trim_start_matches('-').trim().trim_end_matches('-').trim();
        let mut words = trimmed.split_whitespace();
        let keyword = words.next();

        if in_raw {
            if keyword == Some("endraw") {
                in_raw = false;
            }
            i = close_pos + 2;
            continue;
        }

        match keyword {
            Some("raw") => {
                in_raw = true;
            }
            Some("block") => {
                // Opening block: push the block name (first word after `block`).
                // Skip `scoped` and `required` modifiers — the name is always first.
                if let Some(name) = words.next() {
                    block_stack.push(name.to_string());
                }
            }
            Some("endblock") => {
                let name_after = words.next();
                if let Some(block_name) = block_stack.pop() {
                    if name_after.is_none() {
                        // Bare endblock — emit an echo hint (REQ-INLAY-02).
                        let hint_col = endblock_keyword_end_col(source, tag_start);
                        let (line, col) = hint_col;
                        out.push(InlayHint {
                            line,
                            col,
                            label: block_name.clone(),
                            kind: None,
                            tooltip: None,
                            data: InlayHintData::EndBlock {
                                template_path: template_path.to_string(),
                                block_name,
                            },
                        });
                    }
                }
            }
            _ => {}
        }

        i = close_pos + 2; // Skip past `%}`.
    }
}

/// Find the closing `%}` of a tag, starting the search at `start`.
///
/// Returns the byte offset of `%` in `%}`.
/// String literals are skipped so `%}` inside `"..."` or `'...'` is not treated as a closer.
fn find_tag_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut in_str = false;
    let mut str_char = b'"';
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == str_char {
                in_str = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_str = true;
            str_char = b;
        } else if b == b'%' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Return `(line, col)` of the character immediately after the `endblock` keyword
/// within the tag that starts at `tag_start_byte` (the `{` of `{%`).
fn endblock_keyword_end_col(source: &str, tag_start_byte: usize) -> (u32, u32) {
    let slice = source.get(tag_start_byte..).unwrap_or("");
    let kw_offset = slice.find("endblock").unwrap_or(0);
    let end_byte = tag_start_byte + kw_offset + "endblock".len();
    super::byte_to_line_col(source, end_byte)
}

// ── Resolve tooltip helpers ───────────────────────────────────────────────────

fn resolve_param_tooltip(
    symbol_name: &str,
    param_idx: usize,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<String> {
    // Local macro.
    if let Some(mac) = index.macros.iter().find(|m| m.name == symbol_name) {
        let p = mac.parameters.get(param_idx)?;
        return Some(fmt_macro_param(&p.name, p.default.as_deref()));
    }

    // From-imported macro.
    for fi in &index.from_imports {
        for imp in &fi.names {
            let call_name = imp.alias.as_deref().unwrap_or(&imp.name);
            if call_name == symbol_name {
                if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
                    if let Some(mac) = src_idx.macros.iter().find(|m| m.name == imp.name) {
                        let p = mac.parameters.get(param_idx)?;
                        return Some(fmt_macro_param(&p.name, p.default.as_deref()));
                    }
                }
            }
        }
    }

    // Registry filter.
    if let Some(entry) = registry.get(Category::Filter, symbol_name) {
        let p = entry.params.get(param_idx)?;
        return Some(fmt_filter_param(&p.name, p.ty.as_deref(), p.default.as_deref()));
    }

    None
}

fn fmt_macro_param(name: &str, default: Option<&str>) -> String {
    match default {
        Some(d) => format!("{} = {}", name, d),
        None => name.to_string(),
    }
}

fn fmt_filter_param(name: &str, ty: Option<&str>, default: Option<&str>) -> String {
    let type_part = ty.map(|t| format!(": {}", t)).unwrap_or_default();
    let default_part = default.map(|d| format!(" = {}", d)).unwrap_or_default();
    format!("{}{}{}", name, type_part, default_part)
}

// ── Shared source-scanning utilities ─────────────────────────────────────────
