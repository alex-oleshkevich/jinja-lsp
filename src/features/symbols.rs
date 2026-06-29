// REQ-SYM-01..05: document outline + workspace symbol search.

use std::collections::HashMap;

use crate::workspace::{
    index::{TemplateIndex, WorkspaceIndex},
    symbols::{Span, TemplateRefKind, VariableScope},
};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Class,     // block
    Function,  // macro
    Variable,  // top-level set
    Namespace, // import / from-import
    Module,    // extends / include
}

#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    pub range: Span,
    pub selection_range: Span,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub container_name: String,
    pub location: Span,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Build the document outline for `textDocument/documentSymbol` (REQ-SYM-01..02..05).
pub fn document_symbols(source: &str, index: &TemplateIndex) -> Vec<DocumentSymbol> {
    let mut flat = collect_flat(source, index);
    // Sort: start_byte ASC, span_len DESC (enclosing first for containment fold).
    flat.sort_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then(b.span_len.cmp(&a.span_len))
    });
    build_tree(flat)
}

/// Fuzzy-search every macro and block in the workspace (REQ-SYM-03..04).
pub fn workspace_symbols(query: &str, workspace: &WorkspaceIndex) -> Vec<WorkspaceSymbol> {
    let mut results: Vec<(WorkspaceSymbol, u8, usize)> = Vec::new();
    let mut stable_idx = 0usize;

    for (path, tmpl_idx) in &workspace.templates {
        for m in &tmpl_idx.macros {
            if let Some(tier) = fuzzy_tier(query, &m.name) {
                results.push((
                    WorkspaceSymbol {
                        name: m.name.clone(),
                        kind: SymbolKind::Function,
                        container_name: path.clone(),
                        location: m.span.clone(),
                    },
                    tier,
                    stable_idx,
                ));
            }
            stable_idx += 1;
        }
        for b in &tmpl_idx.blocks {
            if let Some(tier) = fuzzy_tier(query, &b.name) {
                results.push((
                    WorkspaceSymbol {
                        name: b.name.clone(),
                        kind: SymbolKind::Class,
                        container_name: path.clone(),
                        location: b.span.clone(),
                    },
                    tier,
                    stable_idx,
                ));
            }
            stable_idx += 1;
        }
    }

    // Sort: tier ASC, name.len() ASC, stable_idx ASC (REQ-SYM-04).
    results.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.0.name.len().cmp(&b.0.name.len()))
            .then(a.2.cmp(&b.2))
    });

    results.into_iter().map(|(sym, _, _)| sym).collect()
}

// ── Flat node collection ──────────────────────────────────────────────────────

struct FlatNode {
    sym: DocumentSymbol,
    start_byte: usize,
    span_len: usize,
}

