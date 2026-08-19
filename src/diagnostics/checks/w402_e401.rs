use super::*;

// ── W402: unreachable-content / E401: invalid-super ──────────────────────────

pub(super) fn check_w402_e401(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    // Only applies to child templates (those that extend a parent).
    let is_child = index
        .template_refs
        .iter()
        .any(|r| matches!(r.kind, TemplateRefKind::Extends));
    if !is_child {
        return;
    }

    // Collect block body byte ranges ([body_start, body_end) = content between the tags).
    let block_ranges: Vec<(usize, usize)> = index
        .blocks
        .iter()
        .filter(|b| b.body.start_byte < b.body.end_byte)
        .map(|b| (b.body.start_byte, b.body.end_byte))
        .collect();

    // A top-level {% macro %} is valid Jinja (callable from within blocks) — set/for
    // bindings inside its body are exempt from W402 the same way block bodies are.
    let macro_ranges: Vec<(usize, usize)> = index
        .macros
        .iter()
        .filter(|m| m.body.start_byte < m.body.end_byte)
        .map(|m| (m.body.start_byte, m.body.end_byte))
        .collect();

    let inside_block = |byte: usize| {
        block_ranges.iter().any(|&(s, e)| s <= byte && byte < e)
            || macro_ranges.iter().any(|&(s, e)| s <= byte && byte < e)
    };

    // W402: variables set outside any block are unreachable in a child template.
    for v in &index.variables {
        if !inside_block(v.span.start_byte) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: DiagCode::W402.code_str().to_owned(),
                slug: DiagCode::W402.slug().to_owned(),
                severity: DiagCode::W402.severity(),
                message: format!(
                    "'{}' is outside any block and will not render in this extends template",
                    v.name
                ),
            });
        }
    }

    // E401: {{ super() }} outside any block has no parent block context.
    // Use the grammar-driven Function references (not a raw byte scan) so HTML prose,
    // comments, and other text outside Jinja delimiters can never match, and so the
    // reported span is the tree-sitter byte span every other check already uses.
    for r in &index.references {
        if r.kind == ReferenceKind::Function
            && r.name == "super"
            && !inside_block(r.span.start_byte)
        {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: r.span.start_line,
                col: r.span.start_col,
                code: DiagCode::E401.code_str().to_owned(),
                slug: DiagCode::E401.slug().to_owned(),
                severity: DiagCode::E401.severity(),
                message: "super() called outside a block".to_owned(),
            });
        }
    }
}
