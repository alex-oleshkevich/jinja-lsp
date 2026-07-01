use std::collections::HashMap;
use std::sync::LazyLock;

use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::workspace::{
    index::TemplateIndex,
    symbols::{
        BlockDefinition, FromImport, ImportAlias, ImportedName, MacroCallSite, MacroDefinition,
        Parameter, Reference, ReferenceKind, Span, SyntaxError, TemplateRefKind,
        TemplateReference, VariableDefinition, VariableScope,
    },
};

// ── helpers ─────────────────────────────────────────────────────────────────

fn node_span(n: Node) -> Span {
    let s = n.start_position();
    let e = n.end_position();
    Span {
        start_byte: n.start_byte(),
        end_byte: n.end_byte(),
        start_line: s.row as u32,
        start_col: s.column as u32,
        end_line: e.row as u32,
        end_col: e.column as u32,
    }
}

fn txt<'a>(n: Node, bytes: &'a [u8]) -> &'a str {
    n.utf8_text(bytes).unwrap_or("")
}

fn ancestor<'a>(mut n: Node<'a>, kind: &str) -> Option<Node<'a>> {
    while let Some(p) = n.parent() {
        if p.kind() == kind {
            return Some(p);
        }
        n = p;
    }
    None
}

// ── pre-compiled queries ─────────────────────────────────────────────────────
// Each query is compiled once on first access (LazyLock) instead of on every
// extract() call. QueryCursor still stays per-call (it holds match state).

macro_rules! jinja_query {
    ($name:ident, $file:literal) => {
        static $name: LazyLock<Query> = LazyLock::new(|| {
            Query::new(&tree_sitter_jinja::language(), include_str!($file))
                .expect(concat!("failed to compile query: ", $file))
        });
    };
}

jinja_query!(Q_MACROS,         "queries/macros.scm");
jinja_query!(Q_PARAMS,         "queries/params.scm");
jinja_query!(Q_BLOCKS,         "queries/blocks.scm");
jinja_query!(Q_SET_UNPACKING,  "queries/set_unpacking.scm");
jinja_query!(Q_SET,            "queries/set.scm");
jinja_query!(Q_FOR_UNPACKING,  "queries/for_unpacking.scm");
jinja_query!(Q_FOR,            "queries/for.scm");
jinja_query!(Q_WITH,           "queries/with.scm");
jinja_query!(Q_TRANS,          "queries/trans.scm");
jinja_query!(Q_CALLER_ARGS,    "queries/caller_args.scm");
jinja_query!(Q_EXTENDS,        "queries/extends.scm");
jinja_query!(Q_INCLUDES,       "queries/includes.scm");
jinja_query!(Q_IMPORTS,        "queries/imports.scm");
jinja_query!(Q_FROM_IMPORTS,   "queries/from_imports.scm");
jinja_query!(Q_IMPORT_NAMES,   "queries/import_names.scm");
jinja_query!(Q_REFERENCES,     "queries/references.scm");
jinja_query!(Q_CALL_SITES,     "queries/call_sites.scm");

// ── public API ───────────────────────────────────────────────────────────────

/// Parse `source` once, run all 17 queries, merge captures into a fresh
/// `TemplateIndex` (REQ-EXTR-03). Syntax errors (JINJA-E001) are recorded.
pub fn extract(source: &str) -> TemplateIndex {
    // REQ-CONV-04: span large-file parsing for observability; pass-1 fast path is unspanned.
    let _large_file_span = if source.len() > 50_000 {
        Some(tracing::info_span!("extract_large_file", bytes = source.len()).entered())
    } else {
        None
    };

    let mut parser = Parser::new();
    if parser.set_language(&tree_sitter_jinja::language()).is_err() {
        return TemplateIndex::empty();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return TemplateIndex::empty(),
    };

    let bytes = source.as_bytes();
    let mut idx = TemplateIndex::empty();

    collect_errors(tree.root_node(), &mut idx.syntax_errors);
    do_macros(&tree, bytes, &mut idx);
    do_blocks(&tree, bytes, &mut idx);
    do_variables(&tree, bytes, &mut idx);
    do_template_refs(&tree, bytes, &mut idx);
    do_imports(&tree, bytes, &mut idx);
    do_from_imports(&tree, bytes, &mut idx);
    do_references(&tree, bytes, &mut idx);
    do_call_sites(&tree, bytes, &mut idx);

    idx
}

// ── syntax errors ────────────────────────────────────────────────────────────