fn collect_flat(source: &str, index: &TemplateIndex) -> Vec<FlatNode> {
    let mut flat: Vec<FlatNode> = Vec::new();

    // Compute full tag extents so we can nest constructs by span containment.
    // The index spans only cover the opening keyword content (e.g. "block foo"),
    // not the full `{% block %}...{% endblock %}` construct.
    let tags = scan_jinja_tags(source);
    let full_extents = compute_full_extents(&tags);

    // Blocks → Class (REQ-SYM-01).
    for b in &index.blocks {
        if let Some(full) = full_tag_span(&tags, &full_extents, TagKind::Block, &b.name) {
            let (start, end) = full;
            flat.push(FlatNode {
                start_byte: start,
                span_len: end - start,
                sym: DocumentSymbol {
                    name: b.name.clone(),
                    kind: SymbolKind::Class,
                    detail: None,
                    selection_range: name_span_in(source, start, &b.name),
                    range: make_span(source, start, end),
                    children: vec![],
                },
            });
        } else if b.span.start_byte < b.span.end_byte {
            // Fallback: use the stored span (opening tag only) for compatibility.
            flat.push(FlatNode {
                start_byte: b.span.start_byte,
                span_len: b.span.end_byte - b.span.start_byte,
                sym: DocumentSymbol {
                    name: b.name.clone(),
                    kind: SymbolKind::Class,
                    detail: None,
                    selection_range: name_span_in(source, b.span.start_byte, &b.name),
                    range: b.span.clone(),
                    children: vec![],
                },
            });
        }
    }

    // Macros → Function (REQ-SYM-01).
    for m in &index.macros {
        if let Some(full) = full_tag_span(&tags, &full_extents, TagKind::Macro, &m.name) {
            let (start, end) = full;
            flat.push(FlatNode {
                start_byte: start,
                span_len: end - start,
                sym: DocumentSymbol {
                    name: m.name.clone(),
                    kind: SymbolKind::Function,
                    detail: Some(macro_params_detail(&m.parameters)),
                    selection_range: name_span_in(source, start, &m.name),
                    range: make_span(source, start, end),
                    children: vec![],
                },
            });
        } else if m.span.start_byte < m.span.end_byte {
            flat.push(FlatNode {
                start_byte: m.span.start_byte,
                span_len: m.span.end_byte - m.span.start_byte,
                sym: DocumentSymbol {
                    name: m.name.clone(),
                    kind: SymbolKind::Function,
                    detail: Some(macro_params_detail(&m.parameters)),
                    selection_range: name_span_in(source, m.span.start_byte, &m.name),
                    range: m.span.clone(),
                    children: vec![],
                },
            });
        }
    }

    // Top-level `{% set %}` variables → Variable (REQ-SYM-01).
    // Variable spans are all-zero in the index, so we use text search to locate
    // the set statement and check it isn't physically inside a block or macro.
    for v in &index.variables {
        if v.scope == VariableScope::Template {
            if let Some(span) = find_set_span(source, &v.name, &tags, &full_extents) {
                flat.push(FlatNode {
                    start_byte: span.start_byte,
                    span_len: span.end_byte - span.start_byte,
                    sym: DocumentSymbol {
                        name: v.name.clone(),
                        kind: SymbolKind::Variable,
                        detail: None,
                        selection_range: name_span_in(source, span.start_byte, &v.name),
                        range: span,
                        children: vec![],
                    },
                });
            }
        }
    }

    // Import aliases → Namespace (REQ-SYM-01, REQ-SYM-05).
    for alias in &index.import_aliases {
        if alias.span.start_byte < alias.span.end_byte {
            flat.push(FlatNode {
                start_byte: alias.span.start_byte,
                span_len: alias.span.end_byte - alias.span.start_byte,
                sym: DocumentSymbol {
                    name: alias.alias.clone(),
                    kind: SymbolKind::Namespace,
                    detail: Some(alias.source.clone()),
                    selection_range: alias.span.clone(),
                    range: alias.span.clone(),
                    children: vec![],
                },
            });
        }
    }

    // From-imports → Namespace, named by source path (REQ-SYM-01, REQ-SYM-05).
    for fi in &index.from_imports {
        if fi.span.start_byte < fi.span.end_byte {
            flat.push(FlatNode {
                start_byte: fi.span.start_byte,
                span_len: fi.span.end_byte - fi.span.start_byte,
                sym: DocumentSymbol {
                    name: fi.source.clone(),
                    kind: SymbolKind::Namespace,
                    detail: Some(fi.source.clone()),
                    selection_range: fi.span.clone(),
                    range: fi.span.clone(),
                    children: vec![],
                },
            });
        }
    }

    // extends / include → Module (REQ-SYM-01, REQ-SYM-05).
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.span.start_byte >= tr.span.end_byte {
            continue;
        }
        let kind = match tr.kind {
            TemplateRefKind::Extends | TemplateRefKind::Include => SymbolKind::Module,
            // Import/From are covered by import_aliases/from_imports above.
            TemplateRefKind::Import | TemplateRefKind::From => continue,
        };
        flat.push(FlatNode {
            start_byte: tr.span.start_byte,
            span_len: tr.span.end_byte - tr.span.start_byte,
            sym: DocumentSymbol {
                name: tr.path.clone(),
                kind,
                detail: Some(tr.path.clone()),
                selection_range: tr.span.clone(),
                range: tr.span.clone(),
                children: vec![],
            },
        });
    }

    flat
}

// ── Tree building — span-containment fold (REQ-SYM-02) ───────────────────────

fn build_tree(flat: Vec<FlatNode>) -> Vec<DocumentSymbol> {
    let n = flat.len();
    if n == 0 {
        return vec![];
    }

    // Find parent for each node: innermost strictly-containing ancestor.
    let mut parents: Vec<Option<usize>> = vec![None; n];
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..n {
        // Pop stack nodes whose span ended before node[i] begins.
        while let Some(&top) = stack.last() {
            if flat[top].start_byte + flat[top].span_len > flat[i].start_byte {
                break;
            }
            stack.pop();
        }
        parents[i] = stack.last().copied();
        stack.push(i);
    }

    // Build adjacency lists.
    let mut child_lists: Vec<Vec<usize>> = vec![vec![]; n];
    let mut top_level: Vec<usize> = Vec::new();
    for (i, &parent) in parents.iter().enumerate() {
        match parent {
            Some(p) => child_lists[p].push(i),
            None => top_level.push(i),
        }
    }

    // Move symbols into the tree. `Option::take` ensures each index is visited once.
    let mut nodes: Vec<Option<FlatNode>> = flat.into_iter().map(Some).collect();
    fn build(i: usize, nodes: &mut Vec<Option<FlatNode>>, cl: &[Vec<usize>]) -> DocumentSymbol {
        let mut node = nodes[i].take().expect("flat-node visited twice — parent-child index is inconsistent");
        for &ci in &cl[i] {
            node.sym.children.push(build(ci, nodes, cl));
        }
        node.sym
    }
    top_level
        .into_iter()
        .map(|i| build(i, &mut nodes, &child_lists))
        .collect()
}

