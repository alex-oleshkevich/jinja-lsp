// REQ-INLN-02, REQ-INLN-03, REQ-INLN-04: detect embedded Jinja templates
// in host files using lightweight pattern matching — no host-AST parsing.

/// A Jinja template string found inside a host file.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineRegion {
    /// The extracted Jinja source string (without surrounding quotes).
    pub content: String,
    /// Byte offset of `content[0]` inside the original host-file source.
    pub host_offset: usize,
    /// 0-indexed line number of the content start in the host file.
    pub host_line: u32,
    /// 0-indexed column of the content start in the host file.
    pub host_col: u32,
}

/// REQ-INLN-02: scan `source` for calls to any of `patterns` whose string
/// argument is a single- or double-quoted literal, and return one InlineRegion
/// per match.
///
/// REQ-INLN-04: only direct string literals are matched; variable arguments,
/// f-strings, and concatenated strings are not detected.
pub fn detect_inline_regions(source: &str, patterns: &[&str]) -> Vec<InlineRegion> {
    let mut regions = Vec::new();
    for pattern in patterns {
        let call_prefix = format!("{pattern}(");
        let mut search_from = 0;
        while let Some(call_pos) = source[search_from..].find(&call_prefix) {
            let abs_call = search_from + call_pos;
            // Word-boundary check: reject if the preceding byte is an identifier char,
            // which means we matched a suffix of a longer name (e.g. "prerender").
            let preceded_by_ident = abs_call > 0 && {
                let b = source.as_bytes()[abs_call - 1];
                b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
            };
            if !preceded_by_ident {
                let after_paren = abs_call + call_prefix.len();
                if let Some(region) = extract_string_literal(source, after_paren) {
                    regions.push(region);
                }
            }
            search_from = abs_call + call_prefix.len();
        }
    }
    // Sort by host_offset so the order is deterministic
    regions.sort_by_key(|r| r.host_offset);
    regions
}

/// Extract a single- or double-quoted string literal starting at `pos` in
/// `source`, returning the content (without quotes) and its host-file location.
fn extract_string_literal(source: &str, pos: usize) -> Option<InlineRegion> {
    let rest = source.get(pos..)?;
    // skip leading whitespace
    let trimmed_offset = rest.find(|c: char| !c.is_whitespace())?;
    let literal_start = pos + trimmed_offset;
    let quote = source.as_bytes().get(literal_start).copied()?;
    if quote != b'"' && quote != b'\'' {
        return None; // not a literal
    }
    let content_start = literal_start + 1;
    let after_quote = source.get(content_start..)?;
    // find closing quote (simple scan, no escape handling for v1)
    let content_len = after_quote.find(quote as char)?;
    let content = after_quote[..content_len].to_owned();
    let host_offset = content_start;
    let (host_line, host_col) = line_col(source, host_offset);
    Some(InlineRegion { content, host_offset, host_line, host_col })
}

fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let before = &source[..offset.min(source.len())];
    let line = before.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let col = before[line_start..].len() as u32;
    (line, col)
}
