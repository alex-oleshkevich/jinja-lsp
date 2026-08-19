use super::*;

// ── E403: missing-required-block ─────────────────────────────────────────────

pub(super) fn check_e403(
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    // Only applies to child templates.
    let extends = index
        .template_refs
        .iter()
        .find(|r| matches!(r.kind, TemplateRefKind::Extends));
    let Some(parent_ref) = extends else { return };
    let Some(parent_idx) = workspace.get_by_ref(&parent_ref.path) else {
        return;
    };

    let child_block_names: std::collections::HashSet<&str> =
        index.blocks.iter().map(|b| b.name.as_str()).collect();

    for pb in &parent_idx.blocks {
        if pb.required && !child_block_names.contains(pb.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: 0,
                col: 0,
                code: DiagCode::E403.code_str().to_owned(),
                slug: DiagCode::E403.slug().to_owned(),
                severity: DiagCode::E403.severity(),
                message: format!(
                    "required block '{}' is not overridden in this template",
                    pb.name
                ),
            });
        }
    }
}