// ── Fuzzy matching (REQ-SYM-04) ──────────────────────────────────────────────

/// Returns the match tier (0=exact, 1=prefix, 2=contiguous, 3=subsequence),
/// or `None` if the query is not a subsequence of the name.
fn fuzzy_tier(query: &str, name: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(3); // empty query matches everything
    }
    let q = query.to_lowercase();
    let n = name.to_lowercase();
    if n == q {
        return Some(0);
    }
    if n.starts_with(&q) {
        return Some(1);
    }
    if n.contains(&q) {
        return Some(2);
    }
    if is_subsequence(&q, &n) {
        return Some(3);
    }
    None
}

fn is_subsequence(query: &str, name: &str) -> bool {
    let mut qc = query.chars().peekable();
    for nc in name.chars() {
        if qc.peek() == Some(&nc) {
            qc.next();
        }
    }
    qc.peek().is_none()
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn macro_params_detail(params: &[crate::workspace::symbols::Parameter]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|p| match &p.default {
            Some(d) => format!("{}={}", p.name, d),
            None => p.name.clone(),
        })
        .collect();
    format!("({})", parts.join(", "))
}

/// Find the selection range for a symbol's name starting from `tag_start_byte`.
/// Searches for the identifier in the source and returns its byte span.
fn name_span_in(source: &str, tag_start_byte: usize, name: &str) -> Span {
    let slice = match source.get(tag_start_byte..) {
        Some(s) => s,
        None => return Span::default(),
    };
    // Find `name` as a whole word: the byte before the match must not be an identifier char.
    let mut search_from = 0;
    let pos = loop {
        match slice[search_from..].find(name) {
            None => return Span::default(),
            Some(rel) => {
                let abs_rel = search_from + rel;
                let preceded_by_ident = abs_rel > 0 && {
                    let p = slice.as_bytes()[abs_rel - 1];
                    p.is_ascii_alphanumeric() || p == b'_'
                };
                if !preceded_by_ident {
                    break abs_rel;
                }
                search_from = abs_rel + 1;
            }
        }
    };
    let abs_start = tag_start_byte + pos;
    let abs_end = abs_start + name.len();
    let (sl, sc) = byte_to_line_col(source, abs_start);
    let (el, ec) = byte_to_line_col(source, abs_end);
    Span { start_byte: abs_start, end_byte: abs_end, start_line: sl, start_col: sc, end_line: el, end_col: ec }
}

/// Convert a byte offset to (line, col) in the source.
fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut pos = 0usize;
    for ch in source.chars() {
        if pos >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf8() as u32;
        }
        pos += ch.len_utf8();
    }
    (line, col)
}

/// Find the byte span of the `{% set name … %}` statement for a top-level
/// variable, or return `None` if the set statement is inside a block or macro
/// (REQ-SYM-01).
///
/// Variable spans in the index are all-zero, so we locate the statement by
/// text search. Containment is checked against the FULL tag extents (from
/// `{%` to the matching `%}` end tag), not the narrow stored spans.
fn find_set_span(
    source: &str,
    name: &str,
    tags: &[TagInfo],
    full_extents: &HashMap<usize, usize>,
) -> Option<Span> {
    let candidates: Vec<usize> = [
        format!("{{% set {} ", name),
        format!("{{% set {}=", name),
        format!("{{% set {} =", name),
        format!("{{%- set {} ", name),
        format!("{{%- set {}=", name),
    ]
    .iter()
    .filter_map(|p| source.find(p.as_str()))
    .collect();

    let start = *candidates.iter().min()?;

    // Reject if the set byte falls within any block/macro's full construct extent.
    let inside = tags.iter().filter(|t| !t.is_close).any(|t| {
        full_extents
            .get(&t.start_byte)
            .map(|&full_end| t.start_byte < start && start < full_end)
            .unwrap_or(false)
    });
    if inside {
        return None;
    }

    let rest = source.get(start..)?;
    let end_offset = rest.find("%}")?;
    let end = start + end_offset + 2;

    let (sl, sc) = byte_to_line_col(source, start);
    let (el, ec) = byte_to_line_col(source, end);
    Some(Span { start_byte: start, end_byte: end, start_line: sl, start_col: sc, end_line: el, end_col: ec })
}

