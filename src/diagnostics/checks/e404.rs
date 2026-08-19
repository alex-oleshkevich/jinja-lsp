use super::*;

// ── E404: recursive-import ────────────────────────────────────────────────────

pub(super) fn check_e404(
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if !matches!(
            tr.kind,
            TemplateRefKind::Extends | TemplateRefKind::Import | TemplateRefKind::From
        ) {
            continue;
        }
        let mut visited = HashSet::new();
        visited.insert(path.to_owned());
        if import_chain_contains(tr.path.as_str(), path, &mut visited, workspace) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: tr.span.start_line,
                col: tr.span.start_col,
                code: DiagCode::E404.code_str().to_owned(),
                slug: DiagCode::E404.slug().to_owned(),
                severity: DiagCode::E404.severity(),
                message: format!("import of '{}' creates a recursive cycle", tr.path),
            });
        }
    }
}

pub(super) fn import_chain_contains(
    current: &str,
    target: &str,
    visited: &mut HashSet<String>,
    workspace: &WorkspaceIndex,
) -> bool {
    // Resolve current to the workspace key (handles relative ref vs absolute key mismatch).
    let current_key = match workspace.resolve_key(current) {
        Some(k) => k.to_owned(),
        None => return false,
    };
    if current_key == target {
        return true;
    }
    if !visited.insert(current_key.clone()) {
        return false;
    }
    let Some(idx) = workspace.templates.get(&current_key) else {
        return false;
    };
    for tr in &idx.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if !matches!(
            tr.kind,
            TemplateRefKind::Extends | TemplateRefKind::Import | TemplateRefKind::From
        ) {
            continue;
        }
        if import_chain_contains(tr.path.as_str(), target, visited, workspace) {
            return true;
        }
    }
    false
}
