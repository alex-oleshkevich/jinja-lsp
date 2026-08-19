use super::*;

// ── E001: syntax error ────────────────────────────────────────────────────────

pub(super) fn check_e001(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    for err in &index.syntax_errors {
        out.push(Diagnostic {
            file: path.to_owned(),
            line: err.span.start_line,
            col: err.span.start_col,
            code: DiagCode::E001.code_str().to_owned(),
            slug: DiagCode::E001.slug().to_owned(),
            severity: DiagCode::E001.severity(),
            message: "syntax error".to_owned(),
        });
    }
}
