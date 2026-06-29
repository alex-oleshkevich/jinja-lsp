use std::collections::HashMap;

use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

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

fn qry(lang: &Language, src: &str) -> Query {
    Query::new(lang, src).expect("query compile")
}

// ── public API ───────────────────────────────────────────────────────────────

/// Parse `source` once, run all 17 queries, merge captures into a fresh
/// `TemplateIndex` (REQ-EXTR-03). Syntax errors (JINJA-E001) are recorded.
pub fn extract(source: &str) -> TemplateIndex {
    let lang = tree_sitter_jinja::language();
    let mut parser = Parser::new();
    parser.set_language(&lang).expect("language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return TemplateIndex::empty(),
    };

    let bytes = source.as_bytes();
    let mut idx = TemplateIndex::empty();

    collect_errors(tree.root_node(), &mut idx.syntax_errors);
    do_macros(&lang, &tree, bytes, &mut idx);
    do_blocks(&lang, &tree, bytes, &mut idx);
    do_variables(&lang, &tree, bytes, &mut idx);
    do_template_refs(&lang, &tree, bytes, &mut idx);
    do_imports(&lang, &tree, bytes, &mut idx);
    do_from_imports(&lang, &tree, bytes, &mut idx);
    do_references(&lang, &tree, bytes, &mut idx);

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

fn do_macros(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let mq = qry(lang, include_str!("queries/macros.scm"));
    let pq = qry(lang, include_str!("queries/params.scm"));

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

fn do_blocks(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let bq = qry(lang, include_str!("queries/blocks.scm"));
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

fn do_variables(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    // Process the "unpacking" variants first so their statement keys are known;
    // the single-var variants skip those keys to avoid double-counting.
    let mut seen_set: HashMap<usize, ()> = HashMap::new();
    let mut seen_for: HashMap<usize, ()> = HashMap::new();

    run_set_unpacking(lang, tree, bytes, idx, &mut seen_set);
    run_set(lang, tree, bytes, idx, &seen_set);
    run_for_unpacking(lang, tree, bytes, idx, &mut seen_for);
    run_for(lang, tree, bytes, idx, &seen_for);
    run_with(lang, tree, bytes, idx);
    run_trans(lang, tree, bytes, idx);
    run_caller_args(lang, tree, bytes, idx);
}

fn run_set_unpacking(
    lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, seen: &mut HashMap<usize, ()>,
) {
    let q = qry(lang, include_str!("queries/set_unpacking.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut names: Vec<String> = vec![];
        let mut key = None;
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "name" | "name2" => {
                    names.push(txt(cap.node, bytes).to_owned());
                    if key.is_none() {
                        key = ancestor(cap.node, "set_statement").map(|n| n.start_byte());
                    }
                }
                _ => {}
            }
        }
        if let Some(k) = key {
            seen.insert(k, ());
            for name in names {
                push_var(idx, name, VariableScope::Template);
            }
        }
    }
}

fn run_set(
    lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, skip: &HashMap<usize, ()>,
) {
    let q = qry(lang, include_str!("queries/set.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut key = None;
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                name = txt(cap.node, bytes).to_owned();
                key = ancestor(cap.node, "set_statement").map(|n| n.start_byte());
            }
        }
        if !name.is_empty() && !skip.contains_key(&key.unwrap_or(0)) {
            push_var(idx, name, VariableScope::Template);
        }
    }
}

fn run_for_unpacking(
    lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, seen: &mut HashMap<usize, ()>,
) {
    let q = qry(lang, include_str!("queries/for_unpacking.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut names: Vec<String> = vec![];
        let mut key = None;
        for cap in m.captures {
            match q.capture_names()[cap.index as usize] {
                "name" | "name2" => {
                    names.push(txt(cap.node, bytes).to_owned());
                    if key.is_none() {
                        key = ancestor(cap.node, "for_statement").map(|n| n.start_byte());
                    }
                }
                _ => {}
            }
        }
        if let Some(k) = key {
            seen.insert(k, ());
            for name in names {
                push_var(idx, name, VariableScope::ForLoop);
            }
        }
    }
}

fn run_for(
    lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8],
    idx: &mut TemplateIndex, skip: &HashMap<usize, ()>,
) {
    let q = qry(lang, include_str!("queries/for.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        let mut name = String::new();
        let mut key = None;
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                name = txt(cap.node, bytes).to_owned();
                key = ancestor(cap.node, "for_statement").map(|n| n.start_byte());
            }
        }
        if !name.is_empty() && !skip.contains_key(&key.unwrap_or(0)) {
            push_var(idx, name, VariableScope::ForLoop);
        }
    }
}

fn run_with(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = qry(lang, include_str!("queries/with.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::With);
            }
        }
    }
}

fn run_trans(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = qry(lang, include_str!("queries/trans.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "name" {
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::Trans);
            }
        }
    }
}

fn run_caller_args(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = qry(lang, include_str!("queries/caller_args.scm"));
    let mut cur = QueryCursor::new();
    let mut ms = cur.matches(&q, tree.root_node(), bytes);
    while let Some(m) = ms.next() {
        for cap in m.captures {
            if q.capture_names()[cap.index as usize] == "caller_var" {
                push_var(idx, txt(cap.node, bytes).to_owned(), VariableScope::CallBlock);
            }
        }
    }
}

fn push_var(idx: &mut TemplateIndex, name: String, scope: VariableScope) {
    idx.variables.push(VariableDefinition { name, scope, span: Span::default(), valid_range: Span::default() });
}

// ── template references ───────────────────────────────────────────────────────

fn do_template_refs(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    // extends
    {
        let q = qry(lang, include_str!("queries/extends.scm"));
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
        let q = qry(lang, include_str!("queries/includes.scm"));
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

fn do_imports(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = qry(lang, include_str!("queries/imports.scm"));
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

fn do_from_imports(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    // from_imports.scm gives source + direct identifier names (not via import_as)
    // import_names.scm gives all names and their aliases
    // Strategy: collect from_imports.scm for source + key; collect import_names.scm for
    // full name+alias data; associate by import_statement start_byte.

    let fq = qry(lang, include_str!("queries/from_imports.scm"));
    let nq = qry(lang, include_str!("queries/import_names.scm"));

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

fn do_references(lang: &Language, tree: &tree_sitter::Tree, bytes: &[u8], idx: &mut TemplateIndex) {
    let q = qry(lang, include_str!("queries/references.scm"));
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
