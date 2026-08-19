use super::*;

// ── E102: undefined filter / E104: undefined test ─────────────────────────────

pub(super) fn check_e102_e104(
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    out: &mut Vec<Diagnostic>,
) {
    for r in &index.references {
        match r.kind {
            ReferenceKind::Filter if registry.get(Category::Filter, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: DiagCode::E102.code_str().to_owned(),
                    slug: DiagCode::E102.slug().to_owned(),
                    severity: DiagCode::E102.severity(),
                    message: format!("undefined filter '{}'", r.name),
                });
            }
            ReferenceKind::Test if registry.get(Category::Test, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: DiagCode::E104.code_str().to_owned(),
                    slug: DiagCode::E104.slug().to_owned(),
                    severity: DiagCode::E104.severity(),
                    message: format!("undefined test '{}'", r.name),
                });
            }
            _ => {}
        }
    }
}
