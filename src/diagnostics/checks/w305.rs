use super::*;

// ── W305: name-shadowing ──────────────────────────────────────────────────────

pub(super) fn check_w305(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let vars = &index.variables;
    for (i, inner) in vars.iter().enumerate() {
        let inner_start = inner.valid_range.start_byte;
        let inner_end = inner.valid_range.end_byte;
        if inner_start >= inner_end {
            continue;
        }
        for outer in vars[..i].iter() {
            if outer.name != inner.name {
                continue;
            }
            let outer_start = outer.valid_range.start_byte;
            let outer_end = outer.valid_range.end_byte;
            if outer_start >= outer_end {
                continue;
            }
            // Inner is nested within outer.
            if outer_start <= inner_start && inner_end <= outer_end {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: inner.span.start_line,
                    col: inner.span.start_col,
                    code: DiagCode::W305.code_str().to_owned(),
                    slug: DiagCode::W305.slug().to_owned(),
                    severity: DiagCode::W305.severity(),
                    message: format!("'{}' shadows an outer-scope variable", inner.name),
                });
                break; // one diagnostic per shadowed var is enough
            }
        }
    }
}
