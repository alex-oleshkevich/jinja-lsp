use super::*;

// ── W201: unused-variable ─────────────────────────────────────────────────────

pub(super) fn check_w201(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> =
        index.references.iter().map(|r| r.name.as_str()).collect();
    for v in &index.variables {
        // Skip variables with no valid_range (external/context vars or unpopulated bindings).
        if v.valid_range.start_byte >= v.valid_range.end_byte {
            continue;
        }
        if !used_names.contains(v.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: DiagCode::W201.code_str().to_owned(),
                slug: DiagCode::W201.slug().to_owned(),
                severity: DiagCode::W201.severity(),
                message: format!("variable '{}' is assigned but never used", v.name),
            });
        }
    }
}
