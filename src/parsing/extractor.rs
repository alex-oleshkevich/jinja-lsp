use std::collections::HashMap;
use std::sync::LazyLock;

use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::workspace::{
    index::TemplateIndex,
    symbols::{
        BlockDefinition, FromImport, ImportAlias, ImportedName, MacroDefinition, Parameter,
        Reference, ReferenceKind, Span, SyntaxError, TemplateRefKind, TemplateReference,
        VariableDefinition, VariableScope,
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

// ── public API ───────────────────────────────────────────────────────────────

/// Parse `source` once, run all 17 queries, merge captures into a fresh
/// `TemplateIndex` (REQ-EXTR-03). Syntax errors (JINJA-E001) are recorded.
pub fn extract(source: &str) -> TemplateIndex {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_jinja::language()).expect("language");

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
        // body spans from the end of the {% macro %} control tag to the start of
        // the matching {% endmacro %} control tag (sibling depth-counting walk).
        let body = macro_body_span(tree.root_node(), ctrl_end, bytes);
        idx.macros.push(MacroDefinition { name, parameters, body, span });
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
    let mut seen: HashMap<usize, usize> = HashMap::new();

    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut span = Span::default();
        let mut key = 0usize;
        let mut required = false;

        for cap in m.captures {
            match bq.capture_names()[cap.index as usize] {
                "name" => {
                    name = txt(cap.node, bytes).to_owned();
                    if let Some(bs) = cap.node.parent() {
                        key = bs.start_byte();
                        span = node_span(bs);
                    }
                }
                "required" => required = true,
                _ => {}
            }
        }

        if name.is_empty() {
            continue;
        }

        if let Some(&i) = seen.get(&key) {
            if required {
                idx.blocks[i].required = true;
            }
        } else {
            seen.insert(key, idx.blocks.len());
            idx.blocks.push(BlockDefinition { name, scoped: false, required, body: Span::default(), span });
        }
    }
}

// ── variables ────────────────────────────────────────────────────────────────

fn do_variables(tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let scope_regions = build_scope_regions(tree.root_node(), bytes);
    let source_len = bytes.len();
    let mut seen_set: HashMap<usize, ()> = HashMap::new();
    let mut seen_for: HashMap<usize, ()> = HashMap::new();

    run_set_unpacking(tree, bytes, idx, &mut seen_set, &scope_regions, source_len);
    run_set(tree, bytes, idx, &seen_set, &scope_regions, source_len);
    run_for_unpacking(tree, bytes, idx, &mut seen_for, &scope_regions);
    run_for(tree, bytes, idx, &seen_for, &scope_regions);
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

fn run_for_unpacking(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, seen: &mut HashMap<usize, ()>,
    scope_regions: &[ScopeRegion],
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
            let valid_range = scope_regions.iter()
                .find(|r| r.scope == VariableScope::ForLoop && r.body_start == for_ctrl_end)
                .map(|r| byte_span(r.body_start, r.body_end))
                .unwrap_or(byte_span(for_ctrl_end, for_ctrl_end));
            for (name, span) in names {
                push_var(idx, name, VariableScope::ForLoop, span, valid_range.clone());
            }
        }
    }
}

fn run_for(
    tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, skip: &HashMap<usize, ()>,
    scope_regions: &[ScopeRegion],
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
            let valid_range = scope_regions.iter()
                .find(|r| r.scope == VariableScope::ForLoop && r.body_start == for_ctrl_end)
                .map(|r| byte_span(r.body_start, r.body_end))
                .unwrap_or(byte_span(for_ctrl_end, for_ctrl_end));
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
                    let kw = std::str::from_utf8(&bytes[stmt.start_byte()..stmt.end_byte()])
                        .unwrap_or("").trim();
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
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "source" => {
                    source = strip_quotes(txt(cap.node, bytes));
                    span = node_span(cap.node);
                }
                "alias" => alias = txt(cap.node, bytes).to_owned(),
                _ => {}
            }
        }
        if !source.is_empty() && !alias.is_empty() {
            idx.import_aliases.push(ImportAlias { alias, source: source.clone(), span: span.clone() });
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

    // Collect imported names keyed by import_statement start_byte
    // For each import_statement, build Vec<ImportedName>
    let mut names_map: HashMap<usize, HashMap<String, Option<String>>> = HashMap::new();
    // inner HashMap: name → alias
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
                        entry.entry(n).or_insert(None);
                    }
                    "alias" => {
                        // alias belongs to the most-recently-added name — find it via the
                        // import_as node's preceding sibling identifier
                        if let Some(import_as) = cap.node.parent() {
                            if let Some(prev) = import_as.prev_named_sibling() {
                                if prev.kind() == "identifier" {
                                    let prev_name = txt(prev, bytes).to_owned();
                                    let alias_text = txt(cap.node, bytes).to_owned();
                                    entry.insert(prev_name, Some(alias_text));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Assemble FromImport entries
    for (key, (source, span)) in source_map {
        let name_alias = names_map.remove(&key).unwrap_or_default();
        let names: Vec<ImportedName> = name_alias
            .into_iter()
            .map(|(name, alias)| ImportedName { name, alias })
            .collect();
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
                "identifier" => ReferenceKind::Identifier,
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

// ── utilities ────────────────────────────────────────────────────────────────

fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_owned()
}
