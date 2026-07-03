// One handler module per LSP feature (REQ-FOLD-06).
// Each is a pure-read handler dispatched from server.rs.
pub mod call_hierarchy;
pub mod code_actions;
pub mod code_lens;
pub mod completions;
pub mod definition;
pub mod document_highlight;
pub mod extract_macro;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod inlay_hints;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbols;
pub mod wrap;

pub fn layer_name() -> &'static str {
    "features"
}

/// Clamp `byte` to the nearest char boundary at or before `byte`.
/// Avoids panics when an LSP byte offset lands mid-UTF-8-sequence.
pub(super) fn clamp_to_char_boundary(source: &str, byte: usize) -> usize {
    let byte = byte.min(source.len());
    // Walk backward at most 3 bytes (max UTF-8 sequence is 4 bytes).
    (0..=byte)
        .rev()
        .find(|&b| source.is_char_boundary(b))
        .unwrap_or(0)
}

/// Returns `true` when `byte` is inside an active `{{ }}` or `{% %}` Jinja delimiter.
/// Content inside Jinja comments `{# #}` returns `false`.
pub(super) fn inside_jinja(source: &str, byte: usize) -> bool {
    let before = &source[..clamp_to_char_boundary(source, byte)];
    let is_active = |open: Option<usize>, close: Option<usize>| match (open, close) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    };
    if is_active(before.rfind("{#"), before.rfind("#}")) {
        return false;
    }
    is_active(before.rfind("{{"), before.rfind("}}"))
        || is_active(before.rfind("{%"), before.rfind("%}"))
}

/// Byte offset from the start of a `{% … %}` tag slice to just after the first keyword.
/// Used to skip the tag keyword before searching for the symbol name.
pub(super) fn after_tag_keyword(tag: &str) -> usize {
    let inner = tag.strip_prefix("{%").unwrap_or(tag);
    let inner = inner.trim_start_matches(['-', '+', ' ', '\t']);
    let keyword_len = inner
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(inner.len());
    tag.len() - inner.len() + keyword_len
}

/// Find `name` as a whole word in `source[start_byte..]`, skipping past the opening tag keyword.
pub(super) fn find_name_in_tag(source: &str, tag_start_byte: usize, name: &str) -> Option<usize> {
    let tag = source.get(tag_start_byte..)?;
    let search_from = tag_start_byte + after_tag_keyword(tag);
    let slice = source.get(search_from..)?;
    let name_bytes = name.as_bytes();
    let slice_bytes = slice.as_bytes();
    let mut i = 0usize;
    while i + name.len() <= slice.len() {
        if &slice_bytes[i..i + name.len()] == name_bytes {
            let before_ok = i == 0 || !is_ident_byte(slice_bytes[i - 1]);
            let after_ok =
                i + name.len() >= slice.len() || !is_ident_byte(slice_bytes[i + name.len()]);
            if before_ok && after_ok {
                return Some(search_from + i);
            }
        }
        i += 1;
    }
    None
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Return `true` when `tmpl` does NOT shadow `macro_name` with a local macro definition.
///
/// Used to scope cross-file symbol searches: if a template defines its own macro with the
/// same name, that template's references resolve to the local definition, not to a remote one.
pub(super) fn template_does_not_shadow_macro(
    tmpl: &crate::workspace::index::TemplateIndex,
    macro_name: &str,
) -> bool {
    !tmpl.macros.iter().any(|m| m.name == macro_name)
}

/// Extract the Jinja identifier word centered at `byte` in `source`.
pub(super) fn word_at_byte(source: &str, byte: usize) -> &str {
    let byte = clamp_to_char_boundary(source, byte);
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(source.len());
    &source[start..end]
}

// jinja-lsp-mna5: shared position/span helpers, previously copy-pasted across
// 7+ feature files (each an independent O(file) scan from byte 0).

/// Convert a 0-based (line, col) position to a byte offset. `col` is a byte
/// offset within the line, clamped to the line's own length.
pub(super) fn line_col_to_byte(source: &str, target_line: u32, target_col: u32) -> usize {
    let mut byte = 0usize;
    for (i, line) in source.split('\n').enumerate() {
        if i == target_line as usize {
            return byte + (target_col as usize).min(line.len());
        }
        byte += line.len() + 1; // +1 for the '\n'
    }
    byte
}

/// Convert a byte offset to a 0-based (line, col) position. `col` is a byte
/// offset within the line (matching `line_col_to_byte`'s convention).
pub(super) fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
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

/// True when `byte` falls strictly inside `span` (empty spans never contain anything).
pub(super) fn byte_in_span(byte: usize, span: &crate::workspace::symbols::Span) -> bool {
    span.start_byte < span.end_byte && span.start_byte <= byte && byte < span.end_byte
}

/// Tie-break priority when multiple reference kinds could apply to the same
/// name at one position — higher wins.
pub(super) fn kind_priority(kind: crate::workspace::symbols::ReferenceKind) -> u8 {
    use crate::workspace::symbols::ReferenceKind;
    match kind {
        ReferenceKind::Filter => 5,
        ReferenceKind::Function => 4,
        ReferenceKind::Test => 3,
        ReferenceKind::Identifier => 2,
        ReferenceKind::Attribute => 1,
    }
}

/// Return the identifier immediately before the `.` preceding an attribute
/// access starting at `attr_start_byte` (e.g. the `post` in `post.title`), or
/// `None` if there is no preceding `.`-joined parent.
pub(super) fn parent_of_attribute(source: &str, attr_start_byte: usize) -> Option<&str> {
    if attr_start_byte == 0 {
        return None;
    }
    let before = source.get(..attr_start_byte)?;
    let dot_pos = before.rfind('.')?;
    let before_dot = &before[..dot_pos];
    let end = before_dot.len();
    let start = before_dot
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let parent = &before_dot[start..end];
    if parent.is_empty() {
        None
    } else {
        Some(parent)
    }
}

/// Resolve a filter's short alias to its canonical registry name.
pub(super) fn resolve_filter_alias(name: &str) -> &str {
    match name {
        "d" => "default",
        "e" => "escape",
        "count" => "length",
        other => other,
    }
}
