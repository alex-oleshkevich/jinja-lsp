// Jinja-only formatter engine — called by both the LSP formatting handler
// and the `jinja-lsp format` CLI front-end (F18).
//
// REQ-FMT-01: normalize delimiter inner spacing to exactly one space.
// REQ-FMT-03: normalize whitespace-control marker spacing (handled by FMT-01 path).
// REQ-FMT-04: normalize filter-pipe spacing, is-test spacing, filter-call arg commas.
// REQ-FMT-07: honor FormattingOptions (tabSize / insertSpaces).

use tree_sitter::{Node, Parser};

pub fn layer_name() -> &'static str {
    "format"
}

/// REQ-FMT-07: Formatting options from the LSP client.
#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    /// Number of spaces per indent level (ignored when `insert_spaces` is false).
    pub tab_size: u32,
    /// Use spaces for indentation (true) or hard tabs (false).
    pub insert_spaces: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { tab_size: 2, insert_spaces: true }
    }
}

/// Format `source` with default options (2 spaces, no tabs).
/// Kept for backward-compatibility with the CLI `format` command.
pub fn format(source: &str) -> String {
    format_with_options(source, FormatOptions::default())
}

/// Format `source` respecting the given LSP FormattingOptions.
///
/// Returns the source unchanged if the file has syntax errors (P3 round-trip safety).
pub fn format_with_options(source: &str, opts: FormatOptions) -> String {
    let lang = tree_sitter_jinja::language();
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return source.to_owned();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return source.to_owned(),
    };

    // Skip files with syntax errors — do not risk producing a corrupt result.
    if tree.root_node().has_error() {
        return source.to_owned();
    }

    let bytes = source.as_bytes();
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    collect_delimiter_normalizations(tree.root_node(), bytes, &mut replacements);

    // Apply delimiter normalizations right-to-left so earlier byte offsets stay valid.
    let after_delimiters = if replacements.is_empty() {
        source.to_owned()
    } else {
        replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
        let mut result = source.to_owned();
        for (start, end, new_text) in replacements {
            result.replace_range(start..end, &new_text);
        }
        result
    };

    // REQ-FMT-02 / REQ-FMT-07: re-indent Jinja-tag lines with the client's indent unit.
    let indent_unit: String = if opts.insert_spaces {
        " ".repeat(opts.tab_size as usize)
    } else {
        "\t".to_owned()
    };
    let after_reindent = reindent(&after_delimiters, &indent_unit);

    if after_reindent == source {
        source.to_owned()
    } else {
        after_reindent
    }
}

// ── REQ-FMT-01/03/04 — Per-delimiter normalization ───────────────────────────