// ── Tag scanner — computes full construct extents ─────────────────────────────
//
// The tree-sitter Jinja grammar stores `block_statement` and `macro_statement`
// as FLAT nodes covering only the opening keyword+name (e.g., "block foo"),
// NOT the full `{% block %}…{% endblock %}` construct.  To know which bytes
// are truly "inside" a block or macro, we do a lightweight scan of the raw
// source text, balancing opening and closing tags with a stack.

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind { Block, Macro }

struct TagInfo {
    start_byte: usize, // byte offset of the `{%`
    end_byte: usize,   // byte offset just past the `%}`
    kind: TagKind,
    name: String,      // block/macro name; empty for end tags
    is_close: bool,
}

/// Scan all `{% block %}`, `{% endblock %}`, `{% macro %}`, `{% endmacro %}`
/// tags in `source` and return them in source order.
fn scan_jinja_tags(source: &str) -> Vec<TagInfo> {
    let mut tags = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'%' {
            if let Some(rel) = source[i + 2..].find("%}") {
                let inner = &source[i + 2..i + 2 + rel];
                let tag_end = i + 2 + rel + 2;
                if let Some(tag) = classify_tag_content(inner, i, tag_end) {
                    tags.push(tag);
                }
                i = tag_end;
                continue;
            }
        }
        i += source[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    tags
}

fn classify_tag_content(inner: &str, start: usize, end: usize) -> Option<TagInfo> {
    let s = inner.trim_matches('-').trim();
    if let Some(rest) = s.strip_prefix("block") {
        let rest = rest.trim();
        if rest.is_empty() || rest == "required" {
            return None; // bare `{% block %}` — not a named opening
        }
        if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            let name = rest.split_whitespace().next().unwrap_or("").to_owned();
            return Some(TagInfo { start_byte: start, end_byte: end, kind: TagKind::Block, name, is_close: false });
        }
    }
    if s.starts_with("endblock") {
        return Some(TagInfo { start_byte: start, end_byte: end, kind: TagKind::Block, name: String::new(), is_close: true });
    }
    if let Some(rest) = s.strip_prefix("macro") {
        let rest = rest.trim();
        if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            let name = rest.split(['(', ' ']).next().unwrap_or("").to_owned();
            return Some(TagInfo { start_byte: start, end_byte: end, kind: TagKind::Macro, name, is_close: false });
        }
    }
    if s.starts_with("endmacro") {
        return Some(TagInfo { start_byte: start, end_byte: end, kind: TagKind::Macro, name: String::new(), is_close: true });
    }
    None
}

/// For each open tag, compute the byte offset where its matching end tag ends.
/// Returns a map: `open_start_byte → close_end_byte`.
fn compute_full_extents(tags: &[TagInfo]) -> HashMap<usize, usize> {
    let mut stack: Vec<usize> = Vec::new(); // indices into `tags`
    let mut result = HashMap::new();
    for (i, tag) in tags.iter().enumerate() {
        if !tag.is_close {
            stack.push(i);
        } else {
            // Find the innermost open tag of the same kind.
            if let Some(pos) = stack.iter().rposition(|&idx| tags[idx].kind == tag.kind) {
                let open_idx = stack.remove(pos);
                result.insert(tags[open_idx].start_byte, tag.end_byte);
            }
        }
    }
    result
}

/// Look up the full byte range `(open_start, close_end)` for the first tag
/// matching `(kind, name)` that has a full extent recorded.
fn full_tag_span(
    tags: &[TagInfo],
    full_extents: &HashMap<usize, usize>,
    kind: TagKind,
    name: &str,
) -> Option<(usize, usize)> {
    tags.iter()
        .filter(|t| !t.is_close && t.kind == kind && t.name == name)
        .find_map(|t| full_extents.get(&t.start_byte).map(|&end| (t.start_byte, end)))
}

/// Build a `Span` from raw byte offsets, computing line/col from `source`.
fn make_span(source: &str, start_byte: usize, end_byte: usize) -> Span {
    let (sl, sc) = byte_to_line_col(source, start_byte);
    let (el, ec) = byte_to_line_col(source, end_byte);
    Span { start_byte, end_byte, start_line: sl, start_col: sc, end_line: el, end_col: ec }
}