fn collect_errors(node: Node, out: &mut Vec<SyntaxError>) {
    if node.is_error() {
        out.push(SyntaxError { span: node_span(node) });
    }
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            collect_errors(c.node(), out);
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── macros + params ──────────────────────────────────────────────────────────

fn do_macros(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let mq = &*Q_MACROS;
    let pq = &*Q_PARAMS;

    // (key=macro_stmt.start_byte, name, header_span, ctrl_end_byte)
    let mut macro_vec: Vec<(usize, String, Span, usize)> = vec![];
    {
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&mq, tree.root_node(), bytes);
        while let Some(m) = ms.next() {
            for cap in m.captures {
                if mq.capture_names()[cap.index as usize] == "name" {
                    if let Some(stmt) = ancestor(cap.node, "macro_statement") {
                        // Walk up to the control node ({% ... %}) to get its end byte.
                        let ctrl_end = ancestor(stmt, "control")
                            .map(|c| c.end_byte())
                            .unwrap_or(stmt.end_byte());
                        macro_vec.push((
                            stmt.start_byte(),
                            txt(cap.node, bytes).to_owned(),
                            node_span(stmt),
                            ctrl_end,
                        ));
                    }
                }
            }
        }
    }

    let mut param_map: HashMap<usize, Vec<Parameter>> = HashMap::new();
    {
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&pq, tree.root_node(), bytes);
        while let Some(m) = ms.next() {
            let mut name = None;
            let mut default = None;
            let mut key = None;
            for cap in m.captures {
                match pq.capture_names()[cap.index as usize] {
                    "name" => {
                        name = Some(txt(cap.node, bytes).to_owned());
                        key = ancestor(cap.node, "macro_statement").map(|n| n.start_byte());
                    }
                    "default" => default = Some(txt(cap.node, bytes).to_owned()),
                    _ => {}
                }
            }
            if let (Some(n), Some(k)) = (name, key) {
                param_map.entry(k).or_default().push(Parameter { name: n, default });
            }
        }
    }

    for (key, name, span, ctrl_end) in macro_vec {
        let parameters = param_map.remove(&key).unwrap_or_default();
        let body = macro_body_span(tree.root_node(), ctrl_end, bytes);
        let doc = extract_first_comment(bytes, body.start_byte, body.end_byte);
        idx.macros.push(MacroDefinition { name, parameters, body, span, doc });
    }
}

/// Compute the body span for a macro by walking the AST siblings from the
/// opening `{% macro %}` control node (whose end byte is `ctrl_end`) forward,
/// counting nested macro opens/closes, until the matching `{% endmacro %}` is
/// found.  Returns a Span covering [ctrl_end .. endmacro_ctrl.start_byte()].
fn macro_body_span(root: Node, ctrl_end: usize, bytes: &[u8]) -> Span {
    let mut c = root.walk();
    if !c.goto_first_child() {
        return Span::default();
    }

    let mut found = false;
    let mut depth = 0usize;
    loop {
        let node = c.node();
        if !found && node.kind() == "control" && node.end_byte() == ctrl_end {
            // Found the opening control tag for this macro.
            found = true;
            depth = 1;
        } else if found && node.kind() == "control" {
            // control → statement → (macro_statement | anonymous "endmacro")
            if let Some(stmt) = node.named_child(0) {
                let inner_kind = stmt.named_child(0).map(|n| n.kind()).unwrap_or("");
                if inner_kind == "macro_statement" {
                    depth += 1;
                } else {
                    let s = std::str::from_utf8(&bytes[stmt.start_byte()..stmt.end_byte()])
                        .unwrap_or("");
                    if s.trim() == "endmacro" {
                        depth -= 1;
                        if depth == 0 {
                            return Span {
                                start_byte: ctrl_end,
                                end_byte: node.start_byte(),
                                ..Span::default()
                            };
                        }
                    }
                }
            }
        }
        if !c.goto_next_sibling() {
            break;
        }
    }
    // Fallback: no endmacro found; body covers to end of source.
    Span {
        start_byte: ctrl_end,
        end_byte: bytes.len(),
        ..Span::default()
    }
}

// ── blocks ───────────────────────────────────────────────────────────────────