/// Walk the tree and collect (start_byte, end_byte, normalized_text) for every
/// `render_expression`, `control`, and `comment` node whose text changes.
fn collect_delimiter_normalizations(
    node: Node,
    bytes: &[u8],
    out: &mut Vec<(usize, usize, String)>,
) {
    let kind = node.kind();
    if matches!(kind, "render_expression" | "control" | "comment") {
        let text = node.utf8_text(bytes).unwrap_or("");
        let normalized = normalize_node(node, text, bytes);
        if normalized != text {
            out.push((node.start_byte(), node.end_byte(), normalized));
        }
        // Don't descend — the whole span is replaced.
        return;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_delimiter_normalizations(cursor.node(), bytes, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── REQ-FMT-02 — Block-body re-indentation ───────────────────────────────────

/// Paired Jinja tags that open a new indentation level.
const OPENERS: &[&str] = &[
    "block", "for", "if", "elif", "else",
    "macro", "call", "with", "filter",
    "autoescape", "trans",
];
/// Tags that close (or re-align at) an indentation level.
const CLOSERS: &[&str] = &[
    "endblock", "endfor", "endif", "endmacro", "endcall", "endwith", "endfilter",
    "elif", "else",
    "endset", "endautoescape", "endtrans",
];

/// Return true if `line` is a Jinja-tag line: first non-whitespace content is `{%`.
fn is_jinja_tag_line(line: &str) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    t.starts_with("{%")
}

/// Extract ALL `(keyword, inner_content)` pairs from ALL `{%...%}` tags on a single line.
fn jinja_tag_keywords_on_line(line: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut s = line;
    while let Some(start) = s.find("{%") {
        let after_open = &s[start + 2..];
        if let Some(end) = after_open.find("%}") {
            let inner = &after_open[..end];
            let kw_str = inner.trim_matches('-').trim();
            if let Some(first) = kw_str.split_whitespace().next() {
                result.push((first.to_owned(), kw_str.to_owned()));
            }
            s = &after_open[end + 2..];
        } else {
            break;
        }
    }
    result
}

/// Return true when `(keyword, inner)` acts as a block opener that increases indentation.
fn is_opener(kw: &str, inner: &str) -> bool {
    if kw == "set" {
        // Block set: `{% set name %}…{% endset %}` has no `=`; inline `{% set x = … %}` does.
        return !inner.contains('=');
    }
    OPENERS.contains(&kw)
}

/// Re-indent Jinja-tag lines so their leading whitespace equals `depth × indent_unit`,
/// where depth is the count of open paired tags enclosing the line.
/// Host-language lines are never modified.
pub fn reindent(source: &str, indent_unit: &str) -> String {
    let mut depth: usize = 0;
    let mut out = String::with_capacity(source.len());

    for (i, line) in source.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }

        if !is_jinja_tag_line(line) {
            // Host-language line: emit verbatim.
            out.push_str(line);
            continue;
        }

        let keywords = jinja_tag_keywords_on_line(line);
        let first_kw = keywords.first().map(|(kw, _)| kw.as_str()).unwrap_or("");

        // Closers (endblock, endif, …) and re-aligners (elif, else) print at depth-1.
        if CLOSERS.contains(&first_kw) && depth > 0 {
            depth -= 1;
        }

        // Write with current depth indentation.
        let stripped = line.trim_start_matches([' ', '\t']);
        for _ in 0..depth {
            out.push_str(indent_unit);
        }
        out.push_str(stripped);

        // Compute net depth delta from ALL tags on this line.
        //
        // First keyword: if it is a closer, the decrement was already applied in the
        // pre-step above; count it only as an opener (+1) if applicable.
        // Subsequent keywords: pure openers +1, pure closers -1, realigners net 0.
        let mut delta: isize = 0;
        for (idx, (kw, inner)) in keywords.iter().enumerate() {
            let in_openers = is_opener(kw, inner);
            let in_closers = CLOSERS.contains(&kw.as_str());
            if idx == 0 {
                // Closer role was already handled as pre-decrement; count opener role only.
                if in_openers {
                    delta += 1;
                }
            } else {
                if in_openers && !in_closers {
                    delta += 1;
                } else if in_closers && !in_openers {
                    delta -= 1;
                }
                // realigners (both opener+closer) at non-first position: net 0
            }
        }

        if delta > 0 {
            depth = depth.saturating_add(delta as usize);
        } else if delta < 0 {
            depth = depth.saturating_sub((-delta) as usize);
        }
    }

    out
}

/// Apply all active passes to a single delimiter node's text.
///
/// Pipeline: FMT-04 sub-edits first (relative positions), then FMT-01 outer padding.
fn normalize_node(node: Node, text: &str, bytes: &[u8]) -> String {
    let node_start = node.start_byte();

    // Collect FMT-04 edits: relative byte positions within `text`.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    collect_fmt04_edits(node, bytes, node_start, &mut edits);

    // Apply FMT-04 edits right-to-left.
    let content = if edits.is_empty() {
        text.to_owned()
    } else {
        edits.sort_by_key(|e| std::cmp::Reverse(e.0));
        let mut buf = text.to_owned();
        for (start, end, new_text) in edits {
            buf.replace_range(start..end, &new_text);
        }
        buf
    };

    // Apply FMT-01: normalize outer delimiter padding.
    normalize_delimiter(&content)
}

// ── REQ-FMT-04 — Filter-pipe / is-test / filter-call-arg normalization ────────

