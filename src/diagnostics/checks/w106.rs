use super::*;

// ── W106: unknown-attribute ───────────────────────────────────────────────────
// REQ-HINT-05: off by default; only fires against hinted context_variables with declared attrs.

pub(super) fn check_w106(
    source: &str,
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    out: &mut Vec<Diagnostic>,
) {
    // Dotted attribute access: {{ obj.attr }} — captured as ReferenceKind::Attribute.
    for r in &index.references {
        if r.kind != ReferenceKind::Attribute {
            continue;
        }
        let attr = r.name.as_str();
        let Some(parent) = attribute_parent(source, r.span.start_byte) else {
            continue;
        };
        let Some(entry) = registry.get(Category::ContextVariable, parent) else {
            continue;
        };
        // REQ-HINT-03: template scope — skip if this hint does not apply to the current file.
        if let Some(t) = &entry.template {
            if !path_matches_template_scope(path, t) {
                continue;
            }
        }
        let declared_attrs = registry.attrs_for(parent);
        if declared_attrs.is_empty() {
            continue;
        }
        if declared_attrs.iter().any(|a| a.attr == attr) {
            continue;
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: DiagCode::W106.code_str().to_owned(),
            slug: DiagCode::W106.slug().to_owned(),
            severity: DiagCode::W106.severity(),
            message: format!("'{}' has no declared attribute '{}'", parent, attr),
        });
    }

    // Subscript attribute access: {{ obj["attr"] }} or {{ obj['attr'] }}.
    // The tree-sitter grammar does not produce Attribute references for subscript nodes,
    // so we scan the source text directly (REQ-HINT-05).
    for (parent, attr, line, col) in subscript_accesses(source) {
        let Some(entry) = registry.get(Category::ContextVariable, parent) else {
            continue;
        };
        if let Some(t) = &entry.template {
            if !path_matches_template_scope(path, t) {
                continue;
            }
        }
        let declared_attrs = registry.attrs_for(parent);
        if declared_attrs.is_empty() {
            continue;
        }
        if declared_attrs.iter().any(|a| a.attr == attr) {
            continue;
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line,
            col,
            code: DiagCode::W106.code_str().to_owned(),
            slug: DiagCode::W106.slug().to_owned(),
            severity: DiagCode::W106.severity(),
            message: format!("'{}' has no declared attribute '{}'", parent, attr),
        });
    }
}

/// Scan source text for `identifier["key"]` and `identifier['key']` patterns,
/// but only inside real `{{ }}`/`{% %}` regions — HTML/JS text (e.g.
/// `session["user"]` inside a `<script>` block) must never match (jinja-lsp-l27o).
/// Returns (parent_name, attr_name, line, col) for each match; col points at the key.
///
/// Tracks line/col incrementally during a single forward scan (byte columns,
/// matching the rest of the codebase's convention) instead of rescanning from
/// byte 0 for every match, which was O(n^2) on subscript-heavy files.
pub(super) fn subscript_accesses(source: &str) -> Vec<(&str, &str, u32, u32)> {
    #[derive(PartialEq, Clone, Copy)]
    enum Delim {
        None,
        ExprOrStmt,
        Comment,
    }

    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut line = 0u32;
    let mut col = 0u32;
    let mut delim = Delim::None;

    // Advance the running (line, col) position by exactly one byte.
    macro_rules! advance {
        () => {
            if bytes[i] == b'\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            i += 1;
        };
    }

    while i < bytes.len() {
        match delim {
            Delim::None => {
                if bytes[i..].starts_with(b"{{") || bytes[i..].starts_with(b"{%") {
                    delim = Delim::ExprOrStmt;
                } else if bytes[i..].starts_with(b"{#") {
                    delim = Delim::Comment;
                }
                advance!();
                continue;
            }
            Delim::Comment => {
                if bytes[i..].starts_with(b"#}") {
                    delim = Delim::None;
                    advance!();
                }
                advance!();
                continue;
            }
            Delim::ExprOrStmt => {
                if bytes[i..].starts_with(b"}}") || bytes[i..].starts_with(b"%}") {
                    delim = Delim::None;
                    advance!();
                    continue;
                }
            }
        }

        // Only look for subscript patterns inside a real expression/statement.
        if delim != Delim::ExprOrStmt || bytes[i] != b'[' {
            advance!();
            continue;
        }

        // Find the identifier before `[`.
        let before_bracket = i;
        if before_bracket == 0 {
            advance!();
            continue;
        }
        let id_end = before_bracket;
        let mut id_start = id_end;
        while id_start > 0
            && (bytes[id_start - 1].is_ascii_alphanumeric() || bytes[id_start - 1] == b'_')
        {
            id_start -= 1;
        }
        if id_start == id_end {
            advance!();
            continue;
        } // no identifier before `[`
        let parent = match std::str::from_utf8(&bytes[id_start..id_end]) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                advance!();
                continue;
            }
        };
        // After `[`, expect an optional space then a quote.
        let mut j = i + 1;
        while j < bytes.len() && bytes[j] == b' ' {
            j += 1;
        }
        if j >= bytes.len() {
            advance!();
            continue;
        }
        let quote = bytes[j];
        if quote != b'"' && quote != b'\'' {
            advance!();
            continue;
        }
        let key_start = j + 1;
        let key_byte = key_start;
        // Find closing quote.
        let mut k = key_start;
        while k < bytes.len() && bytes[k] != quote {
            k += 1;
        }
        if k >= bytes.len() {
            advance!();
            continue;
        }
        let attr = match std::str::from_utf8(&bytes[key_start..k]) {
            Ok(s) if !s.is_empty() => s,
            _ => {
                advance!();
                continue;
            }
        };
        // Verify closing `]` follows.
        let mut l = k + 1;
        while l < bytes.len() && bytes[l] == b' ' {
            l += 1;
        }
        if l >= bytes.len() || bytes[l] != b']' {
            advance!();
            continue;
        }

        // Compute the key's (line, col) from the CURRENT running position, walking
        // only the short span [i..key_byte] rather than rescanning from byte 0.
        let (key_line, key_col) = {
            let mut kl = line;
            let mut kc = col;
            for &b in &bytes[i..key_byte] {
                if b == b'\n' {
                    kl += 1;
                    kc = 0;
                } else {
                    kc += 1;
                }
            }
            (kl, kc)
        };
        out.push((parent, attr, key_line, key_col));
        // Skip past the whole match, keeping line/col in sync.
        while i < l + 1 {
            advance!();
        }
    }
    out
}

/// Scan backwards from `attr_start_byte` to find the parent identifier name.
pub(super) fn attribute_parent(source: &str, attr_start_byte: usize) -> Option<&str> {
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