fn do_blocks(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let bq = &*Q_BLOCKS;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&bq, tree.root_node(), bytes);

    // blocks.scm has two patterns; required blocks match both — deduplicate by start_byte.
    // Value: (block_idx_in_idx, ctrl_end_byte) where ctrl_end is end of {% block name %} tag.
    let mut seen: HashMap<usize, (usize, usize)> = HashMap::new();

    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut span = Span::default();
        let mut key = 0usize;
        let mut scoped = false;
        let mut required = false;
        let mut ctrl_end = 0usize;

        for cap in m.captures {
            match bq.capture_names()[cap.index as usize] {
                "name" => {
                    name = txt(cap.node, bytes).to_owned();
                    if let Some(bs) = cap.node.parent() {
                        key = bs.start_byte();
                        span = node_span(bs);
                        ctrl_end = ancestor(bs, "control")
                            .map(|c| c.end_byte())
                            .unwrap_or(bs.end_byte());
                        // The grammar has no scoped_keyword node. Detect "scoped"
                        // by scanning source bytes from the identifier end to the
                        // nearest closing block delimiter.
                        let name_end = cap.node.end_byte();
                        if let Some(after) = bytes.get(name_end..) {
                            let close = after.windows(2).position(|w| w == b"%}")
                                .or_else(|| after.windows(3).position(|w| w == b"-%}"))
                                .unwrap_or(after.len());
                            if let Ok(segment) = std::str::from_utf8(&after[..close]) {
                                if segment.split_whitespace().any(|w| w == "scoped") {
                                    scoped = true;
                                }
                            }
                        }
                    }
                }
                "required" => required = true,
                _ => {}
            }
        }

        if name.is_empty() {
            continue;
        }

        if let Some(&(i, _)) = seen.get(&key) {
            if scoped { idx.blocks[i].scoped = true; }
            if required { idx.blocks[i].required = true; }
        } else {
            seen.insert(key, (idx.blocks.len(), ctrl_end));
            idx.blocks.push(BlockDefinition { name, scoped, required, body: Span::default(), span, end_name_span: None });
        }
    }

    // Populate BlockDefinition.body using scope regions (which track endblock positions).
    // {% endblock %} (no trailing name) → normal control node, handled by scope regions.
    // {% endblock name %} (trailing name)  → ERROR node in tree-sitter; handled below.
    let scope_regions = build_scope_regions(tree.root_node(), bytes);
    for (_, (i, ctrl_end)) in &seen {
        if let Some(region) = scope_regions.iter().find(|r| {
            r.scope == VariableScope::Block && r.body_start == *ctrl_end
        }) {
            idx.blocks[*i].body = byte_span(region.body_start, region.body_end);
            // No trailing name in a proper endblock control node.
        }
    }

    // Second pass: walk ERROR nodes to handle {% endblock name %} (trailing name).
    // The tree-sitter grammar doesn't support the trailing identifier, so these tags
    // parse as ERROR nodes. For each matching ERROR, set body + end_name_span.
    let mut cur = tree.root_node().walk();
    if cur.goto_first_child() {
        loop {
            let node = cur.node();
            if node.is_error() {
                let tag_start = node.start_byte();
                if let Some(ns) = endblock_trailing_name_span(bytes, tag_start) {
                    let trail_name = std::str::from_utf8(&bytes[ns.start_byte..ns.end_byte])
                        .unwrap_or("");
                    for (_, (bi, ctrl_end)) in &seen {
                        if idx.blocks[*bi].name == trail_name
                            && idx.blocks[*bi].body == Span::default()
                        {
                            idx.blocks[*bi].body = byte_span(*ctrl_end, tag_start);
                            idx.blocks[*bi].end_name_span = Some(ns.clone());
                        }
                    }
                }
            }
            if !cur.goto_next_sibling() { break; }
        }
    }
}