/// Walk the delimiter subtree and collect FMT-04 edits as (rel_start, rel_end, new_text).
/// `node_start` is the absolute byte of the enclosing delimiter — used to convert to relative.
fn collect_fmt04_edits(
    node: Node,
    bytes: &[u8],
    node_start: usize,
    out: &mut Vec<(usize, usize, String)>,
) {
    if node.kind() == "binary_operator" {
        let op = node.utf8_text(bytes).unwrap_or("");
        if op == "|" || op == "is" {
            let (ws_start, ws_end) = surrounding_whitespace(bytes, node.start_byte(), node.end_byte());
            let rel_start = ws_start.saturating_sub(node_start);
            let rel_end = ws_end.saturating_sub(node_start);
            out.push((rel_start, rel_end, format!(" {op} ")));
        }
        return; // binary_operator has no relevant children
    }

    if node.kind() == "function_call" && is_filter_call(node, bytes) {
        if let Some(normalized) = normalize_filter_call(node, bytes) {
            let rel_start = node.start_byte().saturating_sub(node_start);
            let rel_end = node.end_byte().saturating_sub(node_start);
            out.push((rel_start, rel_end, normalized));
        }
        // Don't descend into filter calls — we've replaced the whole span.
        return;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_fmt04_edits(cursor.node(), bytes, node_start, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Return [ws_start, ws_end) spanning the operator AND any surrounding horizontal whitespace.
fn surrounding_whitespace(bytes: &[u8], op_start: usize, op_end: usize) -> (usize, usize) {
    let mut ws_start = op_start;
    while ws_start > 0 && (bytes[ws_start - 1] == b' ' || bytes[ws_start - 1] == b'\t') {
        ws_start -= 1;
    }
    let mut ws_end = op_end;
    while ws_end < bytes.len() && (bytes[ws_end] == b' ' || bytes[ws_end] == b'\t') {
        ws_end += 1;
    }
    (ws_start, ws_end)
}

/// Return `true` when `func_call` is the right operand of a `|` binary_expression.
///
/// Grammar path: function_call → primary_expression → unary_expression → [right side of `|`]
fn is_filter_call(func_call: Node, bytes: &[u8]) -> bool {
    let Some(primary) = func_call.parent() else { return false };
    if primary.kind() != "primary_expression" { return false; }
    let Some(unary) = primary.parent() else { return false };
    if unary.kind() != "unary_expression" { return false; }
    let Some(binary) = unary.parent() else { return false };
    if binary.kind() != "binary_expression" { return false; }
    // The binary_expression's operator must be `|`.
    let mut cursor = binary.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "binary_operator" {
                return child.utf8_text(bytes).unwrap_or("") == "|";
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }
    false
}

/// Reconstruct a filter-call with normalized arg spacing: `name(arg1, arg2, ...)`.
///
/// Returns `None` if the call has no arguments (nothing to normalize).
fn normalize_filter_call(func_call: Node, bytes: &[u8]) -> Option<String> {
    // First named child is the identifier (function name).
    let name_node = func_call.named_child(0)?;
    let name = name_node.utf8_text(bytes).ok()?;

    // Collect all `arg` children.
    let mut args: Vec<String> = Vec::new();
    let mut cursor = func_call.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "arg" {
                let arg_text = child.utf8_text(bytes).ok()?;
                args.push(arg_text.trim().to_owned());
            }
            if !cursor.goto_next_sibling() { break; }
        }
    }

    if args.is_empty() {
        return None;
    }

    let reconstructed = format!("{}({})", name, args.join(", "));
    let original = func_call.utf8_text(bytes).ok()?;
    if reconstructed == original {
        None
    } else {
        Some(reconstructed)
    }
}

// ── REQ-FMT-01 — Delimiter outer padding ─────────────────────────────────────

/// Normalize the padding just inside a single Jinja delimiter.
///
/// Handles optional whitespace-control markers (`{%-`, `-%}` etc.) and
/// preserves multi-line comment interior by trimming only boundary whitespace.
pub fn normalize_delimiter(text: &str) -> String {
    // Detect opening: {{-, {{, {%-, {%, {#-, {#
    let (open, rest) = if let Some(r) = text.strip_prefix("{{-") { ("{{-", r) }
        else if let Some(r) = text.strip_prefix("{%-") { ("{%-", r) }
        else if let Some(r) = text.strip_prefix("{#-") { ("{#-", r) }
        else if let Some(r) = text.strip_prefix("{{") { ("{{", r) }
        else if let Some(r) = text.strip_prefix("{%") { ("{%", r) }
        else if let Some(r) = text.strip_prefix("{#") { ("{#", r) }
        else { return text.to_owned() };

    // Detect closing: -}}, }}, -%}, %}, -#}, #}
    let (content, close) = if let Some(c) = rest.strip_suffix("-}}") { (c, "-}}") }
        else if let Some(c) = rest.strip_suffix("-%}") { (c, "-%}") }
        else if let Some(c) = rest.strip_suffix("-#}") { (c, "-#}") }
        else if let Some(c) = rest.strip_suffix("}}") { (c, "}}") }
        else if let Some(c) = rest.strip_suffix("%}") { (c, "%}") }
        else if let Some(c) = rest.strip_suffix("#}") { (c, "#}") }
        else { return text.to_owned() };

    // Trim only horizontal whitespace at boundaries (preserves multi-line interiors).
    let trimmed = content.trim_matches([' ', '\t']);
    format!("{open} {trimmed} {close}")
}
