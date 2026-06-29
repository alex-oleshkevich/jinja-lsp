// Jinja-only formatter engine — called by both the LSP formatting handler
// and the `jinja-lsp format` CLI front-end (F18).
//
// REQ-FMT-01: normalize delimiter inner spacing to exactly one space.

use tree_sitter::Parser;

pub fn layer_name() -> &'static str {
    "format"
}

/// Format `source` by running all enabled passes.
///
/// Returns the source unchanged if the file has syntax errors (P3 round-trip safety).
pub fn format(source: &str) -> String {
    let lang = tree_sitter_jinja::language();
    let mut parser = Parser::new();
    parser.set_language(&lang).expect("language");

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

    if replacements.is_empty() {
        return source.to_owned();
    }

    // Apply right-to-left so earlier byte offsets stay valid.
    replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut result = source.to_owned();
    for (start, end, new_text) in replacements {
        result.replace_range(start..end, &new_text);
    }
    result
}

// ── REQ-FMT-01 — Delimiter spacing ───────────────────────────────────────────

/// Walk the tree and collect (start_byte, end_byte, normalized_text) for every
/// `render_expression`, `control`, and `comment` node whose spacing differs.
fn collect_delimiter_normalizations(
    node: tree_sitter::Node,
    bytes: &[u8],
    out: &mut Vec<(usize, usize, String)>,
) {
    let kind = node.kind();
    if matches!(kind, "render_expression" | "control" | "comment") {
        let text = node.utf8_text(bytes).unwrap_or("");
        let normalized = normalize_delimiter(text);
        if normalized != text {
            out.push((node.start_byte(), node.end_byte(), normalized));
        }
        // Don't descend into these nodes — the whole span is replaced.
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