/// Scan `{% endblock name %}` starting at `tag_start` and return the span of
/// the trailing identifier (`name`), if present.
fn endblock_trailing_name_span(bytes: &[u8], tag_start: usize) -> Option<Span> {
    let slice = bytes.get(tag_start..)?;
    // Find "endblock" within the tag
    let kw = b"endblock";
    let kw_pos = slice.windows(kw.len()).position(|w| w == kw)?;
    let after_kw = tag_start + kw_pos + kw.len();
    let rest = bytes.get(after_kw..)?;
    // Skip whitespace
    let ws = rest.iter().take_while(|&&b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r').count();
    let name_start = after_kw + ws;
    let name_bytes = bytes.get(name_start..)?;
    // Read identifier chars (alphanumeric or _)
    let name_len = name_bytes.iter().take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_').count();
    if name_len == 0 {
        return None;
    }
    let name_end = name_start + name_len;
    let (sl, sc) = bytes_to_line_col(bytes, name_start);
    let (el, ec) = bytes_to_line_col(bytes, name_end);
    Some(Span { start_byte: name_start, end_byte: name_end, start_line: sl, start_col: sc, end_line: el, end_col: ec })
}

fn bytes_to_line_col(bytes: &[u8], offset: usize) -> (u32, u32) {
    let before = &bytes[..offset.min(bytes.len())];
    let line = before.iter().filter(|&&b| b == b'\n').count() as u32;
    let col = before.iter().rposition(|&b| b == b'\n').map(|p| offset - p - 1).unwrap_or(offset) as u32;
    (line, col)
}

// ── variables ────────────────────────────────────────────────────────────────

fn do_variables(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let scope_regions = build_scope_regions(tree.root_node(), bytes);
    let source_len = bytes.len();
    let mut seen_set: HashMap<usize, ()> = HashMap::new();
    let mut seen_for: HashMap<usize, ()> = HashMap::new();

    run_set_unpacking(tree, bytes, idx, &mut seen_set, &scope_regions, source_len);
    run_set(tree, bytes, idx, &seen_set, &scope_regions, source_len);
    run_set_block(bytes, idx, &scope_regions, source_len);
    run_for_unpacking(tree, bytes, idx, &mut seen_for, &scope_regions, source_len);
    run_for(tree, bytes, idx, &seen_for, &scope_regions, source_len);
    run_with(tree, bytes, idx, &scope_regions);
    run_trans(tree, bytes, idx);
    run_caller_args(tree, bytes, idx);
}

fn run_set_unpacking(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, seen: &mut HashMap<usize, ()>,
    scope_regions: &[ScopeRegion], source_len: usize,
) {
    let q = &*Q_SET_UNPACKING;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut names: Vec<(String, Span)> = vec![];
        let mut key = None;
        let mut set_ctrl_end = 0usize;
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "name" | "name2" => {
                    if key.is_none() {
                        if let Some(set_stmt) = ancestor(cap.node, "set_statement") {
                            key = Some(set_stmt.start_byte());
                            set_ctrl_end = ancestor(set_stmt, "control")
                                .map(|c| c.end_byte())
                                .unwrap_or(set_stmt.end_byte());
                        }
                    }
                    names.push((txt(cap.node, bytes).to_owned(), node_span(cap.node)));
                }
                _ => {}
            }
        }
        if let Some(k) = key {
            seen.insert(k, ());
            let scope = scope_for_byte(scope_regions, k);
            let valid_end = enclosing_region(scope_regions, set_ctrl_end)
                .map(|r| r.body_end)
                .unwrap_or(source_len);
            let valid_range = byte_span(set_ctrl_end, valid_end);
            for (name, span) in names {
                push_var(idx, name, scope, span, valid_range.clone());
            }
        }
    }
}

fn run_set(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, skip: &HashMap<usize, ()>,
    scope_regions: &[ScopeRegion], source_len: usize,
) {
    let q = &*Q_SET;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut name_span = Span::default();
        let mut key = None;
        let mut set_ctrl_end = 0usize;
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                name = txt(cap.node, bytes).to_owned();
                name_span = node_span(cap.node);
                if let Some(set_stmt) = ancestor(cap.node, "set_statement") {
                    key = Some(set_stmt.start_byte());
                    set_ctrl_end = ancestor(set_stmt, "control")
                        .map(|c| c.end_byte())
                        .unwrap_or(set_stmt.end_byte());
                }
            }
        }
        if !name.is_empty() && !skip.contains_key(&key.unwrap_or(0)) {
            let k = key.unwrap_or(0);
            let scope = scope_for_byte(scope_regions, k);
            let valid_end = enclosing_region(scope_regions, set_ctrl_end)
                .map(|r| r.body_end)
                .unwrap_or(source_len);
            push_var(idx, name, scope, name_span, byte_span(set_ctrl_end, valid_end));
        }
    }
}

