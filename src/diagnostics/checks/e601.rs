use super::*;

// ── E601: template-does-not-exist ─────────────────────────────────────────────

pub(super) fn check_e601(
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if matches!(
            tr.kind,
            TemplateRefKind::Extends
                | TemplateRefKind::Include
                | TemplateRefKind::Import
                | TemplateRefKind::From
        ) && workspace.get_by_ref(&tr.path).is_none()
        {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: tr.span.start_line,
                col: tr.span.start_col,
                code: DiagCode::E601.code_str().to_owned(),
                slug: DiagCode::E601.slug().to_owned(),
                severity: DiagCode::E601.severity(),
                message: format!("template '{}' does not exist", tr.path),
            });
        }
    }
}