// REQ-EXTR-09: block-set variables — {% set name %}…{% endset %}.
//
// tree-sitter parses the block-set opening tag as an ERROR node that absorbs
// the rest of the source, so tree-sitter queries cannot find multiple block-set
// tags in the same template.  We use a manual byte scanner instead.
//
// The scanner looks for the literal byte sequence `{%…set…NAME…%}` where
// the only token between NAME and `%}` is optional whitespace — which is the
// exact discriminator between block-set (no `=`) and regular set (has `=`).
fn run_set_block(
    bytes: &[u8],
    idx: &mut TemplateIndex, scope_regions: &[ScopeRegion], source_len: usize,
) {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] != b'{' || bytes[i + 1] != b'%' {
            i += 1;
            continue;
        }
        let tag_start = i;
        let mut j = i + 2;
        // Skip optional whitespace-control modifier (`-` or `+`) directly after `{%`.
        if matches!(bytes.get(j), Some(&b'-') | Some(&b'+')) {
            j += 1;
        }
        j = skip_ascii_ws(bytes, j);
        // Require "set" keyword followed by whitespace.
        if bytes.get(j..j + 3) != Some(b"set") {
            i += 1;
            continue;
        }
        let k_after_set = j + 3;
        if !bytes.get(k_after_set).map(|b| b.is_ascii_whitespace()).unwrap_or(false) {
            i += 1;
            continue;
        }
        let mut k = skip_ascii_ws(bytes, k_after_set);
        // Capture identifier: starts with letter or '_'.
        if !bytes.get(k).map(|b| b.is_ascii_alphabetic() || *b == b'_').unwrap_or(false) {
            i += 1;
            continue;
        }
        let name_start = k;
        while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
            k += 1;
        }
        let name_end = k;
        k = skip_ascii_ws(bytes, k);
        // Skip optional whitespace-control modifier before `%}`.
        if matches!(bytes.get(k), Some(&b'-') | Some(&b'+')) {
            k += 1;
        }
        // Block-set: next token is `%}` (no `=` before the closing delimiter).
        if bytes.get(k..k + 2) != Some(b"%}") {
            i += 1;
            continue;
        }
        let ctrl_end = k + 2;
        let name = match std::str::from_utf8(&bytes[name_start..name_end]) {
            Ok(s) if !s.is_empty() => s,
            _ => { i += 1; continue; }
        };
        let (sl, sc) = bytes_to_line_col(bytes, name_start);
        let (el, ec) = bytes_to_line_col(bytes, name_end);
        let name_span = Span { start_byte: name_start, end_byte: name_end, start_line: sl, start_col: sc, end_line: el, end_col: ec };
        let scope = scope_for_byte(scope_regions, tag_start);
        let valid_end = enclosing_region(scope_regions, ctrl_end)
            .map(|r| r.body_end)
            .unwrap_or(source_len);
        push_var(idx, name.to_owned(), scope, name_span, byte_span(ctrl_end, valid_end));
        i += 1;
    }
}

fn skip_ascii_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
    i
}

fn run_for_unpacking(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, seen: &mut HashMap<usize, ()>,
    scope_regions: &[ScopeRegion], source_len: usize,
) {
    let q = &*Q_FOR_UNPACKING;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut names: Vec<(String, Span)> = vec![];
        let mut key = None;
        let mut for_ctrl_end = 0usize;
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "name" | "name2" => {
                    if key.is_none() {
                        if let Some(for_stmt) = ancestor(cap.node, "for_statement") {
                            key = Some(for_stmt.start_byte());
                            for_ctrl_end = ancestor(for_stmt, "control")
                                .map(|c| c.end_byte())
                                .unwrap_or(for_stmt.end_byte());
                        }
                    }
                    names.push((txt(cap.node, bytes).to_owned(), node_span(cap.node)));
                }
                _ => {}
            }
        }
        if let Some(k) = key {
            seen.insert(k, ());
            // Find the ForLoop region whose body_start matches this for-tag's ctrl end.
            // Fallback to end-of-source for incomplete templates (no {% endfor %}).
            let valid_range = scope_regions.iter()
                .find(|r| r.scope == VariableScope::ForLoop && r.body_start == for_ctrl_end)
                .map(|r| byte_span(r.body_start, r.body_end))
                .unwrap_or(byte_span(for_ctrl_end, source_len));
            for (name, span) in names {
                push_var(idx, name, VariableScope::ForLoop, span, valid_range.clone());
            }
        }
    }
}

fn run_for(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, skip: &HashMap<usize, ()>,
    scope_regions: &[ScopeRegion], source_len: usize,
) {
    let q = &*Q_FOR;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut name_span = Span::default();
        let mut key = None;
        let mut for_ctrl_end = 0usize;
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                name = txt(cap.node, bytes).to_owned();
                name_span = node_span(cap.node);
                if let Some(for_stmt) = ancestor(cap.node, "for_statement") {
                    key = Some(for_stmt.start_byte());
                    for_ctrl_end = ancestor(for_stmt, "control")
                        .map(|c| c.end_byte())
                        .unwrap_or(for_stmt.end_byte());
                }
            }
        }
        if !name.is_empty() && !skip.contains_key(&key.unwrap_or(0)) {
            // Fallback to end-of-source for incomplete templates (no {% endfor %}).
            let valid_range = scope_regions.iter()
                .find(|r| r.scope == VariableScope::ForLoop && r.body_start == for_ctrl_end)
                .map(|r| byte_span(r.body_start, r.body_end))
                .unwrap_or(byte_span(for_ctrl_end, source_len));
            push_var(idx, name, VariableScope::ForLoop, name_span, valid_range);
        }
    }
}

fn run_with(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, scope_regions: &[ScopeRegion],
) {
    let q = &*Q_WITH;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                let name_span = node_span(cap.node);
                let with_ctrl_end = ancestor(cap.node, "with_statement")
                    .and_then(|s| ancestor(s, "control"))
                    .map(|c| c.end_byte())
                    .unwrap_or(cap.node.end_byte());
                let valid_range = scope_regions.iter()
                    .find(|r| r.scope == VariableScope::With && r.body_start == with_ctrl_end)
                    .map(|r| byte_span(r.body_start, r.body_end))
                    .unwrap_or(byte_span(with_ctrl_end, with_ctrl_end));
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::With, name_span, valid_range);
            }
        }
    }
}

fn run_trans(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = &*Q_TRANS;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::Trans, node_span(cap.node), Span::default());
            }
        }
    }
}

fn run_caller_args(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = &*Q_CALLER_ARGS;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "caller_var" {
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::CallBlock, node_span(cap.node), Span::default());
            }
        }
    }
}

fn push_var(idx: &mut TemplateIndex, name: String, scope: VariableScope, span: Span, valid_range: Span) {
    idx.variables.push(VariableDefinition { name, scope, span, valid_range });
}

/// Compute a Span with only byte offsets set (line/col left default).
/// Used for valid_range where line/col are not needed by the resolver.
fn byte_span(start_byte: usize, end_byte: usize) -> Span {
    Span { start_byte, end_byte, ..Span::default() }
}

/// A byte range within the template source that belongs to a named scope body.
/// Jinja2 control tags are siblings in the flat tree, so scope bodies are
/// identified by the byte range between opener and closer control tags.
#[derive(Debug, Clone, Copy)]
struct ScopeRegion {
    scope: VariableScope,
    body_start: usize,
    body_end: usize,
    /// Stack depth at the time this region was opened — used to pick innermost.
    depth: usize,
}

/// Walk the root's direct children to build the list of scope body regions.
/// REQ-DATA-03/07: detects all scope-introducing bodies so that variables
/// defined inside them get correct VariableScope and valid_range.
fn build_scope_regions(root: tree_sitter::Node, bytes: &[u8]) -> Vec<ScopeRegion> {
    let mut regions = Vec::new();
    let mut stack: Vec<(VariableScope, usize, usize)> = Vec::new();

    let mut c = root.walk();
    if !c.goto_first_child() {
        return regions;
    }
    loop {
        let node = c.node();
        if node.kind() == "control" {
            if let Some(stmt) = node.named_child(0) {
                let inner_kind = stmt.named_child(0).map(|n| n.kind()).unwrap_or("");
                let open_scope = match inner_kind {
                    "macro_statement" => Some(VariableScope::Macro),
                    "block_statement" => Some(VariableScope::Block),
                    "filter_statement" => Some(VariableScope::Filter),
                    "autoescape_statement" => Some(VariableScope::Autoescape),
                    "for_statement" => Some(VariableScope::ForLoop),
                    "with_statement" => Some(VariableScope::With),
                    _ => None,
                };
                if let Some(scope) = open_scope {
                    stack.push((scope, node.end_byte(), stack.len()));
                } else {
                    let kw_full = std::str::from_utf8(&bytes[stmt.start_byte()..stmt.end_byte()])
                        .unwrap_or("").trim();
                    let kw = kw_full.split_whitespace().next().unwrap_or("");
                    if matches!(kw, "endmacro" | "endblock" | "endfilter" | "endautoescape" | "endfor" | "endwith") {
                        if let Some((scope, body_start, depth)) = stack.pop() {
                            regions.push(ScopeRegion {
                                scope,
                                body_start,
                                body_end: node.start_byte(),
                                depth,
                            });
                        }
                    }
                }
            }
        }
        if !c.goto_next_sibling() {
            break;
        }
    }
    regions
}

/// Return the innermost scope region that contains `byte`, or Template if none.
fn scope_for_byte(regions: &[ScopeRegion], byte: usize) -> VariableScope {
    regions.iter()
        .filter(|r| r.body_start <= byte && byte < r.body_end)
        .max_by_key(|r| r.depth)
        .map(|r| r.scope)
        .unwrap_or(VariableScope::Template)
}

/// Return the innermost scope region enclosing `byte`, if any.
fn enclosing_region(regions: &[ScopeRegion], byte: usize) -> Option<&ScopeRegion> {
    regions.iter()
        .filter(|r| r.body_start <= byte && byte < r.body_end)
        .max_by_key(|r| r.depth)
}

// ── template references ───────────────────────────────────────────────────────

fn do_template_refs(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    // extends
    {
        let q = &*Q_EXTENDS;
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&q, tree.root_node(), bytes);
        while let Some(m) = ms.next() {
            for cap in m.captures {
                match q.capture_names()[cap.index as usize] {
                    "path" => {
                        let raw = txt(cap.node, bytes);
                        idx.template_refs.push(TemplateReference {
                            kind: TemplateRefKind::Extends,
                            path: strip_quotes(raw),
                            ignore_missing: false,
                            is_dynamic: false,
                            span: node_span(cap.node),
                        });
                    }
                    "dynamic_path" => {
                        idx.template_refs.push(TemplateReference {
                            kind: TemplateRefKind::Extends,
                            path: txt(cap.node, bytes).to_owned(),
                            ignore_missing: false,
                            is_dynamic: true,
                            span: node_span(cap.node),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // includes — collect by statement start_byte to merge ignore_missing flag
    {
        let q = &*Q_INCLUDES;
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&q, tree.root_node(), bytes);
        // keyed by include_statement start_byte
        let mut inc_map: HashMap<usize, TemplateReference> = HashMap::new();

        while let Some(m) = ms.next() {
            for cap in m.captures {
                match q.capture_names()[cap.index as usize] {
                    "path" => {
                        let raw = txt(cap.node, bytes);
                        let key = ancestor(cap.node, "include_statement")
                            .map(|n| n.start_byte())
                            .unwrap_or(cap.node.start_byte());
                        inc_map.entry(key).or_insert(TemplateReference {
                            kind: TemplateRefKind::Include,
                            path: strip_quotes(raw),
                            ignore_missing: false,
                            is_dynamic: false,
                            span: node_span(cap.node),
                        });
                    }
                    "dynamic_path" => {
                        let key = ancestor(cap.node, "include_statement")
                            .map(|n| n.start_byte())
                            .unwrap_or(cap.node.start_byte());
                        inc_map.entry(key).or_insert(TemplateReference {
                            kind: TemplateRefKind::Include,
                            path: txt(cap.node, bytes).to_owned(),
                            ignore_missing: false,
                            is_dynamic: true,
                            span: node_span(cap.node),
                        });
                    }
                    "ignore_missing" => {
                        let key = ancestor(cap.node, "include_statement")
                            .map(|n| n.start_byte())
                            .unwrap_or(cap.node.start_byte());
                        if let Some(r) = inc_map.get_mut(&key) {
                            r.ignore_missing = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        idx.template_refs.extend(inc_map.into_values());
    }
}

// ── imports ───────────────────────────────────────────────────────────────────

fn do_imports(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = &*Q_IMPORTS;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut source = String::new();
        let mut alias = String::new();
        let mut span = Span::default();
        let mut alias_span = Span::default();
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "source" => {
                    source = strip_quotes(txt(cap.node, bytes));
                    span = node_span(cap.node);
                }
                "alias" => {
                    alias = txt(cap.node, bytes).to_owned();
                    alias_span = node_span(cap.node);
                }
                _ => {}
            }
        }
        if !source.is_empty() && !alias.is_empty() {
            idx.import_aliases.push(ImportAlias { alias, source: source.clone(), span: span.clone(), alias_span: alias_span.clone() });
            idx.template_refs.push(TemplateReference {
                kind: TemplateRefKind::Import,
                path: source,
                ignore_missing: false,
                is_dynamic: false,
                span,
            });
        }
    }
}

// ── from … import ─────────────────────────────────────────────────────────────

fn do_from_imports(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let fq = &*Q_FROM_IMPORTS;
    let nq = &*Q_IMPORT_NAMES;

    // Collect source paths keyed by import_statement start_byte
    let mut source_map: HashMap<usize, (String, Span)> = HashMap::new();
    {
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&fq, tree.root_node(), bytes);
        while let Some(m) = ms.next() {
            for cap in m.captures {
                if fq.capture_names()[cap.index as usize] == "source" {
                    let key = ancestor(cap.node, "import_statement")
                        .map(|n| n.start_byte())
                        .unwrap_or(cap.node.start_byte());
                    source_map.entry(key).or_insert_with(|| {
                        (strip_quotes(txt(cap.node, bytes)), node_span(cap.node))
                    });
                }
            }
        }
    }

    // Collect imported names keyed by import_statement start_byte.
    // Inner value: (name → (alias, name_span)).
    let mut names_map: HashMap<usize, HashMap<String, (Option<String>, Span)>> = HashMap::new();
    {
        let mut cur = QueryCursor::new();
        let mut ms = cur.matches(&nq, tree.root_node(), bytes);
        while let Some(m) = ms.next() {
            for cap in m.captures {
                let cap_name = nq.capture_names()[cap.index as usize];
                let key = ancestor(cap.node, "import_statement")
                    .map(|n| n.start_byte())
                    .unwrap_or(cap.node.start_byte());
                let entry = names_map.entry(key).or_default();
                match cap_name {
                    "name" => {
                        let n = txt(cap.node, bytes).to_owned();
                        let span = node_span(cap.node);
                        entry.entry(n).or_insert((None, span));
                    }
                    "alias" => {
                        if let Some(import_as) = cap.node.parent() {
                            if let Some(prev) = import_as.prev_named_sibling() {
                                if prev.kind() == "identifier" {
                                    let prev_name = txt(prev, bytes).to_owned();
                                    let alias_text = txt(cap.node, bytes).to_owned();
                                    if let Some(entry) = entry.get_mut(&prev_name) {
                                        entry.0 = Some(alias_text);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Assemble FromImport entries in document order (sort by start_byte so
    // W304 and similar checks always see imports in the order they appear in source).
    let mut keys: Vec<usize> = source_map.keys().cloned().collect();
    keys.sort_unstable();
    for key in keys {
        let (source, span) = source_map.remove(&key).unwrap();
        let name_alias = names_map.remove(&key).unwrap_or_default();
        let mut names: Vec<ImportedName> = name_alias
            .into_iter()
            .map(|(name, (alias, name_span))| ImportedName { name, alias, name_span })
            .collect();
        names.sort_unstable_by(|a, b| a.name_span.start_byte.cmp(&b.name_span.start_byte));
        idx.from_imports.push(FromImport { source: source.clone(), names, span: span.clone() });
        idx.template_refs.push(TemplateReference {
            kind: TemplateRefKind::From,
            path: source,
            ignore_missing: false,
            is_dynamic: false,
            span,
        });
    }
}

// ── references ────────────────────────────────────────────────────────────────

fn do_references(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = &*Q_REFERENCES;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            let kind = match q.capture_names()[cap.index as usize] {
                "identifier" | "object" => ReferenceKind::Identifier,
                "attribute" => ReferenceKind::Attribute,
                "filter" => ReferenceKind::Filter,
                "function" => ReferenceKind::Function,
                "builtin_test" | "custom_test" => ReferenceKind::Test,
                _ => continue, // helper captures (e.g. @_is_op) are silently skipped
            };
            idx.references.push(Reference {
                name: txt(cap.node, bytes).to_owned(),
                kind,
                span: node_span(cap.node),
            });
        }
    }
}

// ── call sites ────────────────────────────────────────────────────────────────

fn do_call_sites(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = &*Q_CALL_SITES;
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] != "callee" {
                continue;
            }
            let callee = txt(cap.node, bytes).to_owned();
            let span = node_span(cap.node);
            // Navigate to function_call parent to count args.
            let Some(fn_call) = cap.node.parent() else { continue };
            if fn_call.kind() != "function_call" { continue }
            let mut positional_count = 0usize;
            let mut keyword_names = Vec::new();
            let mut c = fn_call.walk();
            for child in fn_call.children(&mut c) {
                if child.kind() != "arg" { continue }
                // A keyword arg has an identifier child followed by a binary_operator ("=").
                let is_keyword = child.child_count() >= 2 && {
                    child.child(0).map(|n| n.kind() == "identifier").unwrap_or(false)
                        && child.child(1).map(|n| n.kind() == "binary_operator").unwrap_or(false)
                };
                if is_keyword {
                    if let Some(name_node) = child.child(0) {
                        keyword_names.push(txt(name_node, bytes).to_owned());
                    }
                } else {
                    positional_count += 1;
                }
            }
            idx.macro_calls.push(MacroCallSite {
                callee,
                positional_count,
                keyword_names,
                span,
            });
        }
    }
}

// ── utilities ────────────────────────────────────────────────────────────────

fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_owned()
}

/// REQ-HOV-03: find the first `{# ... #}` comment in the byte slice [start, end)
/// and return its trimmed inner text. Returns None when no comment is present.
fn extract_first_comment(bytes: &[u8], start: usize, end: usize) -> Option<String> {
    let slice = std::str::from_utf8(bytes.get(start..end)?).ok()?;
    let open = slice.find("{#")?;
    let rest = &slice[open + 2..];
    let close = rest.find("#}")?;
    let inner = rest[..close].trim();
    if inner.is_empty() { None } else { Some(inner.to_owned()) }
}
